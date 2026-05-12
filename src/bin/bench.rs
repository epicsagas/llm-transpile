//! bench — llm-transpile file benchmark runner & HTML report generator
//!
//! ## Subcommands
//!
//! ```text
//! bench run    Benchmark eval/dataset files → dated JSONL log
//! bench report Aggregate JSONL logs → bench-report.html
//! ```
//!
//! ### Log location
//! `~/.agents/transpile/bench/YYYY-MM-DD_HH-MM-SS.jsonl`
//!
//! ### JSONL record schema
//! ```json
//! {
//!   "ts":            "2026-05-12T09:00:00Z",
//!   "run_id":        "2026-05-12_09-00-00",
//!   "file":          "hub-docs_api.md",
//!   "format":        "markdown",
//!   "input_bytes":   12345,
//!   "input_tok":     2048,
//!   "semantic_tok":  1400,
//!   "compressed_tok":1200,
//!   "lossless_tok":  2060,
//!   "sem_pct":       31.6,
//!   "cmp_pct":       41.4,
//!   "los_pct":       -0.6,
//!   "semantic_us":   180,
//!   "compressed_us": 210,
//!   "tok_per_ms":    11378.0,
//!   "word_coverage": 99.1
//! }
//! ```

use clap::{Parser, Subcommand};
use llm_transpile::{FidelityLevel, InputFormat, token_count, transpile};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, BufRead, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::time::Instant;

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "bench",
    about = "llm-transpile file benchmark runner & HTML report generator",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Benchmark eval/dataset files and write a dated JSONL log
    Run {
        /// Root directory of eval datasets (default: eval/dataset)
        #[arg(long, default_value = "eval/dataset")]
        dataset: String,

        /// Log output directory (default: ~/.agents/transpile/bench)
        #[arg(long)]
        log_dir: Option<String>,

        /// Also generate HTML report after run
        #[arg(long, short = 'R')]
        report: bool,

        /// Path for the HTML report (default: bench-report.html)
        #[arg(long, default_value = "bench-report.html")]
        report_out: String,
    },
    /// Aggregate all JSONL logs and generate an HTML report
    Report {
        /// Log directory (default: ~/.agents/transpile/bench)
        #[arg(long)]
        log_dir: Option<String>,

        /// Output HTML path
        #[arg(long, short = 'o', default_value = "bench-report.html")]
        out: String,
    },
}

// ── Data model ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BenchRecord {
    ts: String,
    run_id: String,
    file: String,
    format: String,
    input_bytes: usize,
    input_tok: usize,
    semantic_tok: usize,
    compressed_tok: usize,
    lossless_tok: usize,
    sem_pct: f64,
    cmp_pct: f64,
    los_pct: f64,
    semantic_us: u128,
    compressed_us: u128,
    tok_per_ms: f64,
    word_coverage: f64,
}

// ── Path helpers ──────────────────────────────────────────────────────────────

fn default_log_dir() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".agents/transpile/bench")
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

// ── Time helpers (no chrono) ──────────────────────────────────────────────────

fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    secs_to_iso(secs)
}

fn secs_to_iso(secs: u64) -> String {
    let sec = secs % 60;
    let min = (secs / 60) % 60;
    let hour = (secs / 3600) % 24;
    let (year, month, day) = days_to_ymd(secs / 86400);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

fn days_to_ymd(mut days: u64) -> (u32, u32, u32) {
    let mut year = 1970u32;
    loop {
        let dy = if is_leap(year) { 366 } else { 365 };
        if days < dy {
            break;
        }
        days -= dy;
        year += 1;
    }
    let months = if is_leap(year) {
        [31u32, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31u32, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u32;
    for dm in months {
        if days < dm as u64 {
            break;
        }
        days -= dm as u64;
        month += 1;
    }
    (year, month, days as u32 + 1)
}

fn is_leap(y: u32) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

/// "2026-05-12T01:10:01Z" → "2026-05-12_01-10-01"
fn iso_to_run_id(iso: &str) -> String {
    iso.replace('T', "_")
        .replace(':', "-")
        .trim_end_matches('Z')
        .to_string()
}

// ── Metrics ───────────────────────────────────────────────────────────────────

fn pct_reduction(output: usize, input: usize) -> f64 {
    if input == 0 {
        return 0.0;
    }
    100.0 - (output as f64 / input as f64 * 100.0)
}

/// Lossless word coverage: % of unique content words (>5 chars, all-alpha)
/// from source that appear in the lossless output.
fn word_coverage(source: &str, lossless_out: &str) -> f64 {
    use std::collections::HashSet;
    let words: HashSet<&str> = source
        .split_whitespace()
        .filter(|w| w.len() > 5 && w.chars().all(|c| c.is_alphabetic()))
        .collect();
    if words.is_empty() {
        return 100.0;
    }
    let matched = words.iter().filter(|w| lossless_out.contains(*w)).count();
    matched as f64 / words.len() as f64 * 100.0
}

// ── File collection ───────────────────────────────────────────────────────────

fn collect_files(dir: &str, ext: &str) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return vec![];
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some(ext) {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    files.sort();
    files
}

// ── Single-file benchmark ─────────────────────────────────────────────────────

fn bench_file(path: &Path, fmt: InputFormat, ts: &str, run_id: &str) -> Option<BenchRecord> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("WARN skip {}: {e}", path.display());
            return None; // R8: skip on error, no panic
        }
    };

    let input_tok = token_count(&content);
    let input_bytes = content.len();

    // 3 runs, take median (R1)
    let timed = |fidelity: FidelityLevel, budget: Option<usize>| -> Option<(String, u128)> {
        let mut timings = [0u128; 3];
        let mut out = String::new();
        for t in &mut timings {
            let t0 = Instant::now();
            out = match transpile(&content, fmt, fidelity, budget) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("WARN transpile failed {}: {e}", path.display());
                    return None; // R8
                }
            };
            *t = t0.elapsed().as_micros();
        }
        timings.sort_unstable();
        Some((out, timings[1]))
    };

    let (sem, semantic_us) = timed(FidelityLevel::Semantic, Some(4096))?;
    let (cmp, compressed_us) = timed(FidelityLevel::Compressed, Some(2048))?;
    let (los, _) = timed(FidelityLevel::Lossless, None)?;

    let semantic_tok = token_count(&sem);
    let compressed_tok = token_count(&cmp);
    let lossless_tok = token_count(&los);
    let coverage = word_coverage(&content, &los);
    let tok_per_ms = if semantic_us > 0 {
        input_tok as f64 / semantic_us as f64 * 1000.0
    } else {
        input_tok as f64 * 1000.0
    };

    let fname = path.file_name()?.to_string_lossy().into_owned();
    let format_str = match fmt {
        InputFormat::Markdown => "markdown",
        InputFormat::Html => "html",
        InputFormat::PlainText => "plaintext",
    };

    Some(BenchRecord {
        ts: ts.to_string(),
        run_id: run_id.to_string(),
        file: fname,
        format: format_str.to_string(),
        input_bytes,
        input_tok,
        semantic_tok,
        compressed_tok,
        lossless_tok,
        sem_pct: pct_reduction(semantic_tok, input_tok),
        cmp_pct: pct_reduction(compressed_tok, input_tok),
        los_pct: pct_reduction(lossless_tok, input_tok),
        semantic_us,
        compressed_us,
        tok_per_ms,
        word_coverage: coverage,
    })
}

// ── run subcommand ────────────────────────────────────────────────────────────

fn cmd_run(dataset: &str, log_dir_opt: Option<String>, report: bool, report_out: &str) {
    let ts = now_iso();
    let run_id = iso_to_run_id(&ts);

    let log_dir = log_dir_opt
        .map(PathBuf::from)
        .unwrap_or_else(default_log_dir);
    // R3: auto-create log directory
    if let Err(e) = fs::create_dir_all(&log_dir) {
        eprintln!(
            "ERROR: cannot create log directory {}: {e}",
            log_dir.display()
        );
        std::process::exit(1);
    }

    let log_path = log_dir.join(format!("{run_id}.jsonl"));
    let mut log_file = match fs::File::create(&log_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("ERROR: cannot create log file {}: {e}", log_path.display());
            std::process::exit(1);
        }
    };

    println!("▶ bench run  [{ts}]");
    println!("  dataset : {dataset}");
    println!("  log     : {}", log_path.display());
    println!();

    let sections: &[(&str, &str, InputFormat)] = &[
        ("policy", "md", InputFormat::Markdown),
        ("hf", "md", InputFormat::Markdown),
        ("multilingual", "md", InputFormat::Markdown),
        ("html", "html", InputFormat::Html),
        ("plaintext", "txt", InputFormat::PlainText),
    ];

    let mut all: Vec<BenchRecord> = Vec::new();

    for (sub, ext, fmt) in sections {
        let dir = format!("{dataset}/{sub}");
        let files = collect_files(&dir, ext);
        if files.is_empty() {
            continue;
        }

        println!("  ▸ {sub}/  ({} files)", files.len());
        print_table_header();

        for path in &files {
            if let Some(rec) = bench_file(path, *fmt, &ts, &run_id) {
                print_table_row(&rec);
                let line = serde_json::to_string(&rec).expect("serialize");
                writeln!(log_file, "{line}").expect("write log");
                all.push(rec);
            }
        }
        println!();
    }

    print_grand_summary(&all);

    if report {
        println!("\n  Generating report → {report_out}");
        let records = load_all_logs(&log_dir);
        generate_html(&records, report_out);
        println!("  ✓ {report_out}");
    }
}

// ── report subcommand ─────────────────────────────────────────────────────────

fn cmd_report(log_dir_opt: Option<String>, out: &str) {
    let log_dir = log_dir_opt
        .map(PathBuf::from)
        .unwrap_or_else(default_log_dir);
    println!("▶ bench report");
    println!("  log dir : {}", log_dir.display());

    let records = load_all_logs(&log_dir);
    // R7: empty log dir → error
    if records.is_empty() {
        eprintln!(
            "ERROR: no JSONL logs found in {}. Run `bench run` first.",
            log_dir.display()
        );
        std::process::exit(1);
    }

    println!("  records : {}", records.len());
    generate_html(&records, out);
    println!("  ✓ {out}");
}

// ── Log loader ────────────────────────────────────────────────────────────────

fn load_all_logs(log_dir: &Path) -> Vec<BenchRecord> {
    let Ok(entries) = fs::read_dir(log_dir) else {
        return vec![];
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("jsonl") {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    paths.sort();

    let mut all: Vec<BenchRecord> = Vec::new();
    for path in paths {
        let Ok(f) = fs::File::open(&path) else {
            continue;
        };
        for line in io::BufReader::new(f).lines().map_while(|r| r.ok()) {
            if let Ok(rec) = serde_json::from_str::<BenchRecord>(&line) {
                all.push(rec);
            }
        }
    }
    all
}

// ── Terminal table helpers ────────────────────────────────────────────────────

fn print_table_header() {
    println!(
        "    {:<36} {:>4} {:>6} {:>7} {:>7} {:>7} {:>7} {:>9}",
        "file", "fmt", "in_tok", "sem%", "cmp%", "sem_ms", "cmp_ms", "tok/ms"
    );
    println!("    {}", "─".repeat(90));
}

fn print_table_row(r: &BenchRecord) {
    let sem_ms = r.semantic_us as f64 / 1000.0;
    let cmp_ms = r.compressed_us as f64 / 1000.0;
    println!(
        "    {:<36} {:>4} {:>6} {:>7.1} {:>7.1} {:>7.1} {:>7.1} {:>9.0}",
        trunc(&r.file, 36),
        &r.format[..3],
        r.input_tok,
        r.sem_pct,
        r.cmp_pct,
        sem_ms,
        cmp_ms,
        r.tok_per_ms,
    );
}

fn print_grand_summary(all: &[BenchRecord]) {
    if all.is_empty() {
        return;
    }
    let total_in: usize = all.iter().map(|r| r.input_tok).sum();
    let total_sem: usize = all.iter().map(|r| r.semantic_tok).sum();
    let total_cmp: usize = all.iter().map(|r| r.compressed_tok).sum();
    let total_us: u128 = all.iter().map(|r| r.semantic_us).sum();
    let avg_cov: f64 = all.iter().map(|r| r.word_coverage).sum::<f64>() / all.len() as f64;
    let tok_ms = if total_us > 0 {
        total_in as f64 / total_us as f64 * 1000.0
    } else {
        0.0
    };
    println!("  ── Grand summary ({} files) ──", all.len());
    println!("     input tokens : {total_in}");
    println!(
        "     sem reduction: {:.1}%",
        pct_reduction(total_sem, total_in)
    );
    println!(
        "     cmp reduction: {:.1}%",
        pct_reduction(total_cmp, total_in)
    );
    println!("     word coverage: {avg_cov:.1}% avg");
    println!("     throughput   : {tok_ms:.0} tok/ms");
}

fn trunc(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}

// ── HTML report generator ─────────────────────────────────────────────────────

/// Escape HTML special characters to prevent XSS (R6).
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn pct_class(pct: f64) -> &'static str {
    if pct >= 20.0 {
        "good"
    } else if pct >= 5.0 {
        "ok"
    } else {
        "low"
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (p / 100.0 * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn generate_html(records: &[BenchRecord], out_path: &str) {
    // Collect unique run IDs in sorted order
    let mut run_ids: Vec<String> = {
        let mut set = std::collections::BTreeSet::new();
        for r in records {
            set.insert(r.run_id.clone());
        }
        set.into_iter().collect()
    };
    run_ids.sort();

    // Per-run aggregates
    struct RunSummary {
        run_id: String,
        ts: String,
        files: usize,
        sem_pct: f64,
        cmp_pct: f64,
        tok_ms: f64,
        coverage: f64,
    }
    let summaries: Vec<RunSummary> = run_ids
        .iter()
        .map(|rid| {
            let recs: Vec<&BenchRecord> = records.iter().filter(|r| &r.run_id == rid).collect();
            let ti: usize = recs.iter().map(|r| r.input_tok).sum();
            let ts_val: usize = recs.iter().map(|r| r.semantic_tok).sum();
            let tc: usize = recs.iter().map(|r| r.compressed_tok).sum();
            let avg_cov = if recs.is_empty() {
                0.0
            } else {
                recs.iter().map(|r| r.word_coverage).sum::<f64>() / recs.len() as f64
            };
            let avg_tok = if recs.is_empty() {
                0.0
            } else {
                recs.iter().map(|r| r.tok_per_ms).sum::<f64>() / recs.len() as f64
            };
            let ts_str = recs.first().map(|r| r.ts.clone()).unwrap_or_default();
            RunSummary {
                run_id: rid.clone(),
                ts: ts_str,
                files: recs.len(),
                sem_pct: pct_reduction(ts_val, ti),
                cmp_pct: pct_reduction(tc, ti),
                tok_ms: avg_tok,
                coverage: avg_cov,
            }
        })
        .collect();

    // Chart.js data
    let labels = serde_json::to_string(
        &summaries
            .iter()
            .map(|s| s.run_id.clone())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let sem_data = serde_json::to_string(
        &summaries
            .iter()
            .map(|s| round2(s.sem_pct))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let cmp_data = serde_json::to_string(
        &summaries
            .iter()
            .map(|s| round2(s.cmp_pct))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let tok_data = serde_json::to_string(
        &summaries
            .iter()
            .map(|s| round2(s.tok_ms))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let cov_data = serde_json::to_string(
        &summaries
            .iter()
            .map(|s| round2(s.coverage))
            .collect::<Vec<_>>(),
    )
    .unwrap();

    let scatter_data = serde_json::to_string(
        &records
            .iter()
            .map(|r| {
                serde_json::json!({
                    "x": round2(r.sem_pct), "y": round2(r.tok_per_ms),
                    "file": r.file, "fmt": r.format, "run": r.run_id,
                })
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();

    let fmt_list = ["markdown", "html", "plaintext"];
    let box_data = serde_json::to_string(
        &fmt_list
            .iter()
            .map(|fmt| {
                let mut vals: Vec<f64> = records
                    .iter()
                    .filter(|r| r.format == *fmt)
                    .map(|r| r.sem_pct)
                    .collect();
                vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
                if vals.is_empty() {
                    return serde_json::json!({"fmt": fmt, "min":0,"q1":0,"med":0,"q3":0,"max":0});
                }
                serde_json::json!({
                    "fmt": fmt,
                    "min": round2(vals[0]),
                    "q1":  round2(percentile(&vals, 25.0)),
                    "med": round2(percentile(&vals, 50.0)),
                    "q3":  round2(percentile(&vals, 75.0)),
                    "max": round2(*vals.last().unwrap()),
                })
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();

    // Summary totals
    let total_in: usize = records.iter().map(|r| r.input_tok).sum();
    let total_sem: usize = records.iter().map(|r| r.semantic_tok).sum();
    let total_cmp: usize = records.iter().map(|r| r.compressed_tok).sum();
    let avg_cov: f64 = if records.is_empty() {
        0.0
    } else {
        records.iter().map(|r| r.word_coverage).sum::<f64>() / records.len() as f64
    };
    let avg_tok_ms: f64 = if records.is_empty() {
        0.0
    } else {
        records.iter().map(|r| r.tok_per_ms).sum::<f64>() / records.len() as f64
    };

    // Table rows — all escaped (R6)
    let table_rows: String = records
        .iter()
        .map(|r| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td class='n'>{}</td>\
         <td class='n {}'>{:.1}%</td><td class='n {}'>{:.1}%</td>\
         <td class='n'>{:.1}</td><td class='n'>{:.1}</td>\
         <td class='n'>{:.0}</td><td class='n'>{:.1}%</td></tr>",
                esc(&r.run_id),
                esc(&r.file),
                esc(&r.format),
                r.input_tok,
                pct_class(r.sem_pct),
                r.sem_pct,
                pct_class(r.cmp_pct),
                r.cmp_pct,
                r.semantic_us as f64 / 1000.0,
                r.compressed_us as f64 / 1000.0,
                r.tok_per_ms,
                r.word_coverage,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let run_rows: String = summaries
        .iter()
        .map(|s| {
            format!(
                "<tr><td>{}</td><td>{}</td><td class='n'>{}</td>\
         <td class='n {}'>{:.1}%</td><td class='n {}'>{:.1}%</td>\
         <td class='n'>{:.0}</td><td class='n'>{:.1}%</td></tr>",
                esc(&s.run_id),
                esc(&s.ts),
                s.files,
                pct_class(s.sem_pct),
                s.sem_pct,
                pct_class(s.cmp_pct),
                s.cmp_pct,
                s.tok_ms,
                s.coverage,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let run_options: String = run_ids
        .iter()
        .map(|r| format!("<option>{}</option>", esc(r)))
        .collect::<Vec<_>>()
        .join("");

    let html = format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>bench — llm-transpile report</title>
<script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.2/dist/chart.umd.min.js"></script>
<style>
:root{{--bg:#0f1117;--surf:#1a1d27;--bdr:#2e3147;--txt:#e2e8f0;--mut:#8892a4;
  --acc:#6366f1;--grn:#22c55e;--ylw:#eab308;--red:#ef4444;}}
*{{box-sizing:border-box;margin:0;padding:0;}}
body{{background:var(--bg);color:var(--txt);font-family:system-ui,sans-serif;font-size:14px;}}
header{{padding:20px 28px;border-bottom:1px solid var(--bdr);display:flex;align-items:center;gap:12px;}}
header h1{{font-size:20px;font-weight:700;}}
.badge{{background:var(--acc);color:#fff;font-size:11px;padding:2px 8px;border-radius:99px;font-weight:600;}}
.wrap{{max-width:1400px;margin:0 auto;padding:20px 28px;}}
.cards{{display:grid;grid-template-columns:repeat(auto-fit,minmax(160px,1fr));gap:14px;margin-bottom:28px;}}
.card{{background:var(--surf);border:1px solid var(--bdr);border-radius:10px;padding:18px;}}
.card .lbl{{font-size:11px;color:var(--mut);text-transform:uppercase;letter-spacing:.5px;margin-bottom:6px;}}
.card .val{{font-size:26px;font-weight:700;}}
.card .sub{{font-size:11px;color:var(--mut);margin-top:3px;}}
.charts{{display:grid;grid-template-columns:1fr 1fr;gap:18px;margin-bottom:28px;}}
.cbox{{background:var(--surf);border:1px solid var(--bdr);border-radius:10px;padding:18px;}}
.cbox h3{{font-size:11px;font-weight:600;color:var(--mut);text-transform:uppercase;letter-spacing:.5px;margin-bottom:14px;}}
.cbox canvas{{max-height:240px;}}
section{{margin-bottom:28px;}}
section h2{{font-size:15px;font-weight:600;margin-bottom:12px;padding-bottom:7px;border-bottom:1px solid var(--bdr);}}
table{{width:100%;border-collapse:collapse;background:var(--surf);border-radius:10px;overflow:hidden;border:1px solid var(--bdr);}}
thead tr{{background:#1e2235;}}
th{{padding:9px 12px;text-align:left;font-size:11px;font-weight:600;color:var(--mut);text-transform:uppercase;letter-spacing:.5px;white-space:nowrap;}}
td{{padding:8px 12px;border-top:1px solid var(--bdr);font-size:13px;white-space:nowrap;}}
td.n{{text-align:right;font-variant-numeric:tabular-nums;}}
tr:hover td{{background:rgba(255,255,255,.02);}}
.good{{color:var(--grn);}} .ok{{color:var(--ylw);}} .low{{color:var(--red);}}
.frow{{display:flex;gap:8px;margin-bottom:12px;flex-wrap:wrap;}}
input[type=text],select{{background:var(--surf);border:1px solid var(--bdr);border-radius:7px;
  padding:5px 10px;color:var(--txt);font-size:13px;outline:none;}}
input[type=text]:focus,select:focus{{border-color:var(--acc);}}
button{{background:var(--acc);border:none;color:#fff;border-radius:7px;
  padding:5px 12px;font-size:13px;cursor:pointer;font-weight:600;}}
button:hover{{opacity:.85;}}
@media(max-width:900px){{.charts{{grid-template-columns:1fr;}}}}
</style>
</head>
<body>
<header>
  <h1>bench</h1>
  <span class="badge">llm-transpile</span>
  <span style="color:var(--mut);margin-left:auto;font-size:12px">{runs} runs · {total_files} records</span>
</header>
<div class="wrap">

<div class="cards">
  <div class="card">
    <div class="lbl">Semantic reduction</div>
    <div class="val" style="color:var(--grn)">{sem_pct:.1}%</div>
    <div class="sub">avg all files</div>
  </div>
  <div class="card">
    <div class="lbl">Compressed reduction</div>
    <div class="val" style="color:var(--grn)">{cmp_pct:.1}%</div>
    <div class="sub">avg all files</div>
  </div>
  <div class="card">
    <div class="lbl">Throughput</div>
    <div class="val" style="color:var(--acc)">{avg_tok_ms:.0}</div>
    <div class="sub">tok/ms avg</div>
  </div>
  <div class="card">
    <div class="lbl">Word coverage</div>
    <div class="val" style="color:var(--ylw)">{avg_cov:.1}%</div>
    <div class="sub">lossless avg</div>
  </div>
  <div class="card">
    <div class="lbl">Total input tokens</div>
    <div class="val">{total_in}</div>
    <div class="sub">across all runs</div>
  </div>
  <div class="card">
    <div class="lbl">Runs</div>
    <div class="val">{runs}</div>
    <div class="sub">{total_files} measurements</div>
  </div>
</div>

<div class="charts">
  <div class="cbox">
    <h3>Token reduction over time (%)</h3>
    <canvas id="trendChart"></canvas>
  </div>
  <div class="cbox">
    <h3>Throughput over time (tok/ms)</h3>
    <canvas id="thruChart"></canvas>
  </div>
  <div class="cbox">
    <h3>Sem% vs Throughput — scatter per file</h3>
    <canvas id="scatterChart"></canvas>
  </div>
  <div class="cbox">
    <h3>Reduction distribution by format (min/Q1/med/Q3/max)</h3>
    <canvas id="boxChart"></canvas>
  </div>
</div>

<section>
  <h2>Runs</h2>
  <table>
    <thead><tr><th>run id</th><th>timestamp</th><th>files</th>
      <th>sem%</th><th>cmp%</th><th>tok/ms</th><th>coverage</th></tr></thead>
    <tbody>{run_rows}</tbody>
  </table>
</section>

<section>
  <h2>All records</h2>
  <div class="frow">
    <input type="text" id="ftxt" placeholder="Filter file…" oninput="filter()">
    <select id="ffmt" onchange="filter()">
      <option value="">All formats</option>
      <option>markdown</option><option>html</option><option>plaintext</option>
    </select>
    <select id="frun" onchange="filter()">
      <option value="">All runs</option>
      {run_options}
    </select>
    <button onclick="exportCsv()">⬇ CSV</button>
  </div>
  <table id="tbl">
    <thead><tr><th>run</th><th>file</th><th>format</th><th>in tok</th>
      <th>sem%</th><th>cmp%</th><th>sem ms</th><th>cmp ms</th>
      <th>tok/ms</th><th>coverage</th></tr></thead>
    <tbody id="tbody">{table_rows}</tbody>
  </table>
</section>
</div>

<script>
const G = {{color:'rgba(255,255,255,.05)'}}, F = {{color:'#8892a4'}};
const LABELS={labels}, SEM={sem_data}, CMP={cmp_data}, TOK={tok_data}, COV={cov_data};
const SCATTER={scatter_data}, BOX={box_data};

// Trend
new Chart(document.getElementById('trendChart'),{{
  type:'line',
  data:{{labels:LABELS,datasets:[
    {{label:'Semantic%',data:SEM,borderColor:'#22c55e',backgroundColor:'rgba(34,197,94,.08)',tension:.3,pointRadius:4}},
    {{label:'Compressed%',data:CMP,borderColor:'#6366f1',backgroundColor:'rgba(99,102,241,.08)',tension:.3,pointRadius:4}},
  ]}},
  options:{{plugins:{{legend:{{labels:{{color:'#e2e8f0'}}}}}},
    scales:{{x:{{ticks:F,grid:G}},y:{{ticks:F,grid:G,title:{{display:true,text:'% reduction',color:'#8892a4'}}}}}}}}
}});

// Throughput
new Chart(document.getElementById('thruChart'),{{
  type:'bar',
  data:{{labels:LABELS,datasets:[{{label:'tok/ms',data:TOK,backgroundColor:'rgba(99,102,241,.7)',borderRadius:4}}]}},
  options:{{plugins:{{legend:{{labels:{{color:'#e2e8f0'}}}}}},
    scales:{{x:{{ticks:F,grid:G}},y:{{ticks:F,grid:G,title:{{display:true,text:'tok/ms',color:'#8892a4'}}}}}}}}
}});

// Scatter
const FMT_COL={{'markdown':'#6366f1','html':'#22c55e','plaintext':'#eab308'}};
const grps={{}};
for(const p of SCATTER){{if(!grps[p.fmt])grps[p.fmt]=[];grps[p.fmt].push({{x:p.x,y:p.y,file:p.file,run:p.run}});}}
new Chart(document.getElementById('scatterChart'),{{
  type:'scatter',
  data:{{datasets:Object.entries(grps).map(([fmt,pts])=>({{'label':fmt,'data':pts,'backgroundColor':(FMT_COL[fmt]||'#fff')+'cc','pointRadius':5}})) }},
  options:{{
    plugins:{{legend:{{labels:{{color:'#e2e8f0'}}}},
      tooltip:{{callbacks:{{label:c=>`${{c.raw.file}} (${{c.raw.run}}): ${{c.raw.x}}% / ${{c.raw.y.toFixed(0)}} tok/ms`}}}}}},
    scales:{{x:{{ticks:F,grid:G,title:{{display:true,text:'Semantic reduction %',color:'#8892a4'}}}},
             y:{{ticks:F,grid:G,title:{{display:true,text:'Throughput (tok/ms)',color:'#8892a4'}}}}}}}}
}});

// Box (floating bars)
new Chart(document.getElementById('boxChart'),{{
  type:'bar',
  data:{{labels:BOX.map(b=>b.fmt),datasets:[
    {{label:'min–Q1',data:BOX.map(b=>[b.min,b.q1]),backgroundColor:'rgba(99,102,241,.3)',borderSkipped:false}},
    {{label:'Q1–med',data:BOX.map(b=>[b.q1,b.med]),backgroundColor:'rgba(99,102,241,.6)',borderSkipped:false}},
    {{label:'med–Q3',data:BOX.map(b=>[b.med,b.q3]),backgroundColor:'rgba(34,197,94,.6)',borderSkipped:false}},
    {{label:'Q3–max',data:BOX.map(b=>[b.q3,b.max]),backgroundColor:'rgba(34,197,94,.3)',borderSkipped:false}},
  ]}},
  options:{{plugins:{{legend:{{labels:{{color:'#e2e8f0'}}}}}},
    scales:{{x:{{ticks:F,grid:G}},y:{{ticks:F,grid:G,title:{{display:true,text:'Semantic reduction %',color:'#8892a4'}}}}}}}}
}});

// Filter
function filter(){{
  const txt=document.getElementById('ftxt').value.toLowerCase();
  const fmt=document.getElementById('ffmt').value;
  const run=document.getElementById('frun').value;
  document.querySelectorAll('#tbody tr').forEach(row=>{{
    const c=row.querySelectorAll('td');
    row.style.display=(
      (!txt||c[1].textContent.toLowerCase().includes(txt))&&
      (!fmt||c[2].textContent===fmt)&&
      (!run||c[0].textContent===run)
    )?'':'none';
  }});
}}

// CSV export
function exportCsv(){{
  const hdr=['run','file','format','in_tok','sem%','cmp%','sem_ms','cmp_ms','tok_ms','coverage'];
  const rows=[hdr];
  document.querySelectorAll('#tbody tr').forEach(row=>{{
    if(row.style.display==='none')return;
    rows.push([...row.querySelectorAll('td')].map(td=>td.textContent));
  }});
  const a=document.createElement('a');
  a.href=URL.createObjectURL(new Blob([rows.map(r=>r.join(',')).join('\n')],{{type:'text/csv'}}));
  a.download='bench.csv';a.click();
}}
</script>
</body>
</html>
"##,
        runs = summaries.len(),
        total_files = records.len(),
        sem_pct = pct_reduction(total_sem, total_in),
        cmp_pct = pct_reduction(total_cmp, total_in),
        avg_tok_ms = avg_tok_ms,
        avg_cov = avg_cov,
        total_in = total_in,
        run_rows = run_rows,
        run_options = run_options,
        table_rows = table_rows,
        labels = labels,
        sem_data = sem_data,
        cmp_data = cmp_data,
        tok_data = tok_data,
        cov_data = cov_data,
        scatter_data = scatter_data,
        box_data = box_data,
    );

    if let Err(e) = fs::write(out_path, &html) {
        eprintln!("ERROR: cannot write HTML to {out_path}: {e}");
        std::process::exit(1);
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Run {
            dataset,
            log_dir,
            report,
            report_out,
        } => cmd_run(&dataset, log_dir, report, &report_out),
        Cmd::Report { log_dir, out } => cmd_report(log_dir, &out),
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pct_reduction_zero_input_is_zero() {
        assert_eq!(pct_reduction(100, 0), 0.0);
    }

    #[test]
    fn pct_reduction_half() {
        assert!((pct_reduction(50, 100) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn esc_prevents_xss() {
        let s = esc("<img onerror=alert(1)>.md");
        assert!(!s.contains('<'));
        assert!(!s.contains('>'));
        assert!(s.contains("&lt;"));
    }

    #[test]
    fn iso_to_run_id_format() {
        let id = iso_to_run_id("2026-05-12T01:10:01Z");
        assert_eq!(id, "2026-05-12_01-10-01");
    }

    #[test]
    fn word_coverage_full() {
        assert_eq!(word_coverage("", "anything"), 100.0);
    }

    #[test]
    fn trunc_short_unchanged() {
        assert_eq!(trunc("hello", 10), "hello");
    }

    #[test]
    fn trunc_long_truncated() {
        let s = trunc("abcdefghijk", 5);
        assert!(s.len() <= 7); // 4 chars + '…' (3 UTF-8 bytes)
    }

    #[test]
    fn secs_to_iso_epoch() {
        assert_eq!(secs_to_iso(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn load_all_logs_missing_dir_returns_empty() {
        let result = load_all_logs(Path::new("/tmp/nonexistent_bench_dir_xyz"));
        assert!(result.is_empty());
    }
}
