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
use std::io::{self, Read, Write};
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

    /// Print the PostToolUse hook script to stdout and exit
    #[arg(long)]
    print_hook_script: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Show transpile usage statistics
    ///
    /// Examples:
    ///   transpile stats                # last 1 day
    ///   transpile stats --days 7       # last 7 days
    ///   transpile stats --agent claude # filter by agent
    Stats {
        /// Number of days to look back (default: 1)
        #[arg(long, default_value = "1")]
        days: u32,
        /// Filter by agent name (claude, gemini, codex, opencode)
        #[arg(long)]
        agent: Option<String>,
    },
    /// Configure integrations with AI coding tools (interactive wizard)
    ///
    /// Examples:
    ///   transpile install              # interactive — pick tools from menu
    ///   transpile install claude       # install only Claude Code
    ///   transpile install gemini codex # install specific tools
    ///   transpile install --all        # install every detected tool
    ///   transpile install --list       # show available integrations + status
    ///   transpile install --dry-run    # preview without writing files
    Install {
        /// Integrations to install (omit for interactive menu)
        tools: Vec<String>,
        /// Install all supported integrations without prompting
        #[arg(long)]
        all: bool,
        /// List available integrations and their current status
        #[arg(long)]
        list: bool,
        /// Preview changes without writing any files
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove integrations installed by `transpile install`
    ///
    /// Examples:
    ///   transpile uninstall            # interactive — pick what to remove
    ///   transpile uninstall claude     # remove only Claude Code
    ///   transpile uninstall --all      # remove all installed integrations
    ///   transpile uninstall --dry-run  # preview without removing files
    Uninstall {
        /// Integrations to remove (omit for interactive menu)
        tools: Vec<String>,
        /// Remove all installed integrations without prompting
        #[arg(long)]
        all: bool,
        /// Preview changes without removing any files
        #[arg(long)]
        dry_run: bool,
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

const HOOK_SCRIPT: &str = r#"#!/usr/bin/env bash
set -euo pipefail
THRESHOLD=${TRANSPILE_THRESHOLD:-8192}
INPUT=$(cat)
FILE=$(printf '%s' "$INPUT" | python3 -c "
import json, sys
d = json.load(sys.stdin)
print(d.get('tool_input', {}).get('file_path', ''))
" 2>/dev/null) || exit 0
[ -z "$FILE" ] && exit 0
[ -f "$FILE" ] || exit 0
BYTES=$(wc -c < "$FILE" 2>/dev/null || echo 0)
[ "$BYTES" -lt "$THRESHOLD" ] && exit 0
export TRANSPILE_AGENT=claude
JSON_OUT=$(transpile --input "$FILE" --fidelity semantic --json 2>/dev/null) || exit 0
[ -z "$JSON_OUT" ] && exit 0
FNAME=$(basename "$FILE")
python3 -c "
import json, sys

data  = json.loads(sys.argv[1])
fname = sys.argv[2]
size  = sys.argv[3]

content = data.get('content', '')
inp     = data.get('input_tok', 0)
out     = data.get('output_tok', 0)
pct     = data.get('reduction_pct', '0')
saved   = inp - out

msg = (
    f'[llm-transpile] {fname} ({size}B) \u2192 {inp} tok \u2192 {out} tok '
    f'({pct}% reduction, {saved} tokens saved)\n\n{content}'
)
print(json.dumps({'additionalContext': msg}))
" "$JSON_OUT" "$FNAME" "$BYTES" 2>/dev/null || exit 0
"#;

fn main() {
    let cli = Cli::parse();

    if cli.print_hook_script {
        print!("{HOOK_SCRIPT}");
        process::exit(0);
    }

    match cli.command {
        Some(Command::Stats { days, agent }) => {
            process::exit(run_stats(days, agent));
        }
        Some(Command::Install {
            tools,
            all,
            list,
            dry_run,
        }) => {
            process::exit(install::run_install(tools, all, dry_run, list));
        }
        Some(Command::Uninstall {
            tools,
            all,
            dry_run,
        }) => {
            process::exit(install::run_uninstall(tools, all, dry_run));
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

    // Log stats to ~/.agents/transpile/stats/YYYY-MM-DD.jsonl
    log_stats(
        cli.input.as_deref(),
        &format,
        &fidelity,
        input_tok,
        output_tok,
        reduction,
    );

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

// ── Stats subcommand ───────────────────────────────────────────────────────────

/// One row in the output table, keyed by (date, agent).
#[derive(Debug, PartialEq)]
struct StatsRow {
    date: String,
    agent: String,
    calls: u64,
    input_tok: u64,
    output_tok: u64,
    saved: u64,
}

impl StatsRow {
    fn reduction_pct(&self) -> f64 {
        if self.input_tok == 0 {
            0.0
        } else {
            self.saved as f64 / self.input_tok as f64 * 100.0
        }
    }
}

/// Parse JSONL lines and aggregate into rows, filtered by agent (if given).
fn aggregate_lines(lines: &[&str], agent_filter: Option<&str>) -> Vec<StatsRow> {
    use std::collections::BTreeMap;

    // key: (date, agent)
    let mut map: BTreeMap<(String, String), StatsRow> = BTreeMap::new();

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };

        let ts = v["ts"].as_str().unwrap_or("");
        let date = ts.get(..10).unwrap_or("").to_string();
        let agent = v["agent"].as_str().unwrap_or("").to_string();

        if let Some(filter) = agent_filter
            && agent != filter
        {
            continue;
        }

        let input_tok = v["input_tok"].as_u64().unwrap_or(0);
        let output_tok = v["output_tok"].as_u64().unwrap_or(0);
        let saved = v["saved"].as_u64().unwrap_or(0);

        let entry = map
            .entry((date.clone(), agent.clone()))
            .or_insert(StatsRow {
                date,
                agent,
                calls: 0,
                input_tok: 0,
                output_tok: 0,
                saved: 0,
            });
        entry.calls += 1;
        entry.input_tok += input_tok;
        entry.output_tok += output_tok;
        entry.saved += saved;
    }

    map.into_values().collect()
}

fn run_stats(days: u32, agent: Option<String>) -> i32 {
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => {
            eprintln!("error: HOME environment variable not set");
            return 1;
        }
    };
    let stats_dir = PathBuf::from(&home).join(".agents/transpile/stats");

    // Collect date strings for the range [today-days+1 .. today]
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let today_days = now_secs / 86400;

    let mut all_lines: Vec<String> = Vec::new();

    for offset in 0..days as u64 {
        let day = today_days.saturating_sub(offset);
        let (y, m, d) = epoch_days_to_ymd(day);
        let date_str = format!("{y:04}-{m:02}-{d:02}");
        let path = stats_dir.join(format!("{date_str}.jsonl"));
        if let Ok(contents) = std::fs::read_to_string(&path) {
            for line in contents.lines() {
                all_lines.push(line.to_string());
            }
        }
    }

    let borrowed: Vec<&str> = all_lines.iter().map(|s| s.as_str()).collect();
    let rows = aggregate_lines(&borrowed, agent.as_deref());

    if rows.is_empty() {
        println!("No stats found. Run transpile on some files first.");
        return 0;
    }

    let label = if days == 1 {
        "last 1 day".to_string()
    } else {
        format!("last {days} days")
    };
    println!("transpile stats — {label}");
    println!();

    let sep = "  ──────────────────────────────────────────────────────────────────────────";
    println!(
        "  {:<12}  {:<12}  {:>5}  {:>10}  {:>10}  {:>7}  {:>9}",
        "Date", "Agent", "Calls", "Input tok", "Output tok", "Saved", "Reduction"
    );
    println!("{sep}");

    let mut total_calls: u64 = 0;
    let mut total_input: u64 = 0;
    let mut total_output: u64 = 0;
    let mut total_saved: u64 = 0;

    for row in &rows {
        total_calls += row.calls;
        total_input += row.input_tok;
        total_output += row.output_tok;
        total_saved += row.saved;

        println!(
            "  {:<12}  {:<12}  {:>5}  {:>10}  {:>10}  {:>7}  {:>8.1}%",
            row.date,
            row.agent,
            row.calls,
            format_num(row.input_tok),
            format_num(row.output_tok),
            format_num(row.saved),
            row.reduction_pct(),
        );
    }

    println!("{sep}");

    let total_reduction = if total_input > 0 {
        total_saved as f64 / total_input as f64 * 100.0
    } else {
        0.0
    };
    println!(
        "  {:<12}  {:<12}  {:>5}  {:>10}  {:>10}  {:>7}  {:>8.1}%",
        "Total",
        "",
        total_calls,
        format_num(total_input),
        format_num(total_output),
        format_num(total_saved),
        total_reduction,
    );

    0
}

/// Format a number with thousands separators (space-separated groups of 3).
fn format_num(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(' ');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

fn log_stats(
    input_path: Option<&Path>,
    format: &InputFormat,
    fidelity: &FidelityLevel,
    input_tok: usize,
    output_tok: usize,
    reduction: f64,
) {
    // Best-effort — never fail the main pipeline
    let _ = try_log_stats(
        input_path, format, fidelity, input_tok, output_tok, reduction,
    );
}

fn try_log_stats(
    input_path: Option<&Path>,
    format: &InputFormat,
    fidelity: &FidelityLevel,
    input_tok: usize,
    output_tok: usize,
    reduction: f64,
) -> io::Result<()> {
    let home = std::env::var("HOME").map_err(io::Error::other)?;
    let stats_dir = PathBuf::from(&home).join(".agents/transpile/stats");
    std::fs::create_dir_all(&stats_dir)?;

    // Date-partitioned file
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days = secs / 86400;
    let (y, m, d) = epoch_days_to_ymd(days);
    let date_str = format!("{y:04}-{m:02}-{d:02}");

    let h = (secs % 86400) / 3600;
    let min = (secs % 3600) / 60;
    let s = secs % 60;
    let ts = format!("{date_str}T{h:02}:{min:02}:{s:02}Z");

    let agent = std::env::var("TRANSPILE_AGENT").unwrap_or_default();
    let file_name = input_path
        .and_then(|p| p.file_name())
        .and_then(|f| f.to_str())
        .unwrap_or("stdin");

    let fmt_str = match format {
        InputFormat::Markdown => "markdown",
        InputFormat::Html => "html",
        InputFormat::PlainText => "plaintext",
    };
    let fid_str = match fidelity {
        FidelityLevel::Lossless => "lossless",
        FidelityLevel::Semantic => "semantic",
        FidelityLevel::Compressed => "compressed",
    };

    let entry = serde_json::json!({
        "ts": ts,
        "agent": agent,
        "file": file_name,
        "format": fmt_str,
        "fidelity": fid_str,
        "input_tok": input_tok,
        "output_tok": output_tok,
        "reduction_pct": (reduction * 10.0).round() / 10.0,
        "saved": input_tok.saturating_sub(output_tok),
    });

    let log_path = stats_dir.join(format!("{date_str}.jsonl"));
    // POSIX guarantees atomic append for writes under PIPE_BUF (4096 bytes).
    // A single JSONL stats line is well under this limit.
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    writeln!(file, "{entry}")?;
    Ok(())
}

/// Convert days since Unix epoch to (year, month, day).
fn epoch_days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Civil calendar algorithm from Howard Hinnant
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::{StatsRow, aggregate_lines, epoch_days_to_ymd, format_num};

    #[test]
    fn day_0_unix_epoch() {
        assert_eq!(epoch_days_to_ymd(0), (1970, 1, 1));
    }

    #[test]
    fn day_1() {
        assert_eq!(epoch_days_to_ymd(1), (1970, 1, 2));
    }

    #[test]
    fn leap_year_boundary_2000_03_01() {
        assert_eq!(epoch_days_to_ymd(11017), (2000, 3, 1));
    }

    #[test]
    fn leap_day_2024_02_29() {
        assert_eq!(epoch_days_to_ymd(19782), (2024, 2, 29));
    }

    #[test]
    fn today_2026_04_13() {
        assert_eq!(epoch_days_to_ymd(20556), (2026, 4, 13));
    }

    #[test]
    fn non_leap_century_2100_01_01() {
        assert_eq!(epoch_days_to_ymd(47482), (2100, 1, 1));
    }

    // ── Stats aggregation tests ──────────────────────────────────────────────

    #[test]
    fn aggregate_empty_input() {
        let rows = aggregate_lines(&[], None);
        assert!(rows.is_empty());
    }

    #[test]
    fn aggregate_single_line() {
        let line = r#"{"ts":"2026-04-13T07:12:31Z","agent":"claude","file":"lib.rs","format":"markdown","fidelity":"semantic","input_tok":2993,"output_tok":2749,"reduction_pct":8.2,"saved":244}"#;
        let rows = aggregate_lines(&[line], None);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].date, "2026-04-13");
        assert_eq!(rows[0].agent, "claude");
        assert_eq!(rows[0].calls, 1);
        assert_eq!(rows[0].input_tok, 2993);
        assert_eq!(rows[0].output_tok, 2749);
        assert_eq!(rows[0].saved, 244);
    }

    #[test]
    fn aggregate_groups_by_date_and_agent() {
        let lines = [
            r#"{"ts":"2026-04-13T07:00:00Z","agent":"claude","file":"a.rs","format":"markdown","fidelity":"semantic","input_tok":1000,"output_tok":800,"reduction_pct":20.0,"saved":200}"#,
            r#"{"ts":"2026-04-13T08:00:00Z","agent":"claude","file":"b.rs","format":"markdown","fidelity":"semantic","input_tok":500,"output_tok":400,"reduction_pct":20.0,"saved":100}"#,
            r#"{"ts":"2026-04-13T09:00:00Z","agent":"gemini","file":"c.rs","format":"markdown","fidelity":"semantic","input_tok":2000,"output_tok":1500,"reduction_pct":25.0,"saved":500}"#,
        ];
        let borrowed: Vec<&str> = lines.to_vec();
        let rows = aggregate_lines(&borrowed, None);
        assert_eq!(rows.len(), 2);

        let claude = rows.iter().find(|r| r.agent == "claude").unwrap();
        assert_eq!(claude.calls, 2);
        assert_eq!(claude.input_tok, 1500);
        assert_eq!(claude.output_tok, 1200);
        assert_eq!(claude.saved, 300);

        let gemini = rows.iter().find(|r| r.agent == "gemini").unwrap();
        assert_eq!(gemini.calls, 1);
        assert_eq!(gemini.input_tok, 2000);
    }

    #[test]
    fn aggregate_agent_filter() {
        let lines = [
            r#"{"ts":"2026-04-13T07:00:00Z","agent":"claude","file":"a.rs","format":"markdown","fidelity":"semantic","input_tok":1000,"output_tok":800,"reduction_pct":20.0,"saved":200}"#,
            r#"{"ts":"2026-04-13T08:00:00Z","agent":"gemini","file":"b.rs","format":"markdown","fidelity":"semantic","input_tok":500,"output_tok":400,"reduction_pct":20.0,"saved":100}"#,
        ];
        let borrowed: Vec<&str> = lines.to_vec();
        let rows = aggregate_lines(&borrowed, Some("claude"));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].agent, "claude");
    }

    #[test]
    fn aggregate_skips_malformed_lines() {
        let lines = [
            "not json at all",
            r#"{"ts":"2026-04-13T07:00:00Z","agent":"claude","file":"a.rs","format":"markdown","fidelity":"semantic","input_tok":1000,"output_tok":800,"reduction_pct":20.0,"saved":200}"#,
            r#"{"broken":"#,
        ];
        let borrowed: Vec<&str> = lines.to_vec();
        let rows = aggregate_lines(&borrowed, None);
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn aggregate_groups_across_two_dates() {
        let lines = [
            r#"{"ts":"2026-04-12T07:00:00Z","agent":"claude","file":"a.rs","format":"markdown","fidelity":"semantic","input_tok":1000,"output_tok":800,"reduction_pct":20.0,"saved":200}"#,
            r#"{"ts":"2026-04-13T07:00:00Z","agent":"claude","file":"b.rs","format":"markdown","fidelity":"semantic","input_tok":2000,"output_tok":1600,"reduction_pct":20.0,"saved":400}"#,
        ];
        let borrowed: Vec<&str> = lines.to_vec();
        let rows = aggregate_lines(&borrowed, None);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn reduction_pct_calculation() {
        let row = StatsRow {
            date: "2026-04-13".to_string(),
            agent: "claude".to_string(),
            calls: 1,
            input_tok: 1000,
            output_tok: 730,
            saved: 270,
        };
        let pct = row.reduction_pct();
        assert!((pct - 27.0).abs() < 0.01, "expected 27.0%, got {pct}");
    }

    #[test]
    fn reduction_pct_zero_input() {
        let row = StatsRow {
            date: "2026-04-13".to_string(),
            agent: "claude".to_string(),
            calls: 0,
            input_tok: 0,
            output_tok: 0,
            saved: 0,
        };
        assert_eq!(row.reduction_pct(), 0.0);
    }

    #[test]
    fn format_num_thousands() {
        assert_eq!(format_num(0), "0");
        assert_eq!(format_num(999), "999");
        assert_eq!(format_num(1000), "1 000");
        assert_eq!(format_num(14965), "14 965");
        assert_eq!(format_num(1_000_000), "1 000 000");
    }
}
