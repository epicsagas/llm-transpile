/// examples/test_docs.rs — integration validation of llm-transpiler with real documents
use llm_transpile::{FidelityLevel, InputFormat, transpile};
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

    let input_tokens = llm_transpile::token_count(&content);
    let t0 = Instant::now();
    match transpile(&content, format, fidelity, budget) {
        Ok(output) => {
            let output_tokens = llm_transpile::token_count(&output);
            let reduction = 100.0 - (output_tokens as f64 / input_tokens as f64 * 100.0);
            let elapsed = t0.elapsed().as_millis();
            println!(
                "✓ {path}\n  input {input_tokens} tok → output {output_tokens} tok  ({reduction:.1}% reduction)  {elapsed}ms\n  ---\n{}\n",
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

    println!("═══ Semantic / 4096 token budget ═══\n");
    let docs = [
        (
            format!("{base}/dataset/policy/01_auth_policy.md"),
            InputFormat::Markdown,
        ),
        (
            format!("{base}/dataset/policy/02_api_access.md"),
            InputFormat::Markdown,
        ),
        (
            format!("{base}/dataset/policy/03_data_retention.md"),
            InputFormat::Markdown,
        ),
        (
            format!("{base}/dataset/hf/hub-docs_security.md"),
            InputFormat::Markdown,
        ),
        (
            format!("{base}/dataset/hf/security-tokens.md"),
            InputFormat::Markdown,
        ),
        (
            format!("{base}/dataset/hf/transformers_CONTRIBUTING.md"),
            InputFormat::Markdown,
        ),
        (
            format!("{base}/dataset/hf/model-cards.md"),
            InputFormat::Markdown,
        ),
        (
            format!("{base}/dataset/hf/safetensors_README.md"),
            InputFormat::Markdown,
        ),
    ];

    for (path, fmt) in &docs {
        test_file(path, *fmt, FidelityLevel::Semantic, Some(4096));
    }

    println!("\n═══ Lossless (no information loss) ═══\n");
    test_file(
        &format!("{base}/dataset/policy/01_auth_policy.md"),
        InputFormat::Markdown,
        FidelityLevel::Lossless,
        None,
    );

    println!("\n═══ Compressed (maximum compression) / 1024 tokens ═══\n");
    test_file(
        &format!("{base}/dataset/hf/transformers_CONTRIBUTING.md"),
        InputFormat::Markdown,
        FidelityLevel::Compressed,
        Some(1024),
    );
}
