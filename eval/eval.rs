/// examples/eval.rs — llm-transpiler quantitative evaluation
///
/// Metrics:
///   - Token reduction rate (per fidelity level)
///   - Throughput (tok/ms)
///   - Lossless integrity verification
///   - Streaming TTFT (time to first chunk)
use llm_transpiler::{FidelityLevel, InputFormat, token_count, transpile};
use std::fs;
use std::time::Instant;

#[derive(Debug)]
struct Result {
    file: String,
    input_bytes: usize,
    input_tok: usize,
    semantic_tok: usize,
    compressed_tok: usize,
    lossless_tok: usize,
    semantic_ms: u128,
    compressed_ms: u128,
    lossless_ok: bool,
}

fn eval_file(path: &str) -> Option<Result> {
    let content = fs::read_to_string(path).ok()?;
    let input_tok = token_count(&content);

    // Semantic
    let t0 = Instant::now();
    let sem = transpile(
        &content,
        InputFormat::Markdown,
        FidelityLevel::Semantic,
        Some(4096),
    )
    .ok()?;
    let semantic_ms = t0.elapsed().as_millis();
    let semantic_tok = token_count(&sem);

    // Compressed
    let t0 = Instant::now();
    let cmp = transpile(
        &content,
        InputFormat::Markdown,
        FidelityLevel::Compressed,
        Some(2048),
    )
    .ok()?;
    let compressed_ms = t0.elapsed().as_millis();
    let compressed_tok = token_count(&cmp);

    // Lossless — verify key word preservation
    let los = transpile(
        &content,
        InputFormat::Markdown,
        FidelityLevel::Lossless,
        None,
    )
    .ok()?;
    let lossless_tok = token_count(&los);
    // All 3 sampled words from the source must appear in the output
    let sample_words: Vec<&str> = content
        .split_whitespace()
        .filter(|w| w.len() > 5 && w.chars().all(|c| c.is_alphabetic()))
        .take(3)
        .collect();
    let lossless_ok = sample_words.iter().all(|w| los.contains(w));

    let fname = std::path::Path::new(path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    Some(Result {
        file: fname,
        input_bytes: content.len(),
        input_tok,
        semantic_tok,
        compressed_tok,
        lossless_tok,
        semantic_ms,
        compressed_ms,
        lossless_ok,
    })
}

fn pct(a: usize, b: usize) -> f64 {
    if b == 0 {
        return 0.0;
    }
    100.0 - (a as f64 / b as f64 * 100.0)
}

fn main() {
    let base = concat!(env!("CARGO_MANIFEST_DIR"), "/eval");

    let mut files: Vec<String> = Vec::new();
    for dir in &[
        format!("{base}/dataset/policy"),
        format!("{base}/dataset/hf"),
    ] {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("md") {
                    if let Some(s) = path.to_str() {
                        files.push(s.to_string());
                    }
                }
            }
        }
    }
    files.sort();

    println!(
        "{:<36} {:>6} {:>8} {:>8} {:>8} {:>8} {:>8} {:>7} {:>7} {:>8}",
        "file",
        "in_tok",
        "Sem%red",
        "Cmp%red",
        "Sem_ms",
        "Cmp_ms",
        "tok/ms",
        "Loss%red",
        "Loss_ok",
        "in_KB"
    );
    println!("{}", "-".repeat(110));

    let mut total_input = 0usize;
    let mut total_semantic = 0usize;
    let mut total_compressed = 0usize;
    let mut total_sem_ms = 0u128;
    let mut total_cmp_ms = 0u128;
    let mut lossless_pass = 0usize;
    let mut count = 0usize;

    for f in &files {
        if let Some(r) = eval_file(f) {
            let tokms = if r.semantic_ms > 0 {
                r.input_tok as f64 / r.semantic_ms as f64
            } else {
                r.input_tok as f64
            };
            println!(
                "{:<36} {:>6} {:>8.1} {:>8.1} {:>8} {:>8} {:>7.0} {:>8.1} {:>8} {:>8.1}",
                r.file,
                r.input_tok,
                pct(r.semantic_tok, r.input_tok),
                pct(r.compressed_tok, r.input_tok),
                r.semantic_ms,
                r.compressed_ms,
                tokms,
                pct(r.lossless_tok, r.input_tok),
                if r.lossless_ok { "✓" } else { "✗" },
                r.input_bytes as f64 / 1024.0,
            );
            total_input += r.input_tok;
            total_semantic += r.semantic_tok;
            total_compressed += r.compressed_tok;
            total_sem_ms += r.semantic_ms;
            total_cmp_ms += r.compressed_ms;
            if r.lossless_ok {
                lossless_pass += 1;
            }
            count += 1;
        }
    }

    println!("{}", "═".repeat(110));
    let avg_sem_ms = if count > 0 {
        total_sem_ms / count as u128
    } else {
        0
    };
    let avg_cmp_ms = if count > 0 {
        total_cmp_ms / count as u128
    } else {
        0
    };
    let total_tokms = if total_sem_ms > 0 {
        total_input as f64 / total_sem_ms as f64
    } else {
        0.0
    };
    println!(
        "{:<36} {:>6} {:>8.1} {:>8.1} {:>8} {:>8} {:>7.0} {:>8} {:>8}",
        "total/avg",
        total_input,
        pct(total_semantic, total_input),
        pct(total_compressed, total_input),
        avg_sem_ms,
        avg_cmp_ms,
        total_tokms,
        "",
        format!("{lossless_pass}/{count}"),
    );

    println!("\n📊 Summary:");
    println!(
        "  • Semantic   avg reduction: {:.1}%",
        pct(total_semantic, total_input)
    );
    println!(
        "  • Compressed avg reduction: {:.1}%",
        pct(total_compressed, total_input)
    );
    println!("  • Lossless integrity:       {lossless_pass}/{count} passed");
    println!("  • Throughput (Semantic):    {total_tokms:.0} tok/ms");
    println!("  • Total input tokens:       {total_input}");
    println!("  • Total output (Semantic):  {total_semantic}");
}
