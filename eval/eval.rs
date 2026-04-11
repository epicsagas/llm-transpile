/// eval.rs — llm-transpiler quantitative evaluation
///
/// Metrics per file:
///   - Token reduction rate: Semantic / Compressed / Lossless
///   - Throughput (tok/ms) — median of 3 runs
///   - Lossless word coverage % (unique content words preserved)
///
/// Formats covered:
///   - Markdown  : eval/dataset/policy/ + eval/dataset/hf/
///   - HTML      : eval/dataset/html/
///   - PlainText : eval/dataset/plaintext/
///   - Multilingual Markdown: eval/dataset/multilingual/
use llm_transpiler::{FidelityLevel, InputFormat, token_count, transpile};
use std::fs;
use std::time::Instant;

// ── Result struct ─────────────────────────────────────────────────────────────

#[derive(Debug)]
struct EvalResult {
    file: String,
    format: InputFormat,
    input_bytes: usize,
    input_tok: usize,
    semantic_tok: usize,
    compressed_tok: usize,
    lossless_tok: usize,
    /// Median of 3 runs (ms)
    semantic_ms: u128,
    /// Median of 3 runs (ms)
    compressed_ms: u128,
    /// Percentage of unique content words (>5 chars, alphabetic) from source
    /// that are present in the Lossless output. 100.0 = fully preserved.
    lossless_word_coverage: f64,
}

// ── Core evaluation function ──────────────────────────────────────────────────

fn eval_file(path: &str, format: InputFormat) -> Option<EvalResult> {
    let content = fs::read_to_string(path).ok()?;
    let input_tok = token_count(&content);

    // Helper: run transpile N times and return (output, median_ms)
    let timed = |fmt: InputFormat, fidelity: FidelityLevel, budget: Option<usize>| -> Option<(String, u128)> {
        let mut timings = [0u128; 3];
        let mut out = String::new();
        for t in &mut timings {
            let t0 = Instant::now();
            out = transpile(&content, fmt, fidelity, budget).ok()?;
            *t = t0.elapsed().as_millis();
        }
        timings.sort_unstable();
        Some((out, timings[1])) // median
    };

    let (sem, semantic_ms) = timed(format, FidelityLevel::Semantic, Some(4096))?;
    let (cmp, compressed_ms) = timed(format, FidelityLevel::Compressed, Some(2048))?;
    let (los, _) = timed(format, FidelityLevel::Lossless, None)?;

    let semantic_tok = token_count(&sem);
    let compressed_tok = token_count(&cmp);
    let lossless_tok = token_count(&los);

    // Lossless word coverage: % of unique content words from source found in output.
    // HTML comments are stripped first — the parser intentionally drops them.
    let stripped = strip_html_comments(&content);
    let unique_words: std::collections::HashSet<&str> = stripped
        .split_whitespace()
        .filter(|w| w.len() > 5 && w.chars().all(|c| c.is_alphabetic()))
        .collect();
    let lossless_word_coverage = if unique_words.is_empty() {
        100.0
    } else {
        let matched = unique_words.iter().filter(|w| los.contains(*w)).count();
        matched as f64 / unique_words.len() as f64 * 100.0
    };

    let fname = std::path::Path::new(path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    Some(EvalResult {
        file: fname,
        format,
        input_bytes: content.len(),
        input_tok,
        semantic_tok,
        compressed_tok,
        lossless_tok,
        semantic_ms,
        compressed_ms,
        lossless_word_coverage,
    })
}

/// Strips `<!-- ... -->` comment blocks so their words aren't sampled.
fn strip_html_comments(s: &str) -> String {
    let mut rest = s;
    let mut out = String::with_capacity(s.len());
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        if let Some(end) = rest[start..].find("-->") {
            rest = &rest[start + end + 3..];
        } else {
            break;
        }
    }
    out.push_str(rest);
    out
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn pct(a: usize, b: usize) -> f64 {
    if b == 0 { return 0.0; }
    100.0 - (a as f64 / b as f64 * 100.0)
}

/// Collect all files with a given extension from a directory (sorted).
fn collect_files(dir: &str, ext: &str) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else { return vec![] };
    let mut files: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some(ext) {
                p.to_str().map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect();
    files.sort();
    files
}

// ── Section printer ───────────────────────────────────────────────────────────

fn print_header() {
    println!(
        "{:<38} {:>4} {:>6} {:>8} {:>8} {:>7} {:>7} {:>7} {:>8} {:>9} {:>7}",
        "file", "fmt", "in_tok", "Sem%red", "Cmp%red",
        "Sem_ms", "Cmp_ms", "tok/ms", "Loss%red", "LossCov%", "in_KB"
    );
    println!("{}", "-".repeat(122));
}

fn print_row(r: &EvalResult) {
    let fmt_tag = match r.format {
        InputFormat::Markdown  => "md",
        InputFormat::Html      => "htm",
        InputFormat::PlainText => "txt",
    };
    let tokms = if r.semantic_ms > 0 {
        r.input_tok as f64 / r.semantic_ms as f64
    } else {
        r.input_tok as f64
    };
    println!(
        "{:<38} {:>4} {:>6} {:>8.1} {:>8.1} {:>7} {:>7} {:>7.0} {:>8.1} {:>8.1}% {:>7.1}",
        r.file, fmt_tag, r.input_tok,
        pct(r.semantic_tok, r.input_tok),
        pct(r.compressed_tok, r.input_tok),
        r.semantic_ms, r.compressed_ms, tokms,
        pct(r.lossless_tok, r.input_tok),
        r.lossless_word_coverage,
        r.input_bytes as f64 / 1024.0,
    );
}

fn print_totals(results: &[EvalResult]) {
    if results.is_empty() { return; }

    let total_input: usize = results.iter().map(|r| r.input_tok).sum();
    let total_sem: usize   = results.iter().map(|r| r.semantic_tok).sum();
    let total_cmp: usize   = results.iter().map(|r| r.compressed_tok).sum();
    let total_sem_ms: u128 = results.iter().map(|r| r.semantic_ms).sum();
    let total_cmp_ms: u128 = results.iter().map(|r| r.compressed_ms).sum();
    let avg_coverage: f64  = results.iter().map(|r| r.lossless_word_coverage).sum::<f64>()
        / results.len() as f64;
    let n = results.len();
    let avg_sem_ms = total_sem_ms / n as u128;
    let avg_cmp_ms = total_cmp_ms / n as u128;
    let total_tokms = if total_sem_ms > 0 {
        total_input as f64 / total_sem_ms as f64
    } else {
        0.0
    };

    let total_lossless: usize = results.iter().map(|r| r.lossless_tok).sum();
    println!("{}", "═".repeat(122));
    println!(
        "{:<38} {:>4} {:>6} {:>8.1} {:>8.1} {:>7} {:>7} {:>7.0} {:>8.1} {:>8.1}% {:>7}",
        "total/avg", "", total_input,
        pct(total_sem, total_input),
        pct(total_cmp, total_input),
        avg_sem_ms, avg_cmp_ms, total_tokms,
        pct(total_lossless, total_input),
        avg_coverage, "",
    );
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    let base = concat!(env!("CARGO_MANIFEST_DIR"), "/eval");

    // ── Section 1: Markdown (policy + hf) ──────────────────────────────────────
    println!("\n▶ Markdown — policy + HuggingFace docs");
    print_header();

    let mut md_files: Vec<String> = Vec::new();
    for dir in &[
        format!("{base}/dataset/policy"),
        format!("{base}/dataset/hf"),
        format!("{base}/dataset/multilingual"),
    ] {
        md_files.extend(collect_files(dir, "md"));
    }

    let mut md_results: Vec<EvalResult> = Vec::new();
    for f in &md_files {
        if let Some(r) = eval_file(f, InputFormat::Markdown) {
            print_row(&r);
            md_results.push(r);
        }
    }
    print_totals(&md_results);

    // ── Section 2: HTML ─────────────────────────────────────────────────────────
    let html_files = collect_files(&format!("{base}/dataset/html"), "html");
    if !html_files.is_empty() {
        println!("\n▶ HTML");
        print_header();
        let mut html_results: Vec<EvalResult> = Vec::new();
        for f in &html_files {
            if let Some(r) = eval_file(f, InputFormat::Html) {
                print_row(&r);
                html_results.push(r);
            }
        }
        print_totals(&html_results);
    }

    // ── Section 3: PlainText ────────────────────────────────────────────────────
    let txt_files = collect_files(&format!("{base}/dataset/plaintext"), "txt");
    if !txt_files.is_empty() {
        println!("\n▶ PlainText");
        print_header();
        let mut txt_results: Vec<EvalResult> = Vec::new();
        for f in &txt_files {
            if let Some(r) = eval_file(f, InputFormat::PlainText) {
                print_row(&r);
                txt_results.push(r);
            }
        }
        print_totals(&txt_results);
    }

    // ── Combined summary ────────────────────────────────────────────────────────
    let all: Vec<&EvalResult> = md_results.iter()
        .chain(md_results.iter().take(0)) // placeholder for combined iter
        .collect();
    // Compute combined stats manually
    let all_results: Vec<EvalResult> = {
        let mut v: Vec<String> = md_files.clone();
        v.extend(html_files);
        v.extend(txt_files);
        let fmts: Vec<InputFormat> = md_files.iter().map(|_| InputFormat::Markdown)
            .chain(collect_files(&format!("{base}/dataset/html"), "html").iter().map(|_| InputFormat::Html))
            .chain(collect_files(&format!("{base}/dataset/plaintext"), "txt").iter().map(|_| InputFormat::PlainText))
            .collect();
        v.iter().zip(fmts.iter())
            .filter_map(|(f, fmt)| eval_file(f, *fmt))
            .collect()
    };
    let _ = all; // suppress warning

    let grand_input: usize  = all_results.iter().map(|r| r.input_tok).sum();
    let grand_sem: usize    = all_results.iter().map(|r| r.semantic_tok).sum();
    let grand_cmp: usize    = all_results.iter().map(|r| r.compressed_tok).sum();
    let grand_sem_ms: u128  = all_results.iter().map(|r| r.semantic_ms).sum();
    let grand_tokms = if grand_sem_ms > 0 { grand_input as f64 / grand_sem_ms as f64 } else { 0.0 };
    let grand_coverage: f64 = if all_results.is_empty() { 0.0 } else {
        all_results.iter().map(|r| r.lossless_word_coverage).sum::<f64>() / all_results.len() as f64
    };
    let low_coverage_count = all_results.iter().filter(|r| r.lossless_word_coverage < 90.0).count();

    println!("\n📊 Summary (all formats):");
    println!("  • Documents evaluated:      {}", all_results.len());
    println!("  • Total input tokens:       {grand_input}");
    println!("  • Semantic   avg reduction: {:.1}%", pct(grand_sem, grand_input));
    println!("  • Compressed avg reduction: {:.1}%", pct(grand_cmp, grand_input));
    println!("  • Lossless word coverage:   {grand_coverage:.1}% avg  ({low_coverage_count} files below 90%)");
    println!("  • Throughput (Semantic):    {grand_tokms:.0} tok/ms  [release build]");
    println!("  • Total output (Semantic):  {grand_sem}");
}
