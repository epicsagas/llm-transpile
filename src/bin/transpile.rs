//! transpile — llm-transpile CLI
//!
//! Transpile:
//!   transpile --input doc.md
//!   cat doc.md | transpile --format markdown --fidelity compressed
//!   transpile --input doc.md --json
//!
//! Setup:
//!   transpile install            # interactive wizard
//!   transpile install claude     # install specific tool
//!   transpile install --all      # install all detected tools
//!   transpile uninstall          # remove everything

mod install;

use clap::{Parser, Subcommand, ValueEnum};
use llm_transpile::{FidelityLevel, InputFormat, token_count, transpile};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process;

// ── CLI definition ─────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "transpile",
    about = "Convert documents to LLM-optimized bridge format",
    long_about = "Convert documents to LLM-optimized bridge format.\n\nRun `transpile install` to configure integrations with Claude Code, Gemini CLI, Codex, Cursor, and OpenCode.",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Input file path (reads from stdin if omitted)
    #[arg(short, long, global = false)]
    input: Option<PathBuf>,

    /// Input format (auto-detected from file extension if omitted)
    #[arg(short, long, value_enum, default_value = "markdown")]
    format: FormatArg,

    /// Fidelity / compression level
    #[arg(short = 'l', long, value_enum, default_value = "semantic")]
    fidelity: FidelityArg,

    /// Token budget (unlimited if omitted)
    #[arg(short, long)]
    budget: Option<usize>,

    /// Print only the input token count, then exit
    #[arg(short, long)]
    count: bool,

    /// Output as JSON {input_tok, output_tok, reduction_pct, content}
    #[arg(short, long)]
    json: bool,

    /// Suppress the stats line on stderr
    #[arg(short, long)]
    quiet: bool,

    /// Print stats to stdout after content (single-stream capture)
    #[arg(long)]
    stats: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Configure integrations with AI coding tools (interactive wizard)
    ///
    /// Examples:
    ///   transpile install              # interactive — pick tools from menu
    ///   transpile install claude       # install only Claude Code
    ///   transpile install gemini codex # install specific tools
    ///   transpile install --all        # install every detected tool
    Install {
        /// Tools to install (omit for interactive menu)
        tools: Vec<String>,
        /// Install into all supported tools without prompting
        #[arg(long)]
        all: bool,
    },
    /// Remove all integrations installed by `transpile install`
    ///
    /// Examples:
    ///   transpile uninstall            # remove everything
    ///   transpile uninstall claude     # remove only Claude Code
    Uninstall {
        /// Tools to remove (omit to remove all)
        tools: Vec<String>,
    },
}

#[derive(Clone, ValueEnum)]
enum FormatArg {
    Markdown,
    Html,
    Plaintext,
}

#[derive(Clone, ValueEnum)]
enum FidelityArg {
    Lossless,
    Semantic,
    Compressed,
}

impl FormatArg {
    fn to_input_format(&self) -> InputFormat {
        match self {
            FormatArg::Markdown => InputFormat::Markdown,
            FormatArg::Html => InputFormat::Html,
            FormatArg::Plaintext => InputFormat::PlainText,
        }
    }
}

impl FidelityArg {
    fn to_fidelity_level(&self) -> FidelityLevel {
        match self {
            FidelityArg::Lossless => FidelityLevel::Lossless,
            FidelityArg::Semantic => FidelityLevel::Semantic,
            FidelityArg::Compressed => FidelityLevel::Compressed,
        }
    }
}

fn detect_format(path: &Path, flag: &FormatArg) -> InputFormat {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") | Some("htm") => InputFormat::Html,
        Some("txt") => InputFormat::PlainText,
        Some("md") | Some("markdown") => InputFormat::Markdown,
        _ => flag.to_input_format(),
    }
}

// ── Main ───────────────────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Install { tools, all }) => {
            process::exit(install::run_install(tools, all));
        }
        Some(Command::Uninstall { tools }) => {
            process::exit(install::run_uninstall(tools));
        }
        None => run_transpile(cli),
    }
}

fn run_transpile(cli: Cli) {
    let (input_text, format) = match &cli.input {
        Some(path) => {
            let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("error: cannot read '{}': {e}", path.display());
                process::exit(1);
            });
            let fmt = detect_format(path, &cli.format);
            (text, fmt)
        }
        None => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf).unwrap_or_else(|e| {
                eprintln!("error: failed to read stdin: {e}");
                process::exit(1);
            });
            (buf, cli.format.to_input_format())
        }
    };

    let fidelity = cli.fidelity.to_fidelity_level();
    let input_tok = token_count(&input_text);

    if cli.count {
        println!("input tokens: {input_tok}");
        return;
    }

    let output = transpile(&input_text, format, fidelity, cli.budget).unwrap_or_else(|e| {
        eprintln!("error: transpile failed: {e}");
        process::exit(1);
    });

    let output_tok = token_count(&output);
    let reduction = if input_tok > 0 {
        100.0 - (output_tok as f64 / input_tok as f64 * 100.0)
    } else {
        0.0
    };

    let stats_line = format!("[{input_tok} → {output_tok} tok  {reduction:.1}% reduction]");

    if cli.json {
        let obj = serde_json::json!({
            "input_tok": input_tok,
            "output_tok": output_tok,
            "reduction_pct": format!("{reduction:.1}"),
            "content": output,
        });
        println!("{}", obj);
    } else {
        print!("{output}");
        if cli.stats {
            println!("\n\n{stats_line}");
        } else if !cli.quiet {
            eprintln!("\n{stats_line}");
        }
    }
}
