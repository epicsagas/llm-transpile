/// eval.rs — llm-transpiler quantitative evaluation
///
/// Metrics per file:
///   - Token reduction rate (Semantic / Compressed / Lossless), measured with
///     BOTH the heuristic tokenizer AND the real cl100k BPE tokenizer.
///   - Throughput (tok/ms) — median of 3 runs
///   - Lossless word coverage % (unique content words preserved)
///
/// ## Token honesty
///
/// The heuristic tokenizer (`token_count`) bakes in the assumption that a PUA
/// character costs 1 token. The real cl100k tokenizer disagrees — a PUA char
/// costs 3 tokens (byte-fallback). Reporting only the heuristic therefore
/// *inflates* reduction on PUA-heavy output. This harness reports both and,
/// in `--json` mode, derives the composite score from the **BPE** numbers.
///
/// ## Output modes
///
/// - default (no flag): human-readable table + summary (preserved for REPL use)
/// - `--json`:          single JSON object on stdout for machine consumption
///   (`epic eval` consumes this via `result_type: composite`)
///
/// Formats covered:
///   - Markdown  : eval/dataset/policy/ + eval/dataset/hf/ + eval/dataset/multilingual/
///   - HTML      : eval/dataset/html/
///   - PlainText : eval/dataset/plaintext/
use llm_transpile::{
    DualTokenMeasurement, FidelityLevel, InputFormat, measure_tokens_dual, transpile,
};
use std::fs;
use std::time::Instant;

// ── Result struct ─────────────────────────────────────────────────────────────

#[derive(Debug)]
struct EvalResult {
    file: String,
    format: InputFormat,
    input_bytes: usize,
    input_tok: DualTokenMeasurement,
    semantic_tok: DualTokenMeasurement,
    compressed_tok: DualTokenMeasurement,
    lossless_tok: DualTokenMeasurement,
    /// Median of 3 runs (µs for sub-ms precision)
    semantic_us: u128,
    /// Median of 3 runs (µs)
    compressed_us: u128,
    /// Percentage of unique content words (>5 chars, alphabetic) from source
    /// that are present in the Lossless output. 100.0 = fully preserved.
    lossless_word_coverage: f64,
}

// ── Core evaluation function ──────────────────────────────────────────────────

fn eval_file(path: &str, format: InputFormat) -> Option<EvalResult> {
    let content = fs::read_to_string(path).ok()?;
    let input_tok = measure_tokens_dual(&content);

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

    let semantic_tok = measure_tokens_dual(&sem);
    let compressed_tok = measure_tokens_dual(&cmp);
    let lossless_tok = measure_tokens_dual(&los);

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

// ── Section printer (human-readable mode) ─────────────────────────────────────

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
        r.input_tok.heuristic as f64 / r.semantic_us as f64 * 1000.0
    } else {
        // Sub-microsecond: report as ">1M tok/ms placeholder
        r.input_tok.heuristic as f64 * 1000.0
    };
    println!(
        "{:<38} {:>4} {:>6} {:>8.1} {:>8.1} {:>7.1} {:>7.1} {:>10.0} {:>8.1} {:>8.1}% {:>7.1}",
        r.file,
        fmt_tag,
        r.input_tok.heuristic,
        // Human table uses the heuristic count for continuity with prior reports;
        // the JSON path reports both heuristic and BPE.
        pct(r.semantic_tok.heuristic, r.input_tok.heuristic),
        pct(r.compressed_tok.heuristic, r.input_tok.heuristic),
        sem_ms,
        cmp_ms,
        tokms,
        pct(r.lossless_tok.heuristic, r.input_tok.heuristic),
        r.lossless_word_coverage,
        r.input_bytes as f64 / 1024.0,
    );
}

fn print_totals(results: &[EvalResult]) {
    if results.is_empty() {
        return;
    }

    let total_input: usize = results.iter().map(|r| r.input_tok.heuristic).sum();
    let total_sem: usize = results.iter().map(|r| r.semantic_tok.heuristic).sum();
    let total_cmp: usize = results.iter().map(|r| r.compressed_tok.heuristic).sum();
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

    let total_lossless: usize = results.iter().map(|r| r.lossless_tok.heuristic).sum();
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

// ── JSON mode ─────────────────────────────────────────────────────────────────

/// Aggregates computed from the per-file BPE (real tokenizer) measurements.
///
/// These are the *honest* numbers. The heuristic counterparts are included for
/// comparison so the inflation gap is visible.
#[derive(Debug, serde::Serialize)]
struct JsonSummary {
    documents: usize,
    // Real BPE (cl100k) totals — the basis for the composite score.
    input_tokens_bpe: usize,
    semantic_tokens_bpe: usize,
    compressed_tokens_bpe: usize,
    lossless_tokens_bpe: usize,
    // Heuristic totals — shown to expose the self-referential inflation.
    input_tokens_heuristic: usize,
    semantic_tokens_heuristic: usize,
    compressed_tokens_heuristic: usize,
    semantic_reduction_bpe_pct: f64,
    semantic_reduction_heuristic_pct: f64,
    compressed_reduction_bpe_pct: f64,
    compressed_reduction_heuristic_pct: f64,
    lossless_coverage_pct: f64,
    throughput_tok_per_ms: f64,
    /// 0.0–1.0 composite quality score (see `composite_score`).
    composite: f64,
}

impl JsonSummary {
    fn from_results(all: &[&EvalResult]) -> Self {
        let input_bpe: usize = all.iter().map(|r| r.input_tok.bpe.unwrap_or(0)).sum();
        let sem_bpe: usize = all.iter().map(|r| r.semantic_tok.bpe.unwrap_or(0)).sum();
        let cmp_bpe: usize = all.iter().map(|r| r.compressed_tok.bpe.unwrap_or(0)).sum();
        let los_bpe: usize = all.iter().map(|r| r.lossless_tok.bpe.unwrap_or(0)).sum();

        let input_h: usize = all.iter().map(|r| r.input_tok.heuristic).sum();
        let sem_h: usize = all.iter().map(|r| r.semantic_tok.heuristic).sum();
        let cmp_h: usize = all.iter().map(|r| r.compressed_tok.heuristic).sum();

        let sem_us: u128 = all.iter().map(|r| r.semantic_us).sum();
        let throughput = if sem_us > 0 {
            input_h as f64 / sem_us as f64 * 1000.0
        } else {
            0.0
        };
        let coverage = if all.is_empty() {
            0.0
        } else {
            all.iter().map(|r| r.lossless_word_coverage).sum::<f64>() / all.len() as f64
        };

        let sem_red_bpe = pct(sem_bpe, input_bpe);
        let sem_red_h = pct(sem_h, input_h);
        let cmp_red_bpe = pct(cmp_bpe, input_bpe);
        let cmp_red_h = pct(cmp_h, input_h);

        let composite = composite_score(
            sem_red_bpe,
            cmp_red_bpe,
            coverage,
            throughput,
            los_bpe,
            input_bpe,
        );

        Self {
            documents: all.len(),
            input_tokens_bpe: input_bpe,
            semantic_tokens_bpe: sem_bpe,
            compressed_tokens_bpe: cmp_bpe,
            lossless_tokens_bpe: los_bpe,
            input_tokens_heuristic: input_h,
            semantic_tokens_heuristic: sem_h,
            compressed_tokens_heuristic: cmp_h,
            semantic_reduction_bpe_pct: sem_red_bpe,
            semantic_reduction_heuristic_pct: sem_red_h,
            compressed_reduction_bpe_pct: cmp_red_bpe,
            compressed_reduction_heuristic_pct: cmp_red_h,
            lossless_coverage_pct: coverage,
            throughput_tok_per_ms: throughput,
            composite,
        }
    }
}

/// Computes a 0.0–1.0 composite score from the *BPE* (real tokenizer) numbers.
///
/// Components (each normalized to 0.0–1.0):
/// - `reduction`: semantic BPE reduction, saturated at 40% (0.40 → 1.0).
///   40% is the README's headline claim; reaching it earns full credit.
/// - `coverage`: lossless word coverage / 100. Fidelity floor — compression
///   that destroys content cannot score well here.
/// - `throughput`: tok/ms, log-scaled and saturated at 1000 tok/ms (≈33× the
///   Python baseline). Rewards speed without letting it dominate.
/// - `lossless_overhead`: penalizes if Lossless mode *adds* tokens vs input
///   (bridge-format structural overhead). 0% overhead → 1.0.
///
/// Weights: reduction 0.40 · coverage 0.30 · throughput 0.15 · lossless 0.15.
fn composite_score(
    sem_reduction_bpe_pct: f64,
    _cmp_reduction_bpe_pct: f64,
    coverage_pct: f64,
    throughput_tok_per_ms: f64,
    lossless_tokens_bpe: usize,
    input_tokens_bpe: usize,
) -> f64 {
    let reduction = (sem_reduction_bpe_pct / 40.0).clamp(0.0, 1.0);
    let coverage = (coverage_pct / 100.0).clamp(0.0, 1.0);
    // log10-scaled throughput: 1 tok/ms → 0.0, 10 → 0.5, 1000 → 1.0 (saturated).
    let throughput = if throughput_tok_per_ms > 0.0 {
        ((throughput_tok_per_ms).log10() / 3.0).clamp(0.0, 1.0)
    } else {
        0.0
    };
    // Lossless should not inflate token count. overhead > 0 → penalty.
    let lossless_ratio = if input_tokens_bpe == 0 {
        1.0
    } else {
        lossless_tokens_bpe as f64 / input_tokens_bpe as f64
    };
    // ratio 1.0 (no change) → 1.0; 1.2 (20% bloat) → 0.8; clamp at [0,1].
    let lossless = (2.0 - lossless_ratio).clamp(0.0, 1.0);

    0.40 * reduction + 0.30 * coverage + 0.15 * throughput + 0.15 * lossless
}

fn print_json(summary: &JsonSummary) {
    // Single JSON object on stdout — `epic eval` parses `composite`.
    match serde_json::to_string_pretty(summary) {
        Ok(s) => println!("{s}"),
        Err(e) => eprintln!("eval: failed to serialize JSON summary: {e}"),
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    let json_mode = std::env::args().any(|a| a == "--json");

    let base = concat!(env!("CARGO_MANIFEST_DIR"), "/eval");

    let mut md_files: Vec<String> = Vec::new();
    for dir in &[
        format!("{base}/dataset/policy"),
        format!("{base}/dataset/hf"),
        format!("{base}/dataset/multilingual"),
    ] {
        md_files.extend(collect_files(dir, "md"));
    }
    let html_files = collect_files(&format!("{base}/dataset/html"), "html");
    let txt_files = collect_files(&format!("{base}/dataset/plaintext"), "txt");

    let mut all_results: Vec<EvalResult> = Vec::new();

    if !json_mode {
        // ── Human-readable sections ─────────────────────────────────────────
        println!("\n▶ Markdown — policy + HuggingFace docs");
        print_header();
    }
    let mut md_results: Vec<EvalResult> = Vec::new();
    for f in &md_files {
        if let Some(r) = eval_file(f, InputFormat::Markdown) {
            if !json_mode {
                print_row(&r);
            }
            md_results.push(r);
        }
    }
    if !json_mode {
        print_totals(&md_results);
    }
    all_results.extend(md_results);

    if !html_files.is_empty() {
        if !json_mode {
            println!("\n▶ HTML");
            print_header();
        }
        let mut html_results: Vec<EvalResult> = Vec::new();
        for f in &html_files {
            if let Some(r) = eval_file(f, InputFormat::Html) {
                if !json_mode {
                    print_row(&r);
                }
                html_results.push(r);
            }
        }
        if !json_mode {
            print_totals(&html_results);
        }
        all_results.extend(html_results);
    }

    if !txt_files.is_empty() {
        if !json_mode {
            println!("\n▶ PlainText");
            print_header();
        }
        let mut txt_results: Vec<EvalResult> = Vec::new();
        for f in &txt_files {
            if let Some(r) = eval_file(f, InputFormat::PlainText) {
                if !json_mode {
                    print_row(&r);
                }
                txt_results.push(r);
            }
        }
        if !json_mode {
            print_totals(&txt_results);
        }
        all_results.extend(txt_results);
    }

    let refs: Vec<&EvalResult> = all_results.iter().collect();

    if json_mode {
        let summary = JsonSummary::from_results(&refs);
        print_json(&summary);
        return;
    }

    // ── Human-readable combined summary ────────────────────────────────────
    let summary = JsonSummary::from_results(&refs);
    println!("\n📊 Summary (all formats):");
    println!("  • Documents evaluated:      {}", summary.documents);
    println!(
        "  • Semantic reduction (BPE):       {:.1}%",
        summary.semantic_reduction_bpe_pct
    );
    println!(
        "  • Semantic reduction (heuristic): {:.1}%  ← self-referential, inflated",
        summary.semantic_reduction_heuristic_pct
    );
    println!(
        "  • Compressed reduction (BPE):     {:.1}%",
        summary.compressed_reduction_bpe_pct
    );
    println!(
        "  • Compressed reduction (heuristic): {:.1}%",
        summary.compressed_reduction_heuristic_pct
    );
    println!(
        "  • Lossless word coverage:   {:.1}% avg",
        summary.lossless_coverage_pct
    );
    println!(
        "  • Throughput (Semantic):    {:.0} tok/ms  [release build]",
        summary.throughput_tok_per_ms
    );
    println!(
        "  • Composite score:          {:.3} / 1.0  (BPE-based)",
        summary.composite
    );
}
