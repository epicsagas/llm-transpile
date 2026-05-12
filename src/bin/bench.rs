//! bench — llm-transpile file benchmark runner & HTML report generator
//!
//! Used as `transpile bench run` / `transpile bench report`.

use llm_transpile::{FidelityLevel, InputFormat, token_count, transpile};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, BufRead, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::time::Instant;

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

fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return format!("{home}/{rest}");
    }
    path.to_string()
}

fn ensure_parent_dir(path: &str) -> std::io::Result<()> {
    let p = std::path::Path::new(path);
    if let Some(parent) = p.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
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

pub fn cmd_run(dataset: &str, log_dir_opt: Option<String>, report: bool, report_out: &str) -> i32 {
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
        return 1;
    }

    let log_path = log_dir.join(format!("{run_id}.jsonl"));
    let mut log_file = match fs::File::create(&log_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("ERROR: cannot create log file {}: {e}", log_path.display());
            return 1;
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

    // R7: no records produced → error exit
    if all.is_empty() {
        eprintln!("ERROR: no records produced. Check dataset path: {dataset}");
        return 1;
    }

    print_grand_summary(&all);

    if report {
        let report_out = expand_tilde(report_out);
        println!("\n  Generating report → {report_out}");
        if let Err(e) = ensure_parent_dir(&report_out) {
            eprintln!("ERROR: cannot create report directory: {e}");
            return 1;
        }
        let records = load_all_logs(&log_dir);
        generate_html(&records, &report_out);
        println!("  ✓ {report_out}");
    }

    0
}

// ── report subcommand ─────────────────────────────────────────────────────────

pub fn cmd_report(log_dir_opt: Option<String>, out: &str, no_open: bool) -> i32 {
    let log_dir = log_dir_opt
        .map(PathBuf::from)
        .unwrap_or_else(default_log_dir);
    println!("▶ bench report");
    println!("  log dir : {}", log_dir.display());

    let records = load_all_logs(&log_dir);
    if records.is_empty() {
        eprintln!(
            "ERROR: no JSONL logs found in {}. Run `transpile bench run` first.",
            log_dir.display()
        );
        return 1;
    }

    println!("  records : {}", records.len());
    let out = expand_tilde(out);
    if let Err(e) = ensure_parent_dir(&out) {
        eprintln!("ERROR: cannot create report directory: {e}");
        return 1;
    }
    generate_html(&records, &out);
    println!("  ✓ {out}");

    if !no_open {
        let _ = std::process::Command::new("open").arg(&out).spawn();
    }

    0
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
        r.format.get(..3).unwrap_or(&r.format),
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
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

/// Escape `</` so JSON strings embedded in `<script>` cannot prematurely
/// close the script block (JS spec allows `<\/` as valid escape).
fn js_safe(json: &str) -> String {
    json.replace("</", "<\\/")
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

    // Format distribution (pie chart)
    let fmt_list = ["markdown", "html", "plaintext"];
    let fmt_counts: Vec<usize> = fmt_list
        .iter()
        .map(|f| records.iter().filter(|r| r.format == *f).count())
        .collect();
    let fmt_count_data = serde_json::to_string(&fmt_counts).unwrap();

    // Input token size histogram (8 buckets)
    let tok_buckets = [128usize, 512, 1024, 2048, 4096, 8192, 32768, usize::MAX];
    let bucket_labels = ["<128", "<512", "<1K", "<2K", "<4K", "<8K", "<32K", "32K+"];
    let mut hist = vec![0usize; tok_buckets.len()];
    for r in records {
        let bucket = tok_buckets
            .iter()
            .position(|&cap| r.input_tok < cap)
            .unwrap_or(tok_buckets.len() - 1);
        hist[bucket] += 1;
    }
    let hist_labels = serde_json::to_string(&bucket_labels).unwrap();
    let hist_data = serde_json::to_string(&hist).unwrap();

    // Word coverage donut buckets
    let cov_buckets = [
        (
            "100%",
            records.iter().filter(|r| r.word_coverage >= 100.0).count(),
        ),
        (
            "≥95%",
            records
                .iter()
                .filter(|r| r.word_coverage >= 95.0 && r.word_coverage < 100.0)
                .count(),
        ),
        (
            "≥80%",
            records
                .iter()
                .filter(|r| r.word_coverage >= 80.0 && r.word_coverage < 95.0)
                .count(),
        ),
        (
            "<80%",
            records.iter().filter(|r| r.word_coverage < 80.0).count(),
        ),
    ];
    let cov_donut_labels =
        serde_json::to_string(&cov_buckets.iter().map(|(l, _)| *l).collect::<Vec<_>>()).unwrap();
    let cov_donut_data =
        serde_json::to_string(&cov_buckets.iter().map(|(_, c)| *c).collect::<Vec<_>>()).unwrap();

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
<html lang="en" data-lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>bench — llm-transpile report</title>
<script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.2/dist/chart.umd.min.js"></script>
<style>
:root{{--bg:#0f1117;--surf:#1a1d27;--bdr:#2e3147;--txt:#e2e8f0;--mut:#8892a4;
  --acc:#6366f1;--grn:#22c55e;--ylw:#eab308;--red:#ef4444;--thead:#1e2235;}}
:root[data-theme="light"]{{--bg:#f5f5f7;--surf:#ffffff;--bdr:#d1d5db;--txt:#1e293b;--mut:#374151;
  --acc:#4f46e5;--grn:#16a34a;--ylw:#ca8a04;--red:#dc2626;--thead:#f1f5f9;}}
*{{box-sizing:border-box;margin:0;padding:0;}}
body{{background:var(--bg);color:var(--txt);font-family:system-ui,sans-serif;font-size:14px;}}
header{{padding:16px 28px;border-bottom:1px solid var(--bdr);display:flex;align-items:center;gap:12px;flex-wrap:wrap;}}
header h1{{font-size:20px;font-weight:700;}}
.badge{{background:var(--acc);color:#fff;font-size:11px;padding:2px 8px;border-radius:99px;font-weight:600;}}
.lang-btn{{background:transparent;border:1px solid var(--bdr);color:var(--mut);
  border-radius:7px;padding:4px 12px;font-size:12px;cursor:pointer;font-weight:600;transition:all .15s;}}
.lang-btn:hover{{border-color:var(--acc);color:var(--txt);}}
.hdr-actions{{display:flex;gap:6px;flex-shrink:0;}}
.hdr-meta{{font-size:12px;color:var(--mut);}}
.wrap{{max-width:1400px;margin:0 auto;padding:20px 28px;}}
/* ── KPI cards ── */
.cards{{display:grid;grid-template-columns:repeat(auto-fit,minmax(160px,1fr));gap:14px;margin-bottom:28px;}}
.card{{background:var(--surf);border:1px solid var(--bdr);border-radius:10px;padding:18px;position:relative;cursor:help;}}
.card .lbl{{font-size:11px;color:var(--mut);text-transform:uppercase;letter-spacing:.5px;margin-bottom:6px;}}
.card .val{{font-size:26px;font-weight:700;}}
.card .sub{{font-size:11px;color:var(--mut);margin-top:3px;}}
.card .tip{{display:none;position:absolute;bottom:calc(100% + 8px);left:50%;transform:translateX(-50%);
  background:var(--surf);border:1px solid var(--bdr);border-radius:8px;padding:10px 14px;
  font-size:12px;color:var(--txt);line-height:1.6;width:220px;z-index:99;
  box-shadow:0 4px 20px rgba(0,0,0,.5);pointer-events:none;white-space:normal;}}
.card:hover .tip{{display:block;}}
/* ── Charts ── */
.charts{{display:grid;grid-template-columns:1fr 1fr;gap:18px;margin-bottom:28px;}}
@media(max-width:1200px){{.charts{{grid-template-columns:1fr 1fr;gap:14px;}}}}
@media(max-width:768px){{
  .charts{{grid-template-columns:1fr;gap:12px;}}
  .cards{{grid-template-columns:repeat(auto-fit,minmax(130px,1fr));gap:10px;}}
  header{{padding:12px 16px;flex-wrap:wrap;}}
  header h1{{font-size:16px;}}
  .hdr-meta{{display:none;}}
  .wrap{{padding:14px 16px;}}
  .cbox{{padding:14px;}}
  .cbox canvas{{max-height:200px;}}
  table{{font-size:12px;}}
  th,td{{padding:5px 6px;}}
}}
@media(max-width:480px){{
  .cards{{grid-template-columns:1fr 1fr;}}
  .card .val{{font-size:22px;}}
  header h1{{font-size:14px;}}
  .badge{{font-size:10px;padding:2px 6px;}}
  table{{display:block;overflow-x:auto;}}
  .frow{{flex-direction:column;align-items:stretch;}}
}}
.cbox{{background:var(--surf);border:1px solid var(--bdr);border-radius:10px;padding:18px;}}
.cbox-hdr{{display:flex;align-items:center;gap:6px;margin-bottom:14px;}}
.cbox h3{{font-size:11px;font-weight:600;color:var(--mut);text-transform:uppercase;letter-spacing:.5px;flex:1;}}
.tip-icon{{width:16px;height:16px;border-radius:50%;background:var(--bdr);color:var(--mut);
  font-size:10px;font-weight:700;display:flex;align-items:center;justify-content:center;
  cursor:help;position:relative;flex-shrink:0;}}
.tip-icon .tip{{display:none;position:absolute;top:calc(100% + 6px);right:0;
  background:var(--surf);border:1px solid var(--bdr);border-radius:8px;padding:10px 14px;
  font-size:12px;color:var(--txt);line-height:1.6;width:240px;z-index:99;
  box-shadow:0 4px 20px rgba(0,0,0,.5);pointer-events:none;white-space:normal;text-transform:none;letter-spacing:0;}}
.tip-icon:hover .tip{{display:block;}}
.cbox canvas{{max-height:240px;}}
/* ── Tables ── */
section{{margin-bottom:28px;}}
section .sec-hdr{{display:flex;align-items:center;gap:8px;margin-bottom:12px;padding-bottom:7px;border-bottom:1px solid var(--bdr);}}
section .sec-hdr h2{{font-size:15px;font-weight:600;}}
table{{width:100%;border-collapse:collapse;background:var(--surf);border-radius:10px;overflow:hidden;border:1px solid var(--bdr);}}
thead tr{{background:var(--thead);}}
th{{padding:9px 12px;text-align:left;font-size:11px;font-weight:600;color:var(--mut);
  text-transform:uppercase;letter-spacing:.5px;white-space:nowrap;}}
td{{padding:8px 12px;border-top:1px solid var(--bdr);font-size:13px;white-space:nowrap;}}
td.n{{text-align:right;font-variant-numeric:tabular-nums;}}
tr:hover td{{background:rgba(255,255,255,.02);}}
.good{{color:var(--grn);}} .ok{{color:var(--ylw);}} .low{{color:var(--red);}}
/* ── Filters ── */
.frow{{display:flex;gap:8px;margin-bottom:12px;flex-wrap:wrap;align-items:center;}}
input[type=text],select{{background:var(--surf);border:1px solid var(--bdr);border-radius:7px;
  padding:5px 10px;color:var(--txt);font-size:13px;outline:none;}}
input[type=text]:focus,select:focus{{border-color:var(--acc);}}
button{{background:var(--acc);border:none;color:#fff;border-radius:7px;
  padding:5px 12px;font-size:13px;cursor:pointer;font-weight:600;}}
button:hover{{opacity:.85;}}
/* ── Legend ── */
.legend{{display:flex;gap:16px;flex-wrap:wrap;margin-bottom:10px;font-size:12px;color:var(--mut);}}
.legend span{{display:flex;align-items:center;gap:5px;}}
.dot{{width:10px;height:10px;border-radius:50%;display:inline-block;}}
</style>
</head>
<body>
<div class="wrap">
<header style="margin-bottom:18px;">
  <h1>bench</h1>
  <span class="badge">llm-transpile</span>
  <span class="hdr-meta" data-i18n="hdr_meta">{runs} runs · {total_files} records</span>
  <span class="hdr-actions">
    <button class="lang-btn" onclick="toggleLang()" id="langBtn">한국어</button>
    <button class="lang-btn" onclick="toggleTheme()" id="themeBtn">☀</button>
  </span>
</header>

<!-- ── KPI Cards ── -->
<div class="cards">
  <div class="card">
    <div class="lbl" data-i18n="kpi_sem_lbl">Semantic Reduction</div>
    <div class="val" style="color:var(--grn)">{sem_pct:.1}%</div>
    <div class="sub" data-i18n="kpi_avg_files">avg all files</div>
    <div class="tip" data-i18n="tip_sem"></div>
  </div>
  <div class="card">
    <div class="lbl" data-i18n="kpi_cmp_lbl">Compressed Reduction</div>
    <div class="val" style="color:var(--grn)">{cmp_pct:.1}%</div>
    <div class="sub" data-i18n="kpi_avg_files">avg all files</div>
    <div class="tip" data-i18n="tip_cmp"></div>
  </div>
  <div class="card">
    <div class="lbl" data-i18n="kpi_thru_lbl">Throughput</div>
    <div class="val" style="color:var(--acc)">{avg_tok_ms:.0}</div>
    <div class="sub" data-i18n="kpi_tokms">tok/ms avg</div>
    <div class="tip" data-i18n="tip_thru"></div>
  </div>
  <div class="card">
    <div class="lbl" data-i18n="kpi_cov_lbl">Word Coverage</div>
    <div class="val" style="color:var(--ylw)">{avg_cov:.1}%</div>
    <div class="sub" data-i18n="kpi_lossless">lossless avg</div>
    <div class="tip" data-i18n="tip_cov"></div>
  </div>
  <div class="card">
    <div class="lbl" data-i18n="kpi_total_lbl">Total Input Tokens</div>
    <div class="val">{total_in}</div>
    <div class="sub" data-i18n="kpi_all_runs">across all runs</div>
    <div class="tip" data-i18n="tip_total"></div>
  </div>
  <div class="card">
    <div class="lbl" data-i18n="kpi_runs_lbl">Runs</div>
    <div class="val">{runs}</div>
    <div class="sub"><span data-i18n="kpi_measurements">{total_files} measurements</span></div>
    <div class="tip" data-i18n="tip_runs"></div>
  </div>
</div>

<!-- colour guide -->
<div class="legend" style="margin-bottom:18px;">
  <span><span class="dot" style="background:var(--grn)"></span><span data-i18n="legend_good">≥20% — good</span></span>
  <span><span class="dot" style="background:var(--ylw)"></span><span data-i18n="legend_ok">5–20% — ok</span></span>
  <span><span class="dot" style="background:var(--red)"></span><span data-i18n="legend_low">&lt;5% — low / negative</span></span>
</div>

<!-- ── Charts ── -->
<div class="charts">
  <div class="cbox">
    <div class="cbox-hdr">
      <h3 data-i18n="chart_trend_title">Token Reduction Over Time (%)</h3>
      <div class="tip-icon">?<div class="tip" data-i18n="tip_chart_trend"></div></div>
    </div>
    <canvas id="trendChart"></canvas>
  </div>
  <div class="cbox">
    <div class="cbox-hdr">
      <h3 data-i18n="chart_thru_title">Throughput Over Time (tok/ms)</h3>
      <div class="tip-icon">?<div class="tip" data-i18n="tip_chart_thru"></div></div>
    </div>
    <canvas id="thruChart"></canvas>
  </div>
  <div class="cbox">
    <div class="cbox-hdr">
      <h3 data-i18n="chart_scatter_title">Sem% vs Throughput — Scatter</h3>
      <div class="tip-icon">?<div class="tip" data-i18n="tip_chart_scatter"></div></div>
    </div>
    <canvas id="scatterChart"></canvas>
  </div>
  <div class="cbox">
    <div class="cbox-hdr">
      <h3 data-i18n="chart_box_title">Reduction by Format (min/Q1/med/Q3/max)</h3>
      <div class="tip-icon">?<div class="tip" data-i18n="tip_chart_box"></div></div>
    </div>
    <canvas id="boxChart"></canvas>
  </div>
  <div class="cbox">
    <div class="cbox-hdr">
      <h3 data-i18n="chart_pie_title">File Count by Format</h3>
      <div class="tip-icon">?<div class="tip" data-i18n="tip_chart_pie"></div></div>
    </div>
    <canvas id="pieChart"></canvas>
  </div>
  <div class="cbox">
    <div class="cbox-hdr">
      <h3 data-i18n="chart_hist_title">Input Token Size Distribution</h3>
      <div class="tip-icon">?<div class="tip" data-i18n="tip_chart_hist"></div></div>
    </div>
    <canvas id="histChart"></canvas>
  </div>
  <div class="cbox">
    <div class="cbox-hdr">
      <h3 data-i18n="chart_cov_title">Word Coverage (Lossless Quality)</h3>
      <div class="tip-icon">?<div class="tip" data-i18n="tip_chart_cov"></div></div>
    </div>
    <canvas id="covDonut"></canvas>
  </div>
</div>

<!-- ── Runs table ── -->
<section>
  <div class="sec-hdr">
    <h2 data-i18n="sec_runs">Runs</h2>
    <div class="tip-icon">?<div class="tip" data-i18n="tip_sec_runs"></div></div>
  </div>
  <table>
    <thead><tr>
      <th data-i18n="col_run_id">run id</th>
      <th data-i18n="col_timestamp">timestamp</th>
      <th data-i18n="col_files">files</th>
      <th>sem%</th><th>cmp%</th>
      <th data-i18n="col_tokms">tok/ms</th>
      <th data-i18n="col_coverage">coverage</th>
    </tr></thead>
    <tbody>{run_rows}</tbody>
  </table>
</section>

<!-- ── All records ── -->
<section>
  <div class="sec-hdr">
    <h2 data-i18n="sec_records">All Records</h2>
    <div class="tip-icon">?<div class="tip" data-i18n="tip_sec_records"></div></div>
  </div>
  <div class="frow">
    <input type="text" id="ftxt" data-i18n-ph="filter_ph" placeholder="Filter file…" oninput="filterTbl()">
    <select id="ffmt" onchange="filterTbl()">
      <option value="" data-i18n="fmt_all">All formats</option>
      <option>markdown</option><option>html</option><option>plaintext</option>
    </select>
    <select id="frun" onchange="filterTbl()">
      <option value="" data-i18n="run_all">All runs</option>
      {run_options}
    </select>
    <button onclick="exportCsv()" data-i18n="btn_csv">⬇ CSV</button>
  </div>
  <table id="tbl">
    <thead><tr>
      <th data-i18n="col_run">run</th>
      <th data-i18n="col_file">file</th>
      <th data-i18n="col_format">format</th>
      <th data-i18n="col_intok">in tok</th>
      <th>sem%</th><th>cmp%</th>
      <th data-i18n="col_sem_ms">sem ms</th>
      <th data-i18n="col_cmp_ms">cmp ms</th>
      <th data-i18n="col_tokms">tok/ms</th>
      <th data-i18n="col_coverage">coverage</th>
    </tr></thead>
    <tbody id="tbody">{table_rows}</tbody>
  </table>
</section>
</div>

<script>
// ── i18n ─────────────────────────────────────────────────────────────────────
const I18N = {{
  en: {{
    hdr_meta: '{runs} runs · {total_files} records',
    kpi_sem_lbl: 'Semantic Reduction',
    kpi_cmp_lbl: 'Compressed Reduction',
    kpi_thru_lbl: 'Throughput',
    kpi_cov_lbl: 'Word Coverage',
    kpi_total_lbl: 'Total Input Tokens',
    kpi_runs_lbl: 'Runs',
    kpi_avg_files: 'avg all files',
    kpi_tokms: 'tok/ms avg',
    kpi_lossless: 'lossless avg',
    kpi_all_runs: 'across all runs',
    kpi_measurements: '{total_files} measurements',
    tip_sem: 'Tokens saved in Semantic mode vs raw input. Target ≥15%. Green ≥20%, yellow 5–20%, red <5%.',
    tip_cmp: 'Tokens saved in Compressed mode (aggressive). Higher than Semantic because more content is pruned.',
    tip_thru: 'How many input tokens are processed per millisecond. Higher = faster. Depends on file size and CPU warmup.',
    tip_cov: 'Fraction of unique content words (>5 chars) from source that survive in Lossless output. Should stay ≥95%.',
    tip_total: 'Sum of all input tokens across every file and every run. Gives a sense of total workload processed.',
    tip_runs: 'Each run = one invocation of `bench run`. Running repeatedly reveals throughput variance and warmup effects.',
    legend_good: '≥20% — good',
    legend_ok: '5–20% — ok',
    legend_low: '<5% — low / negative',
    chart_trend_title: 'Token Reduction Over Time (%)',
    chart_thru_title: 'Throughput Over Time (tok/ms)',
    chart_scatter_title: 'Sem% vs Throughput — Scatter',
    chart_box_title: 'Reduction by Format (min/Q1/med/Q3/max)',
    chart_pie_title: 'File Count by Format',
    chart_hist_title: 'Input Token Size Distribution',
    chart_cov_title: 'Word Coverage (Lossless Quality)',
    tip_chart_trend: 'Line chart of Semantic% and Compressed% reduction per run. Rising trend = the compressor is getting better over time (or input files changed).',
    tip_chart_thru: 'Bar chart: tok/ms per run. A large jump between runs usually indicates CPU cache warmup. Subsequent runs are more representative.',
    tip_chart_scatter: 'Each dot is one file. X = semantic reduction %, Y = throughput. Hover to see filename. Files clustered top-right are both compact and fast.',
    tip_chart_box: 'Box plot per format showing min, Q1, median, Q3, max of Semantic reduction. Wide spread = inconsistent compression. Negative means the output grew.',
    tip_chart_pie: 'Share of each file format in the dataset. Dominated by Markdown; HTML/Plaintext are minority.',
    tip_chart_hist: 'How many files fall into each token-size bucket. Most eval files are <4K tokens. 32K+ indicates very large documents.',
    tip_chart_cov: 'Word coverage buckets for Lossless output. Ideally 100% (deep green). Yellow/red means content words were lost — investigate the compressor.',
    sec_runs: 'Runs',
    sec_records: 'All Records',
    tip_sec_runs: 'One row per bench run. Compare sem% and cmp% across runs to spot regressions after code changes.',
    tip_sec_records: 'Full record table. Filter by file name, format, or run. Click CSV to export the visible rows.',
    col_run_id: 'run id', col_timestamp: 'timestamp', col_files: 'files',
    col_tokms: 'tok/ms', col_coverage: 'coverage', col_run: 'run',
    col_file: 'file', col_format: 'format', col_intok: 'in tok',
    col_sem_ms: 'sem ms', col_cmp_ms: 'cmp ms',
    filter_ph: 'Filter file…', fmt_all: 'All formats', run_all: 'All runs', btn_csv: '⬇ CSV',
  }},
  ko: {{
    hdr_meta: '{runs}회 실행 · {total_files}개 기록',
    kpi_sem_lbl: '시맨틱 압축률',
    kpi_cmp_lbl: '최대 압축률',
    kpi_thru_lbl: '처리 속도',
    kpi_cov_lbl: '단어 보존율',
    kpi_total_lbl: '총 입력 토큰',
    kpi_runs_lbl: '실행 횟수',
    kpi_avg_files: '전체 파일 평균',
    kpi_tokms: 'tok/ms 평균',
    kpi_lossless: '무손실 평균',
    kpi_all_runs: '전체 실행 합산',
    kpi_measurements: '{total_files}개 측정값',
    tip_sem: '시맨틱 모드에서 원본 대비 줄어든 토큰 비율입니다. 목표 ≥15%. 초록 ≥20%, 노랑 5~20%, 빨강 <5%.',
    tip_cmp: '압축 모드(적극적 제거)에서 절약된 토큰 비율. 시맨틱보다 높지만 정보 손실 가능성도 큽니다.',
    tip_thru: '1밀리초당 처리되는 입력 토큰 수. 높을수록 빠릅니다. 파일 크기와 CPU 웜업 상태에 영향 받습니다.',
    tip_cov: '원본의 주요 단어(5자 초과) 중 무손실 출력에 살아남은 비율. 95% 이상 유지 권장.',
    tip_total: '모든 실행·파일에 걸친 입력 토큰 총합. 처리된 전체 워크로드 규모를 나타냅니다.',
    tip_runs: '실행 1회 = `bench run` 1번 호출. 반복 실행으로 처리 속도 분산과 웜업 효과를 확인하세요.',
    legend_good: '≥20% — 양호',
    legend_ok: '5~20% — 보통',
    legend_low: '<5% — 낮음 / 음수',
    chart_trend_title: '실행별 토큰 압축률 추이 (%)',
    chart_thru_title: '실행별 처리 속도 (tok/ms)',
    chart_scatter_title: '시맨틱 압축률 vs 처리 속도 (파일별)',
    chart_box_title: '포맷별 압축률 분포 (최소/Q1/중앙/Q3/최대)',
    chart_pie_title: '포맷별 파일 수',
    chart_hist_title: '입력 토큰 크기 분포',
    chart_cov_title: '단어 보존율 (무손실 품질)',
    tip_chart_trend: '실행별 시맨틱·압축 모드의 토큰 절약률 꺾은선 그래프입니다. 상승 추세라면 압축기 성능이 향상되거나 입력 파일이 바뀐 것입니다.',
    tip_chart_thru: '실행별 처리 속도 막대 그래프. 첫 실행과 이후 실행 간 큰 차이는 CPU 캐시 웜업 때문입니다. 두 번째 이후 값이 더 신뢰할 수 있습니다.',
    tip_chart_scatter: '각 점 = 파일 1개. X = 시맨틱 압축률, Y = 처리 속도. 마우스를 올리면 파일명이 표시됩니다. 오른쪽 위에 모일수록 빠르고 효율적입니다.',
    tip_chart_box: '포맷별 시맨틱 압축률의 최솟값·Q1·중앙값·Q3·최댓값 박스 그래프. 범위가 넓으면 압축 결과가 일관되지 않은 것입니다. 음수면 출력이 오히려 커진 것.',
    tip_chart_pie: '데이터셋 내 파일 포맷 비율. Markdown이 대부분을 차지하며 HTML·Plaintext는 소수입니다.',
    tip_chart_hist: '토큰 크기 구간별 파일 수. 평가 파일 대부분은 4K 토큰 이하. 32K+ 는 매우 큰 문서입니다.',
    tip_chart_cov: '무손실 출력의 단어 보존율 구간 도넛 차트. 진한 초록(100%)이 이상적입니다. 노랑·빨강이 나타나면 압축기가 핵심 단어를 제거하고 있다는 신호입니다.',
    sec_runs: '실행 기록',
    sec_records: '전체 측정값',
    tip_sec_runs: '실행 1회당 1행. sem%·cmp%를 비교해 코드 변경 후 성능 저하를 감지하세요.',
    tip_sec_records: '전체 레코드 테이블. 파일명·포맷·실행 ID로 필터링하고 CSV로 내보낼 수 있습니다.',
    col_run_id: '실행 ID', col_timestamp: '타임스탬프', col_files: '파일 수',
    col_tokms: 'tok/ms', col_coverage: '보존율', col_run: '실행',
    col_file: '파일', col_format: '포맷', col_intok: '입력 토큰',
    col_sem_ms: '시맨틱 ms', col_cmp_ms: '압축 ms',
    filter_ph: '파일 필터…', fmt_all: '전체 포맷', run_all: '전체 실행', btn_csv: '⬇ CSV 내보내기',
  }},
}};

let LANG = 'en';
function t(k) {{ return (I18N[LANG]||I18N.en)[k] || k; }}
function applyLang() {{
  document.documentElement.setAttribute('lang', LANG);
  document.querySelectorAll('[data-i18n]').forEach(el => {{
    const k = el.getAttribute('data-i18n');
    el.textContent = t(k);
  }});
  document.querySelectorAll('[data-i18n-ph]').forEach(el => {{
    el.placeholder = t(el.getAttribute('data-i18n-ph'));
  }});
  document.getElementById('langBtn').textContent = LANG === 'en' ? '한국어' : 'English';
}}
function toggleLang() {{
  LANG = LANG === 'en' ? 'ko' : 'en';
  applyLang();
}}
function toggleTheme(){{
  const r=document.documentElement;
  const next=(r.getAttribute('data-theme')||'dark')==='dark'?'light':'dark';
  r.setAttribute('data-theme',next);
  document.getElementById('themeBtn').textContent=next==='dark'?'☀':'🌙';
  localStorage.setItem('bench-theme',next);
  renderCharts();
}}
(function(){{const s=localStorage.getItem('bench-theme');if(s){{document.documentElement.setAttribute('data-theme',s);document.getElementById('themeBtn').textContent=s==='dark'?'☀':'🌙';}}}})();
// auto-detect browser language
if (navigator.language && navigator.language.startsWith('ko')) {{ LANG = 'ko'; }}
applyLang();

// ── Chart.js ─────────────────────────────────────────────────────────────────
function getStyle(v){{return getComputedStyle(document.documentElement).getPropertyValue(v).trim();}}
function getTC(){{return getStyle('--mut')||'#8892a4';}}
function getGC(){{return getStyle('--bdr')||'rgba(128,128,128,.15)';}}
const LABELS={labels}, SEM={sem_data}, CMP={cmp_data}, TOK={tok_data}, COV={cov_data};
const SCATTER={scatter_data}, BOX={box_data};
const FMT_COL={{'markdown':'#6366f1','html':'#22c55e','plaintext':'#eab308'}};
const grps={{}};
for(const p of SCATTER){{if(!grps[p.fmt])grps[p.fmt]=[];grps[p.fmt].push({{x:p.x,y:p.y,file:p.file,run:p.run}});}}
let charts={{}};
function mkChart(id,cfg){{if(charts[id]){{charts[id].destroy();}}charts[id]=new Chart(document.getElementById(id),cfg);}}
function renderCharts(){{
  const tc=getTC(),gc=getGC();
  const F={{color:tc}},G={{color:gc}};
  mkChart('trendChart',{{type:'line',data:{{labels:LABELS,datasets:[
    {{label:'Semantic%',data:SEM,borderColor:'#22c55e',backgroundColor:'rgba(34,197,94,.08)',tension:.3,pointRadius:4}},
    {{label:'Compressed%',data:CMP,borderColor:'#6366f1',backgroundColor:'rgba(99,102,241,.08)',tension:.3,pointRadius:4}},
  ]}},options:{{plugins:{{legend:{{labels:{{color:tc}}}}}},scales:{{x:{{ticks:F,grid:G}},y:{{ticks:F,grid:G,title:{{display:true,text:'% reduction',color:tc}}}}}}}}}});
  mkChart('thruChart',{{type:'bar',data:{{labels:LABELS,datasets:[{{label:'tok/ms',data:TOK,backgroundColor:'rgba(99,102,241,.7)',borderRadius:4}}]}},options:{{plugins:{{legend:{{labels:{{color:tc}}}}}},scales:{{x:{{ticks:F,grid:G}},y:{{ticks:F,grid:G,title:{{display:true,text:'tok/ms',color:tc}}}}}}}}}});
  mkChart('scatterChart',{{type:'scatter',data:{{datasets:Object.entries(grps).map(([fmt,pts])=>({{'label':fmt,'data':pts,'backgroundColor':(FMT_COL[fmt]||'#fff')+'cc','pointRadius':5}}))}},options:{{plugins:{{legend:{{labels:{{color:tc}}}},tooltip:{{callbacks:{{label:c=>`${{c.raw.file}} (${{c.raw.run}}): ${{c.raw.x}}% / ${{c.raw.y.toFixed(0)}} tok/ms`}}}}}},scales:{{x:{{ticks:F,grid:G,title:{{display:true,text:'Semantic reduction %',color:tc}}}},y:{{ticks:F,grid:G,title:{{display:true,text:'Throughput (tok/ms)',color:tc}}}}}}}}}});
  mkChart('boxChart',{{type:'bar',data:{{labels:BOX.map(b=>b.fmt),datasets:[
    {{label:'min–Q1',data:BOX.map(b=>[b.min,b.q1]),backgroundColor:'rgba(99,102,241,.3)',borderSkipped:false}},
    {{label:'Q1–med',data:BOX.map(b=>[b.q1,b.med]),backgroundColor:'rgba(99,102,241,.6)',borderSkipped:false}},
    {{label:'med–Q3',data:BOX.map(b=>[b.med,b.q3]),backgroundColor:'rgba(34,197,94,.6)',borderSkipped:false}},
    {{label:'Q3–max',data:BOX.map(b=>[b.q3,b.max]),backgroundColor:'rgba(34,197,94,.3)',borderSkipped:false}},
  ]}},options:{{plugins:{{legend:{{labels:{{color:tc}}}}}},scales:{{x:{{ticks:F,grid:G}},y:{{ticks:F,grid:G,title:{{display:true,text:'Semantic reduction %',color:tc}}}}}}}}}});
  mkChart('pieChart',{{type:'doughnut',data:{{labels:{fmt_labels},datasets:[{{data:{fmt_count_data},backgroundColor:['rgba(99,102,241,.8)','rgba(34,197,94,.8)','rgba(234,179,8,.8)'],borderColor:['#6366f1','#22c55e','#eab308'],borderWidth:2}}]}},options:{{plugins:{{legend:{{labels:{{color:tc,padding:16}}}},tooltip:{{callbacks:{{label:c=>`${{c.label}}: ${{c.raw}} files (${{Math.round(c.raw/c.dataset.data.reduce((a,b)=>a+b,0)*100)}}%)`}}}}}}}}}});
  mkChart('histChart',{{type:'bar',data:{{labels:{hist_labels},datasets:[{{label:'files',data:{hist_data},backgroundColor:'rgba(99,102,241,.65)',borderColor:'#6366f1',borderWidth:1,borderRadius:4}}]}},options:{{plugins:{{legend:{{display:false}}}},scales:{{x:{{ticks:F,grid:G,title:{{display:true,text:'Token range',color:tc}}}},y:{{ticks:{{...F,stepSize:1}},grid:G,title:{{display:true,text:'# files',color:tc}}}}}}}}}});
  mkChart('covDonut',{{type:'doughnut',data:{{labels:{cov_donut_labels},datasets:[{{data:{cov_donut_data},backgroundColor:['rgba(34,197,94,.85)','rgba(34,197,94,.45)','rgba(234,179,8,.7)','rgba(239,68,68,.7)'],borderColor:['#22c55e','#22c55e','#eab308','#ef4444'],borderWidth:2}}]}},options:{{plugins:{{legend:{{labels:{{color:tc,padding:14}}}},tooltip:{{callbacks:{{label:c=>`${{c.label}}: ${{c.raw}} files`}}}}}},cutout:'60%'}}}});
}}
renderCharts();

// ── Filter & CSV ─────────────────────────────────────────────────────────────
function filterTbl(){{
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
        labels = js_safe(&labels),
        sem_data = js_safe(&sem_data),
        cmp_data = js_safe(&cmp_data),
        tok_data = js_safe(&tok_data),
        cov_data = js_safe(&cov_data),
        scatter_data = js_safe(&scatter_data),
        box_data = js_safe(&box_data),
        fmt_labels = js_safe(&serde_json::to_string(&["markdown", "html", "plaintext"]).unwrap()),
        fmt_count_data = js_safe(&fmt_count_data),
        hist_labels = js_safe(&hist_labels),
        hist_data = js_safe(&hist_data),
        cov_donut_labels = js_safe(&cov_donut_labels),
        cov_donut_data = js_safe(&cov_donut_data),
    );

    if let Err(e) = fs::write(out_path, &html) {
        eprintln!("ERROR: cannot write HTML to {out_path}: {e}");
        std::process::exit(1);
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

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
        // 4 ASCII chars + '…' (3 UTF-8 bytes) = 7 bytes max
        assert!(s.len() <= 7);
        assert!(s.chars().count() <= 5);
    }

    #[test]
    fn trunc_multibyte_no_panic() {
        // Korean chars are 3 bytes each — must not panic on byte boundary
        let s = trunc("안녕하세요반갑습니다", 5);
        assert!(s.chars().count() <= 5);
    }

    #[test]
    fn js_safe_escapes_script_close() {
        assert_eq!(js_safe("</script>"), "<\\/script>");
    }

    #[test]
    fn js_safe_leaves_normal_json_unchanged() {
        let json = r#"{"x":1,"y":"hello"}"#;
        assert_eq!(js_safe(json), json);
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
