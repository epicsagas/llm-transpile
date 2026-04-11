/// examples/eval.rs — llm-transpiler 정량 평가
///
/// 평가 지표:
///   - 토큰 절감률 (fidelity 별)
///   - 처리 속도 (tok/ms)
///   - Lossless 무결성 검증
///   - 스트리밍 TTFT (첫 청크 도달 시간)
use llm_transpiler::{transpile, token_count, FidelityLevel, InputFormat};
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
    let sem = transpile(&content, InputFormat::Markdown, FidelityLevel::Semantic, Some(4096)).ok()?;
    let semantic_ms = t0.elapsed().as_millis();
    let semantic_tok = token_count(&sem);

    // Compressed
    let t0 = Instant::now();
    let cmp = transpile(&content, InputFormat::Markdown, FidelityLevel::Compressed, Some(2048)).ok()?;
    let compressed_ms = t0.elapsed().as_millis();
    let compressed_tok = token_count(&cmp);

    // Lossless — 주요 단어 보존 확인
    let los = transpile(&content, InputFormat::Markdown, FidelityLevel::Lossless, None).ok()?;
    let lossless_tok = token_count(&los);
    // 원문의 임의 단어 3개가 출력에 모두 포함되어야 함
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
    if b == 0 { return 0.0; }
    100.0 - (a as f64 / b as f64 * 100.0)
}

fn main() {
    let base = concat!(env!("CARGO_MANIFEST_DIR"), "/eval");
    let files = [
        format!("{base}/dataset/policy/01_auth_policy.md"),
        format!("{base}/dataset/policy/02_api_access.md"),
        format!("{base}/dataset/policy/03_data_retention.md"),
        format!("{base}/dataset/hf/hub-docs_security.md"),
        format!("{base}/dataset/hf/security-tokens.md"),
        format!("{base}/dataset/hf/datasets-cards.md"),
        format!("{base}/dataset/hf/repositories-getting-started.md"),
        format!("{base}/dataset/hf/spaces-overview.md"),
        format!("{base}/dataset/hf/model-cards.md"),
        format!("{base}/dataset/hf/safetensors_README.md"),
        format!("{base}/dataset/hf/transformers_CONTRIBUTING.md"),
    ];

    println!("{:<36} {:>6} {:>8} {:>8} {:>8} {:>8} {:>8} {:>7} {:>7} {:>8}",
        "파일", "입력tok", "Sem절감%", "Cmp절감%", "Sem_ms", "Cmp_ms", "tok/ms", "Loss절감%", "Loss무결", "입력KB");
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
            let tokms = if r.semantic_ms > 0 { r.input_tok as f64 / r.semantic_ms as f64 } else { r.input_tok as f64 };
            println!("{:<36} {:>6} {:>8.1} {:>8.1} {:>8} {:>8} {:>7.0} {:>8.1} {:>8} {:>8.1}",
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
            if r.lossless_ok { lossless_pass += 1; }
            count += 1;
        }
    }

    println!("{}", "═".repeat(110));
    let avg_sem_ms = if count > 0 { total_sem_ms / count as u128 } else { 0 };
    let avg_cmp_ms = if count > 0 { total_cmp_ms / count as u128 } else { 0 };
    let total_tokms = if total_sem_ms > 0 { total_input as f64 / total_sem_ms as f64 } else { 0.0 };
    println!("{:<36} {:>6} {:>8.1} {:>8.1} {:>8} {:>8} {:>7.0} {:>8} {:>8}",
        "합계/평균",
        total_input,
        pct(total_semantic, total_input),
        pct(total_compressed, total_input),
        avg_sem_ms,
        avg_cmp_ms,
        total_tokms,
        "",
        format!("{lossless_pass}/{count}"),
    );

    println!("\n📊 요약:");
    println!("  • Semantic   평균 절감: {:.1}%", pct(total_semantic, total_input));
    println!("  • Compressed 평균 절감: {:.1}%", pct(total_compressed, total_input));
    println!("  • Lossless 무결성:      {lossless_pass}/{count} 통과");
    println!("  • 처리 속도 (Semantic): {total_tokms:.0} tok/ms");
    println!("  • 총 입력 토큰:         {total_input}");
    println!("  • 총 출력 (Semantic):   {total_semantic}");
}
