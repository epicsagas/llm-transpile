//! # llm-transpiler
//!
//! A high-performance Rust library that converts raw documents (Markdown, HTML,
//! Plain Text, Tables, etc.) into a structured bridge format so LLM agents can
//! receive **maximum information with minimum tokens**.
//!
//! ## Quick Start
//!
//! ```rust
//! use llm_transpiler::{transpile, FidelityLevel, InputFormat};
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
//! use llm_transpiler::{transpile_stream, FidelityLevel, InputFormat};
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
}

// ────────────────────────────────────────────────
// Internal helpers
// ────────────────────────────────────────────────

/// Strips Unicode PUA range (U+E000–U+F8FF) characters from the input string.
/// Prevents external input from colliding with the internal symbol substitution scheme.
fn strip_pua(input: &str) -> std::borrow::Cow<'_, str> {
    if input.chars().any(|c| ('\u{E000}'..='\u{F8FF}').contains(&c)) {
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
    let input = strip_pua(input);
    let input = input.as_ref();

    // 1. Parse → IR
    let mut doc = parser::parse(input, format, fidelity, budget)
        .map_err(TranspileError::Parse)?;

    // 2. Compress (only when a budget is provided)
    if let Some(b) = budget {
        let compressor = AdaptiveCompressor::new();
        let cfg = CompressionConfig {
            budget: b,
            current_tokens: stream::estimate_tokens(input),
            fidelity,
        };
        doc.nodes = compressor.compress(std::mem::take(&mut doc.nodes), &cfg);
    }

    // 3. Render
    let mut dict = SymbolDict::new();
    let output = render_full(&doc, &mut dict);
    Ok(output)
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
    let sanitized = strip_pua(input);
    let input_ref = sanitized.as_ref();

    let doc = match parser::parse(input_ref, format, fidelity, Some(budget)) {
        Ok(doc) => doc,
        Err(msg) => {
            // Parse failure: immediately return a stream containing a single Err chunk.
            // futures::future::ready() is Unpin, so it can be safely used with stream::once.
            return Box::pin(futures::stream::once(futures::future::ready(
                Err(StreamError::Parse(msg)),
            )));
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
        let result = transpile(SAMPLE_MD, InputFormat::Markdown, FidelityLevel::Semantic, Some(2048));
        assert!(result.is_ok(), "transpile should succeed: {:?}", result.err());
        let output = result.unwrap();
        assert!(output.contains("<B>"), "output must contain <B> tag");
        assert!(output.contains("</B>"), "output must contain </B> closing tag");
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
        let output = transpile(input_with_pua, InputFormat::PlainText, FidelityLevel::Lossless, None).unwrap();
        assert!(!output.contains('\u{E000}'), "PUA characters must not appear in output");
        assert!(output.contains("hello"), "plain text must be preserved");
        assert!(output.contains("world"), "adjacent text after PUA removal must be preserved");
    }

    #[tokio::test]
    async fn stream_error_variant_is_send_and_stream_works() {
        use futures::StreamExt;
        use stream::StreamError;

        // Compile-time check for StreamError::Parse variant
        fn _assert_send<T: Send>(_: T) {}
        _assert_send(StreamError::Parse("test".to_string()));

        // Verify normal streaming behavior
        let mut stream = transpile_stream(SAMPLE_MD, InputFormat::Markdown, FidelityLevel::Semantic, 8192).await;
        let first = stream.next().await.expect("at least one chunk must exist");
        assert!(first.is_ok(), "valid input must yield an Ok chunk: {:?}", first.err());
    }
}
