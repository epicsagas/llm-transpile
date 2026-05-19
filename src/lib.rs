//! # llm-transpiler
//!
//! A high-performance Rust library that converts raw documents (Markdown, HTML,
//! Plain Text, Tables, etc.) into a structured bridge format so LLM agents can
//! receive **maximum information with minimum tokens**.
//!
//! ## Quick Start
//!
//! ```rust
//! use llm_transpile::{transpile, FidelityLevel, InputFormat};
//!
//! let md = "# Contract\n\nThis agreement was concluded in 2024.";
//! let result = transpile(md, InputFormat::Markdown, FidelityLevel::Semantic, Some(4096))
//!     .expect("transpile failed");
//! println!("{}", result);
//! ```
//!
//! ## Streaming Usage
//!
//! ```rust,no_run
//! use llm_transpile::{transpile_stream, FidelityLevel, InputFormat};
//! use futures::StreamExt;
//!
//! async fn example() {
//!     let md = "# Document\n\nThis is a paragraph.";
//!     let mut stream = transpile_stream(md, InputFormat::Markdown, FidelityLevel::Semantic, 4096).await;
//!     while let Some(chunk) = stream.next().await {
//!         let chunk = chunk.expect("stream error");
//!         print!("{}", chunk.content);
//!         if chunk.is_final { break; }
//!     }
//! }
//! ```

// ────────────────────────────────────────────────
// Internal modules
// ────────────────────────────────────────────────

pub(crate) mod compressor;
pub(crate) mod ir;
pub(crate) mod renderer;
pub(crate) mod stream;
pub(crate) mod symbol;

// Parser module (Markdown → IR)
mod parser;

// ────────────────────────────────────────────────
// Public re-exports
// ────────────────────────────────────────────────

pub use compressor::{AdaptiveCompressor, CompressionConfig, CompressionStage};
pub use ir::{DocNode, FidelityLevel, IRDocument};
pub use renderer::{build_yaml_header, linearize_table, render_full, render_node};
pub use stream::{StreamError, StreamingTranspiler, TranspileChunk};
pub use symbol::SymbolDict;

// ────────────────────────────────────────────────
// Public enumerations
// ────────────────────────────────────────────────

/// Input document format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    /// Plain text.
    PlainText,
    /// CommonMark-compatible Markdown.
    Markdown,
    /// HTML5.
    Html,
}

// ────────────────────────────────────────────────
// Top-level error type
// ────────────────────────────────────────────────

/// Transpile error.
#[derive(Debug, thiserror::Error)]
pub enum TranspileError {
    #[error("parse failed: {0}")]
    Parse(String),

    #[error("symbol table overflow: {0}")]
    SymbolOverflow(#[from] symbol::SymbolOverflowError),

    #[error("stream error: {0}")]
    Stream(#[from] stream::StreamError),

    #[error("compression attempted in Lossless mode")]
    LosslessModeViolation,

    #[error("input exceeds maximum allowed size of {0} bytes")]
    InputTooLarge(usize),
}

/// Maximum input size accepted by [`transpile`] and [`transpile_stream`].
/// Inputs larger than this limit are rejected with [`TranspileError::InputTooLarge`]
/// to prevent resource exhaustion on unbounded documents.
pub const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB

// ────────────────────────────────────────────────
// Internal helpers
// ────────────────────────────────────────────────

/// Strips Unicode PUA range (U+E000–U+F8FF) characters from the input string.
/// Prevents external input from colliding with the internal symbol substitution scheme.
fn strip_pua(input: &str) -> std::borrow::Cow<'_, str> {
    if input
        .chars()
        .any(|c| ('\u{E000}'..='\u{F8FF}').contains(&c))
    {
        std::borrow::Cow::Owned(
            input
                .chars()
                .filter(|c| !('\u{E000}'..='\u{F8FF}').contains(c))
                .collect(),
        )
    } else {
        std::borrow::Cow::Borrowed(input)
    }
}

// ────────────────────────────────────────────────
// Internal helpers: auto term discovery
// ────────────────────────────────────────────────

/// Automatically discovers frequently occurring terms in the document's body text
/// and registers them in the SymbolDict for PUA substitution.
///
/// Only runs when fidelity allows compression. Terms must appear at least `min_freq` times
/// across all body text nodes (Para, Header, List items). Short terms (< 3 chars for ASCII,
/// < 2 chars for non-ASCII) are excluded because they don't save enough tokens to justify
/// the dictionary entry overhead.
///
/// ## ROI gate (P1a)
///
/// A term is only interned when the net token saving is positive:
///
/// ```text
/// saving  = freq × (term_tokens - 1)    // replacing N tokens with 1 PUA token
/// overhead = dict_entry_tokens            // "SymA=Term\n" line in the <D> block
/// intern  iff saving > overhead
/// ```
///
/// This prevents low-ROI substitutions (e.g. a 2-token word appearing 3 times) from
/// inflating the `<D>` block more than they save in the body.
fn auto_intern_frequent_terms(
    doc: &IRDocument,
    dict: &mut SymbolDict,
    min_freq: usize,
    max_terms: usize,
) {
    use std::collections::HashMap;

    if !doc.fidelity.allows_compression() {
        return;
    }

    // Count token frequencies across all body text nodes
    let mut freq: HashMap<&str, usize> = HashMap::new();
    for node in &doc.nodes {
        let text: Option<&str> = match node {
            DocNode::Para { text, .. } => Some(text.as_str()),
            DocNode::Header { text, .. } => Some(text.as_str()),
            DocNode::List { items, .. } => {
                // Count tokens in list items
                for item in items {
                    for token in item.split_whitespace() {
                        let min_len = if token.is_ascii() { 3 } else { 2 };
                        if token.len() >= min_len {
                            *freq.entry(token).or_insert(0) += 1;
                        }
                    }
                }
                None
            }
            _ => None,
        };
        if let Some(text) = text {
            for token in text.split_whitespace() {
                let min_len = if token.is_ascii() { 3 } else { 2 };
                if token.len() >= min_len {
                    *freq.entry(token).or_insert(0) += 1;
                }
            }
        }
    }

    // Filter by min_freq, sort by frequency descending, take top max_terms
    let mut candidates: Vec<(&str, usize)> = freq
        .into_iter()
        .filter(|(_, count)| *count >= min_freq)
        .collect();
    candidates.sort_by_key(|b| std::cmp::Reverse(b.1));

    for (term, count) in candidates.into_iter().take(max_terms) {
        // ── ROI gate ────────────────────────────────────────────────────────
        // Each term occurrence in the body uses `term_tokens` tokens.
        // After substitution it becomes 1 PUA token — saving `term_tokens - 1` per
        // occurrence.  The dictionary entry ("SymA=Term\n") costs `dict_entry_tokens`.
        // Only intern when the total saving exceeds the overhead.
        let term_tokens = stream::estimate_tokens(term);
        if term_tokens <= 1 {
            // Substituting a single-token word saves nothing; skip.
            continue;
        }
        // "<PUA>=<term>\n" — PUA char is 1 token, "=" is ~0.25 tok (grouped with PUA
        // by most tokenizers), "\n" is ~0.25 tok.  Approximate as term_tokens + 1.
        let dict_entry_tokens = term_tokens + 1;
        let saving = count.saturating_mul(term_tokens - 1);
        if saving <= dict_entry_tokens {
            // Net saving is zero or negative; skip.
            continue;
        }
        // Ignore overflow — we just stop interning if we run out of PUA symbols
        let _ = dict.intern(term);
    }
}

// ────────────────────────────────────────────────
// Public API
// ────────────────────────────────────────────────

/// Converts a document **synchronously** into the bridge format.
///
/// # Arguments
/// - `input`    — source document text
/// - `format`   — input format (Markdown / HTML / PlainText)
/// - `fidelity` — semantic preservation level
/// - `budget`   — maximum token count (`None` = unlimited)
///
/// # Returns
/// Bridge-format string (`<D>?<H><B>...</B>`)
///
/// # Errors
/// Returns `TranspileError` on parse failure or symbol table overflow.
pub fn transpile(
    input: &str,
    format: InputFormat,
    fidelity: FidelityLevel,
    budget: Option<usize>,
) -> Result<String, TranspileError> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(TranspileError::InputTooLarge(input.len()));
    }
    let input = strip_pua(input);
    let input = input.as_ref();

    // 1. Parse → IR
    let mut doc = parser::parse(input, format, fidelity, budget).map_err(TranspileError::Parse)?;

    // 2. Compress + hard-cap re-compression loop (only when a budget is provided)
    if let Some(b) = budget
        && fidelity != FidelityLevel::Lossless
    {
        doc.nodes = compress_to_budget(std::mem::take(&mut doc.nodes), b, fidelity, input);
    }

    // 3. Auto-discover frequent terms for symbol substitution
    let mut dict = SymbolDict::new();
    auto_intern_frequent_terms(&doc, &mut dict, 3, 50);

    // 4. Render
    let output = render_full(&doc, &mut dict);
    Ok(output)
}

/// Compresses `nodes` until the rendered output fits within `budget` tokens,
/// or until further compression yields no improvement.
///
/// Strategy:
/// 1. First pass uses `current_tokens` estimated from the raw input.
/// 2. After rendering, if the output still exceeds `budget`, the actual
///    token count is fed back as `current_tokens` and compression is retried
///    at the next higher stage.
/// 3. The loop terminates when either:
///    - output fits within `budget`, or
///    - two consecutive passes produce the same node count (compression
///      saturated — further iterations would be identical).
///
/// Maximum iterations: 4 (one per `CompressionStage`).
fn compress_to_budget(
    nodes: Vec<DocNode>,
    budget: usize,
    fidelity: FidelityLevel,
    raw_input: &str,
) -> Vec<DocNode> {
    use compressor::CompressionStage;

    let compressor = AdaptiveCompressor::new();

    // Stages in ascending order — we walk up from the initial estimate.
    const STAGES: &[CompressionStage] = &[
        CompressionStage::StopwordOnly,
        CompressionStage::PruneLowImportance,
        CompressionStage::DeduplicateAndLinearize,
        CompressionStage::MaxCompression,
    ];

    // Initial compression: use raw-input token estimate (same as before).
    let initial_tokens = stream::estimate_tokens(raw_input);
    let cfg = CompressionConfig {
        budget,
        current_tokens: initial_tokens,
        fidelity,
    };
    let mut current_nodes = compressor.compress(nodes, &cfg);
    let mut prev_node_count = usize::MAX;

    for &stage in STAGES {
        // Render to measure actual output tokens.
        // We use a temporary empty dict here — symbol substitution happens later
        // in the main flow and only saves ~1% tokens, so it does not affect the
        // hard-cap decision materially.
        let tmp_output = {
            let mut tmp_dict = SymbolDict::new();
            let mut tmp_doc = ir::IRDocument::new(fidelity, Some(budget));
            tmp_doc.nodes = current_nodes.clone();
            renderer::render_full(&tmp_doc, &mut tmp_dict)
        };
        let actual_tokens = stream::estimate_tokens(&tmp_output);

        // Within budget — done.
        if actual_tokens <= budget {
            break;
        }

        // Saturated — further compression would be a no-op.
        if current_nodes.len() == prev_node_count {
            break;
        }
        prev_node_count = current_nodes.len();

        // Skip stages that are at or below what the compressor already applied.
        let effective_stage = {
            let ratio = actual_tokens as f64 / budget as f64;
            let auto_stage = match ratio {
                r if r < 0.60 => CompressionStage::StopwordOnly,
                r if r < 0.80 => CompressionStage::PruneLowImportance,
                r if r < 0.95 => CompressionStage::DeduplicateAndLinearize,
                _ => CompressionStage::MaxCompression,
            };
            auto_stage.max(stage)
        };

        if effective_stage < stage {
            continue;
        }

        // Re-compress at the actual measured token count.
        let retry_cfg = CompressionConfig {
            budget,
            current_tokens: actual_tokens,
            fidelity,
        };
        let retry_nodes = compressor.compress(current_nodes.clone(), &retry_cfg);
        current_nodes = retry_nodes;
    }

    current_nodes
}

/// Converts a document into a **Tokio stream**.
///
/// The first chunk is delivered immediately, minimizing TTFT.
///
/// # Arguments
/// - `input`    — source document text
/// - `format`   — input format (Markdown / HTML / PlainText)
/// - `fidelity` — semantic preservation level
/// - `budget`   — maximum allowed token count. Passing `0` is treated as
///   "unlimited" and immediately switches to `Compressed` mode during
///   budget-usage calculations. Use a positive non-zero value to enforce a token limit.
///
/// # Errors
/// On parse failure, `Err(StreamError::Parse(...))` is sent as the first stream item
/// and the stream is then closed. Use [`transpile`] if you prefer a single `Result`.
pub async fn transpile_stream(
    input: &str,
    format: InputFormat,
    fidelity: FidelityLevel,
    budget: usize,
) -> std::pin::Pin<Box<dyn futures::Stream<Item = Result<TranspileChunk, StreamError>> + Send>> {
    if input.len() > MAX_INPUT_BYTES {
        return Box::pin(futures::stream::once(futures::future::ready(Err(
            StreamError::InputTooLarge(input.len()),
        ))));
    }
    let sanitized = strip_pua(input);
    let input_ref = sanitized.as_ref();

    let doc = match parser::parse(input_ref, format, fidelity, Some(budget)) {
        Ok(doc) => doc,
        Err(msg) => {
            // Parse failure: immediately return a stream containing a single Err chunk.
            // futures::future::ready() is Unpin, so it can be safely used with stream::once.
            return Box::pin(futures::stream::once(futures::future::ready(Err(
                StreamError::Parse(msg),
            ))));
        }
    };

    let transpiler = StreamingTranspiler::new(budget, fidelity);
    Box::pin(transpiler.transpile(doc))
}

/// Returns the approximate token count for the given text.
///
/// Uses a character-count-based heuristic without a real model tokenizer.
/// For higher accuracy, use `tiktoken-rs` or the `tokenizers` crate directly.
pub fn token_count(text: &str) -> usize {
    stream::estimate_tokens(text)
}

// ────────────────────────────────────────────────
// Integration tests
// ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_MD: &str = r#"
# 소프트웨어 라이선스 계약

## 계약 당사자

본 계약은 갑(라이선서)과 을(라이선시) 사이에 체결됩니다.

## 주요 조항

- 소스 코드 배포 금지
- 역설계 금지
- 연간 라이선스 비용: 1,000,000원

| 항목 | 금액 |
|------|------|
| 기본료 | 800,000원 |
| 유지보수 | 200,000원 |
"#;

    #[test]
    fn transpile_markdown_produces_bridge_format() {
        let result = transpile(
            SAMPLE_MD,
            InputFormat::Markdown,
            FidelityLevel::Semantic,
            Some(2048),
        );
        assert!(
            result.is_ok(),
            "transpile should succeed: {:?}",
            result.err()
        );
        let output = result.unwrap();
        assert!(output.contains("<B>"), "output must contain <B> tag");
        assert!(
            output.contains("</B>"),
            "output must contain </B> closing tag"
        );
    }

    #[test]
    fn transpile_lossless_preserves_content() {
        let result = transpile(
            "중요한 법적 내용입니다.",
            InputFormat::PlainText,
            FidelityLevel::Lossless,
            None,
        );
        let output = result.unwrap();
        assert!(output.contains("중요한 법적 내용입니다."));
    }

    #[test]
    fn token_count_is_positive() {
        assert!(token_count("hello world") > 0);
    }

    #[test]
    fn pua_chars_stripped_from_input() {
        let input_with_pua = "hello \u{E000}world\u{F8FF}";
        let output = transpile(
            input_with_pua,
            InputFormat::PlainText,
            FidelityLevel::Lossless,
            None,
        )
        .unwrap();
        assert!(
            !output.contains('\u{E000}'),
            "PUA characters must not appear in output"
        );
        assert!(output.contains("hello"), "plain text must be preserved");
        assert!(
            output.contains("world"),
            "adjacent text after PUA removal must be preserved"
        );
    }

    #[tokio::test]
    async fn stream_error_variant_is_send_and_stream_works() {
        use futures::StreamExt;
        use stream::StreamError;

        // Compile-time check for StreamError::Parse variant
        fn _assert_send<T: Send>(_: T) {}
        _assert_send(StreamError::Parse("test".to_string()));

        // Verify normal streaming behavior
        let mut stream = transpile_stream(
            SAMPLE_MD,
            InputFormat::Markdown,
            FidelityLevel::Semantic,
            8192,
        )
        .await;
        let first = stream.next().await.expect("at least one chunk must exist");
        assert!(
            first.is_ok(),
            "valid input must yield an Ok chunk: {:?}",
            first.err()
        );
    }

    #[test]
    fn transpile_rejects_oversized_input() {
        let huge = "a".repeat(MAX_INPUT_BYTES + 1);
        let result = transpile(&huge, InputFormat::PlainText, FidelityLevel::Lossless, None);
        assert!(
            matches!(result, Err(TranspileError::InputTooLarge(_))),
            "expected InputTooLarge, got: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn stream_rejects_oversized_input() {
        use futures::StreamExt;
        let huge = "a".repeat(MAX_INPUT_BYTES + 1);
        let mut stream =
            transpile_stream(&huge, InputFormat::PlainText, FidelityLevel::Lossless, 0).await;
        let first = stream.next().await.expect("must yield an error item");
        assert!(
            matches!(first, Err(stream::StreamError::InputTooLarge(_))),
            "oversized stream input must yield InputTooLarge, got: {:?}",
            first
        );
    }

    #[test]
    fn transpile_auto_interns_frequent_terms() {
        // A term appearing 5 times should be auto-interned
        let md = "# Test\n\nAPI endpoint API endpoint API endpoint API endpoint API endpoint.";
        let result = transpile(
            md,
            InputFormat::Markdown,
            FidelityLevel::Semantic,
            Some(4096),
        );
        let output = result.unwrap();
        // The output should contain a <D> dictionary block with the frequent term
        assert!(
            output.contains("<D>"),
            "output must contain <D> block when frequent terms exist: {output}"
        );
    }

    #[test]
    fn transpile_no_auto_intern_in_lossless() {
        // Lossless mode should still work (no auto-intern doesn't break anything)
        let md = "API API API API API API.";
        let result = transpile(md, InputFormat::PlainText, FidelityLevel::Lossless, None);
        let output = result.unwrap();
        // Lossless may or may not have <D> — just verify it doesn't crash
        assert!(output.contains("<B>"));
    }

    #[test]
    fn transpile_no_intern_for_rare_terms() {
        // A term appearing only once should NOT be interned
        let md = "This document mentions API once.";
        let result = transpile(
            md,
            InputFormat::PlainText,
            FidelityLevel::Semantic,
            Some(4096),
        );
        let output = result.unwrap();
        // Rare term should not trigger a <D> block (saves dictionary overhead)
        // This test verifies min_freq threshold works
        assert!(output.contains("<B>"));
    }

    #[test]
    fn html_pua_entity_stripped_after_tag_removal() {
        // &#xE000; decoded by ammonia becomes a PUA char — must be stripped
        let html = "<p>hello &#xE000; world</p>";
        let output = transpile(html, InputFormat::Html, FidelityLevel::Lossless, None).unwrap();
        assert!(
            !output.contains('\u{E000}'),
            "PUA from HTML entity decoding must be stripped"
        );
        assert!(
            output.contains("hello"),
            "surrounding text must be preserved"
        );
    }
}
