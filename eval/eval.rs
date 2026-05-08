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
use llm_transpile::{FidelityLevel, InputFormat, token_count, transpile};
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
    /// Median of 3 runs (µs for sub-ms precision)
    semantic_us: u128,
    /// Median of 3 runs (µs)
    compressed_us: u128,
    /// Percentage of unique content words (>5 chars, alphabetic) from source
    /// that are present in the Lossless output. 100.0 = fully preserved.
    /// Note: words replaced by PUA symbols are checked in the <D> dict block,
    /// so this metric accurately reflects content preservation even after
    /// symbol substitution.
    lossless_word_coverage: f64,
}

// ── Core evaluation function ──────────────────────────────────────────────────

fn eval_file(path: &str, format: InputFormat) -> Option<EvalResult> {
    let content = fs::read_to_string(path).ok()?;
    let input_tok = token_count(&content);

    // Helper: run transpile N times and return (output, median_µs)
    let timed = |fmt: InputFormat,
                 fidelity: FidelityLevel,
                 budget: Option<usize>|
     -> Option<(String, u128)> {
        let mut timings = [0u128; 3];
        let mut out = String::new();
        for t in &mut timings {
            let t0 = Instant::now();
            out = transpile(&content, fmt, fidelity, budget).ok()?;
            *t = t0.elapsed().as_micros();
        }
        timings.sort_unstable();
        Some((out, timings[1])) // median
    };

    let (sem, semantic_us) = timed(format, FidelityLevel::Semantic, Some(4096))?;
    let (cmp, compressed_us) = timed(format, FidelityLevel::Compressed, Some(2048))?;
    let (los, _) = timed(format, FidelityLevel::Lossless, None)?;

    let semantic_tok = token_count(&sem);
    let compressed_tok = token_count(&cmp);
    let lossless_tok = token_count(&los);

    // Lossless word coverage: % of unique content words from source found in output.
    // Strip HTML comments AND script/style blocks before sampling source words,
    // since the parser correctly drops those non-content sections.
    let stripped = strip_non_content(&content);
    let unique_words: std::collections::HashSet<&str> = stripped
        .split_whitespace()
        .filter(|w| w.len() > 5 && w.chars().all(|c| c.is_alphabetic()))
        .collect();
    let lossless_word_coverage = if unique_words.is_empty() {
        100.0
    } else {
        // Check both the output body and the <D> dictionary block so that
        // PUA-substituted words are counted as present.
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
        semantic_us,
        compressed_us,
        lossless_word_coverage,
    })
}

/// Strips HTML comments (`<!-- ... -->`), `<script>` blocks, and `<style>` blocks
/// so their words aren't counted in the source word sample.
/// The parser intentionally drops all of these as non-content.
fn strip_non_content(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;

    while !rest.is_empty() {
        // Find the next tag or comment to strip
        let next_comment = rest.find("<!--");
        let next_script = rest.find("<script");
        let next_style = rest.find("<style");

        // Pick the earliest match
        let earliest = [next_comment, next_script, next_style]
            .iter()
            .filter_map(|&x| x)
            .min();

        let Some(start) = earliest else {
            out.push_str(rest);
            break;
        };

        out.push_str(&rest[..start]);

        if Some(start) == next_comment {
            if let Some(end) = rest[start..].find("-->") {
                rest = &rest[start + end + 3..];
            } else {
                break;
            }
        } else if Some(start) == next_script {
            if let Some(end) = rest[start..].find("</script>") {
                rest = &rest[start + end + 9..];
            } else {
                break;
            }
        } else {
            // style
            if let Some(end) = rest[start..].find("</style>") {
                rest = &rest[start + end + 8..];
            } else {
                break;
            }
        }
    }

    out
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn pct(a: usize, b: usize) -> f64 {
    if b == 0 {
        return 0.0;
    }
    100.0 - (a as f64 / b as f64 * 100.0)
}

/// Collect all files with a given extension from a directory (sorted).
fn collect_files(dir: &str, ext: &str) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return vec![];
    };
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
        "{:<38} {:>4} {:>6} {:>8} {:>8} {:>7} {:>7} {:>10} {:>8} {:>9} {:>7}",
        "file",
        "fmt",
        "in_tok",
        "Sem%red",
        "Cmp%red",
        "Sem_ms",
        "Cmp_ms",
        "tok/ms",
        "Loss%red",
        "LossCov%",
        "in_KB"
    );
    println!("{}", "-".repeat(126));
}

fn print_row(r: &EvalResult) {
    let fmt_tag = match r.format {
        InputFormat::Markdown => "md",
        InputFormat::Html => "htm",
        InputFormat::PlainText => "txt",
    };
    // Use µs for precision; display as ms with 1 decimal
    let sem_ms = r.semantic_us as f64 / 1000.0;
    let cmp_ms = r.compressed_us as f64 / 1000.0;
    // tok/ms: use µs denominator, convert back
    let tokms = if r.semantic_us > 0 {
        r.input_tok as f64 / r.semantic_us as f64 * 1000.0
    } else {
        // Sub-microsecond: report as ">1M tok/ms placeholder
        r.input_tok as f64 * 1000.0
    };
    println!(
        "{:<38} {:>4} {:>6} {:>8.1} {:>8.1} {:>7.1} {:>7.1} {:>10.0} {:>8.1} {:>8.1}% {:>7.1}",
        r.file,
        fmt_tag,
        r.input_tok,
        pct(r.semantic_tok, r.input_tok),
        pct(r.compressed_tok, r.input_tok),
        sem_ms,
        cmp_ms,
        tokms,
        pct(r.lossless_tok, r.input_tok),
        r.lossless_word_coverage,
        r.input_bytes as f64 / 1024.0,
    );
}

fn print_totals(results: &[EvalResult]) {
    if results.is_empty() {
        return;
    }

    let total_input: usize = results.iter().map(|r| r.input_tok).sum();
    let total_sem: usize = results.iter().map(|r| r.semantic_tok).sum();
    let total_cmp: usize = results.iter().map(|r| r.compressed_tok).sum();
    let total_sem_us: u128 = results.iter().map(|r| r.semantic_us).sum();
    let total_cmp_us: u128 = results.iter().map(|r| r.compressed_us).sum();
    let avg_coverage: f64 = results
        .iter()
        .map(|r| r.lossless_word_coverage)
        .sum::<f64>()
        / results.len() as f64;
    let n = results.len();
    let avg_sem_ms = total_sem_us as f64 / n as f64 / 1000.0;
    let avg_cmp_ms = total_cmp_us as f64 / n as f64 / 1000.0;
    // Aggregate throughput: total tokens / total time (µs → ms)
    let total_tokms = if total_sem_us > 0 {
        total_input as f64 / total_sem_us as f64 * 1000.0
    } else {
        total_input as f64 * 1000.0 // all sub-µs
    };

    let total_lossless: usize = results.iter().map(|r| r.lossless_tok).sum();
    println!("{}", "═".repeat(126));
    println!(
        "{:<38} {:>4} {:>6} {:>8.1} {:>8.1} {:>7.1} {:>7.1} {:>10.0} {:>8.1} {:>8.1}% {:>7}",
        "total/avg",
        "",
        total_input,
        pct(total_sem, total_input),
        pct(total_cmp, total_input),
        avg_sem_ms,
        avg_cmp_ms,
        total_tokms,
        pct(total_lossless, total_input),
        avg_coverage,
        "",
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
    let mut html_results: Vec<EvalResult> = Vec::new();
    if !html_files.is_empty() {
        println!("\n▶ HTML");
        print_header();
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
    let mut txt_results: Vec<EvalResult> = Vec::new();
    if !txt_files.is_empty() {
        println!("\n▶ PlainText");
        print_header();
        for f in &txt_files {
            if let Some(r) = eval_file(f, InputFormat::PlainText) {
                print_row(&r);
                txt_results.push(r);
            }
        }
        print_totals(&txt_results);
    }

    // ── Combined summary (reuse already-computed results — no re-evaluation) ────
    let all_results: Vec<&EvalResult> = md_results
        .iter()
        .chain(html_results.iter())
        .chain(txt_results.iter())
        .collect();

    let grand_input: usize = all_results.iter().map(|r| r.input_tok).sum();
    let grand_sem: usize = all_results.iter().map(|r| r.semantic_tok).sum();
    let grand_cmp: usize = all_results.iter().map(|r| r.compressed_tok).sum();
    let grand_sem_us: u128 = all_results.iter().map(|r| r.semantic_us).sum();
    let grand_tokms = if grand_sem_us > 0 {
        grand_input as f64 / grand_sem_us as f64 * 1000.0
    } else {
        grand_input as f64 * 1000.0
    };
    let grand_coverage: f64 = if all_results.is_empty() {
        0.0
    } else {
        all_results
            .iter()
            .map(|r| r.lossless_word_coverage)
            .sum::<f64>()
            / all_results.len() as f64
    };
    let low_coverage_count = all_results
        .iter()
        .filter(|r| r.lossless_word_coverage < 90.0)
        .count();

    println!("\n📊 Summary (all formats):");
    println!("  • Documents evaluated:      {}", all_results.len());
    println!("  • Total input tokens:       {grand_input}");
    println!(
        "  • Semantic   avg reduction: {:.1}%",
        pct(grand_sem, grand_input)
    );
    println!(
        "  • Compressed avg reduction: {:.1}%",
        pct(grand_cmp, grand_input)
    );
    println!(
        "  • Lossless word coverage:   {grand_coverage:.1}% avg  ({low_coverage_count} files below 90%)"
    );
    println!("  • Throughput (Semantic):    {grand_tokms:.0} tok/ms  [release build]");
    println!("  • Total output (Semantic):  {grand_sem}");
}
