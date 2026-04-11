/// examples/test_docs.rs — 실제 문서로 llm-transpiler 통합 검증
use llm_transpiler::{transpile, FidelityLevel, InputFormat};
use std::fs;
use std::time::Instant;

fn test_file(path: &str, format: InputFormat, fidelity: FidelityLevel, budget: Option<usize>) {
    let content = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("SKIP {path}: {e}");
            return;
        }
    };

    let input_tokens = llm_transpiler::token_count(&content);
    let t0 = Instant::now();
    match transpile(&content, format, fidelity, budget) {
        Ok(output) => {
            let output_tokens = llm_transpiler::token_count(&output);
            let reduction = 100.0 - (output_tokens as f64 / input_tokens as f64 * 100.0);
            let elapsed = t0.elapsed().as_millis();
            println!(
                "✓ {path}\n  입력 {input_tokens} tok → 출력 {output_tokens} tok  ({reduction:.1}% 절감)  {elapsed}ms\n  ---\n{}\n",
                output.lines().take(8).collect::<Vec<_>>().join("\n")
            );
        }
        Err(e) => {
            eprintln!("✗ {path}: {e:?}");
        }
    }
}

fn main() {
    let base = concat!(env!("CARGO_MANIFEST_DIR"), "/eval");

    println!("═══ Semantic / 4096 토큰 예산 ═══\n");
    let docs = [
        (format!("{base}/dataset/01_auth_policy.md"),    InputFormat::Markdown),
        (format!("{base}/dataset/02_api_access.md"),     InputFormat::Markdown),
        (format!("{base}/dataset/03_data_retention.md"), InputFormat::Markdown),
        (format!("{base}/hf_dataset/hub-docs_security.md"),        InputFormat::Markdown),
        (format!("{base}/hf_dataset/security-tokens.md"),          InputFormat::Markdown),
        (format!("{base}/hf_dataset/transformers_CONTRIBUTING.md"),InputFormat::Markdown),
        (format!("{base}/README.md"),                         InputFormat::Markdown),
        (format!("{base}/README.md"),                            InputFormat::Markdown),
        (format!("{base}/RESEARCH.md"),                       InputFormat::Markdown),
    ];

    for (path, fmt) in &docs {
        test_file(path, *fmt, FidelityLevel::Semantic, Some(4096));
    }

    println!("\n═══ Lossless (무손실) ═══\n");
    test_file(
        &format!("{base}/dataset/01_auth_policy.md"),
        InputFormat::Markdown,
        FidelityLevel::Lossless,
        None,
    );

    println!("\n═══ Compressed (최대 압축) / 1024 토큰 ═══\n");
    test_file(
        &format!("{base}/hf_dataset/transformers_CONTRIBUTING.md"),
        InputFormat::Markdown,
        FidelityLevel::Compressed,
        Some(1024),
    );
}
