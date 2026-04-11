//! transpile — llm-transpiler CLI
//!
//! Usage examples:
//!   transpile --input doc.md
//!   transpile --input doc.html --format html --fidelity compressed --budget 2048
//!   transpile --input doc.md --count
//!   cat doc.md | transpile --format markdown
//!   transpile --input doc.md --json

use clap::{Parser, ValueEnum};
use llm_transpiler::{FidelityLevel, InputFormat, token_count, transpile};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process;

#[derive(Parser)]
#[command(
    name = "transpile",
    about = "Convert documents to LLM-optimized bridge format",
    version
)]
struct Cli {
    /// Input file path (reads from stdin if omitted)
    #[arg(short, long)]
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

    /// Print only input/output token counts, not the transpiled content
    #[arg(short, long)]
    count: bool,

    /// Output result as JSON {input_tok, output_tok, reduction_pct, content}
    #[arg(short, long)]
    json: bool,

    /// Suppress the stats line written to stderr ([N → M tok  X% reduction])
    #[arg(short, long)]
    quiet: bool,

    /// Print stats line to stdout after content (instead of stderr)
    /// Useful when you want content + stats in a single captured stream
    #[arg(long)]
    stats: bool,
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

/// Detect format from file extension, falling back to the CLI flag.
fn detect_format(path: &Path, flag: &FormatArg) -> InputFormat {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") | Some("htm") => InputFormat::Html,
        Some("txt") => InputFormat::PlainText,
        Some("md") | Some("markdown") => InputFormat::Markdown,
        _ => flag.to_input_format(),
    }
}

fn main() {
    let cli = Cli::parse();

    // ── Read input ──────────────────────────────────────────────────────────
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

    // ── --count shortcut ────────────────────────────────────────────────────
    if cli.count {
        println!("input tokens: {input_tok}");
        return;
    }

    // ── Transpile ────────────────────────────────────────────────────────────
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

    // ── Output ───────────────────────────────────────────────────────────────
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
            // Stats on stdout after content — useful when capturing a single stream
            println!("\n\n{stats_line}");
        } else if !cli.quiet {
            // Default: stats on stderr so stdout stays clean for piping
            eprintln!("\n{stats_line}");
        }
    }
}
