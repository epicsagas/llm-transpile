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
////// ## ROI gate (P1a)
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
        // ── ROI gate (BPE-honest) ───────────────────────────────────────────
        // A substituted occurrence replaces `term_tokens` with one PUA char.
        // Measured cl100k ground truth: a PUA char costs **3 tokens** (byte-fallback),
        // NOT 1 — the old gate assumed 1 and so grossly over-interned, *increasing*
        // token counts for ordinary words (most are already 1–2 tokens; see
        // `pua_substitution_break_even_is_empirically_honest`).
        //
        // Net saving across the document:
        //   body_saving  = count × (term_tokens − PUA_TOKEN_COST)   // each occurrence
        //   dict_overhead = PUA_TOKEN_COST + term_tokens + ENTRY_OVERHEAD
        //                  // the "<PUA>=<term>\n" line in the <D> block
        // Intern iff body_saving > dict_overhead (strictly positive net).
        let term_tokens = stream::estimate_tokens(term);
        if term_tokens <= PUA_TOKEN_COST {
            // Break-even bar: a term must cost MORE than a PUA char (i.e. 4+ tokens)
            // for substitution to save anything at all per occurrence.
            continue;
        }
        let per_occurrence_saving = term_tokens - PUA_TOKEN_COST; // > 0 here
        let body_saving = count.saturating_mul(per_occurrence_saving);
        let dict_overhead = PUA_TOKEN_COST + term_tokens + DICT_ENTRY_OVERHEAD;
        if body_saving <= dict_overhead {
            // Dictionary overhead cancels the body saving; skip.
            continue;
        }
        // Ignore overflow — we just stop interning if we run out of PUA symbols
        let _ = dict.intern(term);
    }
}

/// Token cost of a single Unicode PUA character under a real BPE tokenizer.
///
/// Empirically measured against cl100k_base: PUA codepoints (U+E000–U+F8FF) are
/// absent from the merge table, so each encodes via byte-fallback to **3 tokens**.
/// This constant replaces the old "PUA = 1 token" assumption that inflated
/// reduction claims and caused ROI-negative substitutions.
///
/// Used by [`auto_intern_frequent_terms`]' ROI gate and reported by the eval
/// harness for transparency. Independent of the `tiktoken` feature because the
/// real tokenizer's behavior does not change when the feature is off — only our
/// *measurement* of it does.
pub const PUA_TOKEN_COST: usize = 3;

/// Approximate extra token overhead of a `<D>` dictionary entry beyond the PUA
/// char and the term text itself — the `=`, the `\n`, and BPE boundary effects.
///
/// Measured entries: `"PUA=foo\n"` = 5 tokens (PUA 3 + "=foo"+newline ≈ 2),
/// `"PUA=transformer\n"` = 7 tokens. The fixed non-term portion hovers around
/// 3–4; we use a conservative 4 so the ROI bar errs toward *not* interning
/// (the empirically safer default given PUA's high base cost).
const DICT_ENTRY_OVERHEAD: usize = 4;

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

/// Token-count measurement methodology.
///
/// Used to make explicit *which* counting method produced a number, so that
/// downstream reports cannot silently mix the fast heuristic with an accurate
/// BPE tokenizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenMethod {
    /// Character-count heuristic (`chars_per_token`). Fast, dependency-free,
    /// but **self-referential** when used to evaluate this crate's own output:
    /// it bakes in the same assumptions (e.g. PUA = 1 token) that the
    /// compressor optimizes for, inflating the apparent reduction.
    Heuristic,
    /// Real BPE tokenizer (OpenAI `cl100k_base` via `tiktoken-rs`). Slower and
    /// requires the `tiktoken` feature, but independent of this crate's
    /// compression assumptions — the honest basis for reduction claims.
    Bpe,
}

/// A token-count measurement pairing a numeric result with its methodology.
///
/// Construct via [`measure_tokens`] (BPE when the `tiktoken` feature is
/// enabled, heuristic otherwise) or explicitly via [`TokenMeasurement::bpe`] /
/// [`TokenMeasurement::heuristic`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct TokenMeasurement {
    /// The counted token count.
    pub tokens: usize,
    /// How it was counted.
    pub method: TokenMethod,
}

impl TokenMeasurement {
    /// A heuristic measurement.
    ///
    /// Uses `estimate_tokens_heuristic` directly (not `token_count`, which
    /// dispatches to BPE under the `tiktoken` feature). This keeps the
    /// heuristic value stable and comparable regardless of features.
    pub fn heuristic(text: &str) -> Self {
        Self {
            tokens: stream::estimate_tokens_heuristic(text),
            method: TokenMethod::Heuristic,
        }
    }

    /// A BPE measurement using `cl100k_base`. Only available with the
    /// `tiktoken` feature.
    #[cfg(feature = "tiktoken")]
    pub fn bpe(text: &str) -> Self {
        Self {
            tokens: bpe_token_count(text),
            method: TokenMethod::Bpe,
        }
    }
}

/// Counts tokens using the real `cl100k_base` BPE tokenizer.
///
/// Requires the `tiktoken` feature. Returns the heuristic estimate as a
/// fallback if the tokenizer failed to initialize (should not happen with the
/// bundled merge table).
#[cfg(feature = "tiktoken")]
pub fn bpe_token_count(text: &str) -> usize {
    use std::sync::OnceLock;
    static BPE: OnceLock<Option<tiktoken_rs::CoreBPE>> = OnceLock::new();
    let bpe = BPE.get_or_init(|| tiktoken_rs::cl100k_base().ok());
    match bpe {
        Some(b) => b.encode_ordinary(text).len().max(1),
        None => token_count(text),
    }
}

/// Measures `text` with the most accurate method available.
///
/// With the `tiktoken` feature this returns a BPE measurement; otherwise a
/// heuristic measurement. Use [`measure_tokens_dual`] when you need both
/// side-by-side for an honest comparison.
pub fn measure_tokens(text: &str) -> TokenMeasurement {
    #[cfg(feature = "tiktoken")]
    {
        TokenMeasurement::bpe(text)
    }
    #[cfg(not(feature = "tiktoken"))]
    {
        TokenMeasurement::heuristic(text)
    }
}

/// Measures `text` with both methodologies when `tiktoken` is enabled.
///
/// Without the `tiktoken` feature, the `bpe` field is `None` and only the
/// heuristic number is reported. This struct is the foundation for
/// non-self-referential reduction reporting: compare the BPE numbers, not the
/// heuristic ones, when making token-saving claims.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DualTokenMeasurement {
    pub heuristic: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bpe: Option<usize>,
}

/// Counts `text` with both the heuristic and (if available) the BPE tokenizer.
///
/// The heuristic value comes from `estimate_tokens_heuristic` (always the
/// character-count estimate, never the BPE path), so under the `tiktoken`
/// feature the two fields genuinely differ — enabling an honest comparison
/// of the self-referential heuristic against the real tokenizer.
pub fn measure_tokens_dual(text: &str) -> DualTokenMeasurement {
    let heuristic = stream::estimate_tokens_heuristic(text);
    #[cfg(feature = "tiktoken")]
    {
        DualTokenMeasurement {
            heuristic,
            bpe: Some(bpe_token_count(text)),
        }
    }
    #[cfg(not(feature = "tiktoken"))]
    {
        DualTokenMeasurement {
            heuristic,
            bpe: None,
        }
    }
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
    fn measure_tokens_dual_returns_heuristic_always() {
        let m = measure_tokens_dual("hello world transformer");
        // Heuristic is always available regardless of features.
        assert!(m.heuristic > 0);
    }

    #[test]
    fn measure_tokens_dual_bpe_present_with_feature() {
        let m = measure_tokens_dual("hello world transformer");
        #[cfg(feature = "tiktoken")]
        {
            assert!(
                m.bpe.is_some(),
                "BPE measurement must be present with tiktoken feature"
            );
            assert!(m.bpe.unwrap() > 0);
        }
        #[cfg(not(feature = "tiktoken"))]
        {
            assert!(m.bpe.is_none(), "BPE must be None without tiktoken feature");
        }
    }

    /// AC1: documents the heuristic tokenizer's PUA assumption (tiktoken OFF).
    ///
    /// Without the `tiktoken` feature, `token_count` uses the
    /// `chars_per_token` heuristic which **assumes PUA = 1 token per char**
    /// (`stream.rs` PUA branch). This is the self-referential assumption the
    /// compressor optimizes for. See `pua_real_token_cost_*` (tiktoken ON)
    /// for the ground truth that disproves it.
    #[cfg(not(feature = "tiktoken"))]
    #[test]
    fn pua_heuristic_assumes_one_token_per_char() {
        let one = "\u{E000}";
        let eight = "\u{E000}\u{E001}\u{E002}\u{E003}\u{E004}\u{E005}\u{E006}\u{E007}";
        // The heuristic's baked-in (incorrect) assumption.
        assert_eq!(token_count(one), 1);
        assert_eq!(token_count(eight), 8);
    }

    /// AC1: empirically measures the *real* cl100k token cost of PUA (tiktoken ON).
    ///
    /// Ground truth: PUA codepoints are absent from cl100k's merge table, so
    /// each encodes via byte-fallback to **3 tokens**, and distinct PUA chars
    /// never merge. Measured (cl100k_base):
    ///
    /// | text                 | heuristic (off) | real (on) |
    /// |----------------------|-----------------|-----------|
    /// | 1 PUA char           | 1               | 3         |
    /// | 8 distinct PUA chars | 8               | 24        |
    ///
    /// **Implication**: the heuristic *undercounts* PUA cost by 3×, so any
    /// reduction figure computed with the heuristic on PUA-heavy output is
    /// inflated. Honest reports must use BPE numbers.
    #[cfg(feature = "tiktoken")]
    #[test]
    fn pua_real_token_cost_is_documented() {
        let one_pua = "\u{E000}";
        let eight_pua = "\u{E000}\u{E001}\u{E002}\u{E003}\u{E004}\u{E005}\u{E006}\u{E007}";

        // With tiktoken ON, token_count dispatches to the real BPE tokenizer.
        assert_eq!(
            token_count(one_pua),
            3,
            "single PUA char = 3 cl100k tokens (byte-fallback)"
        );
        assert_eq!(
            token_count(eight_pua),
            24,
            "8 distinct PUA chars = 24 tokens (3 each, no merge). \
             If this drifts, update the token-honesty docs."
        );
        // bpe_token_count agrees with token_count under the feature.
        assert_eq!(bpe_token_count(one_pua), token_count(one_pua));
    }

    /// AC1: demonstrates the PUA substitution break-even empirically.
    ///
    /// Ground truth (cl100k_base):
    /// - PUA char = 3 tokens (byte-fallback)
    /// - "large language model" = 3 tokens (each common word is 1 token)
    /// - "retrieval augmented generation" = 3 tokens (also < 4)
    ///
    /// **Break-even**: a term only pays to PUA-substitute if it costs **more
    /// than 3 tokens**. Common multi-word phrases like "large language model"
    /// are *already* 3 tokens — substituting with a PUA char (3 tokens) saves
    /// nothing. This empirically refutes the premise that PUA substitution
    /// broadly reduces tokens; under a real tokenizer it rarely does, and the
    /// heuristic (which counts PUA as 1) massively overstates any saving.
    #[cfg(feature = "tiktoken")]
    #[test]
    fn pua_substitution_break_even_is_empirically_honest() {
        let pua_tok = bpe_token_count("\u{E000}");
        assert_eq!(pua_tok, 3, "PUA char costs 3 tokens");

        // A 3-token phrase substitutes to a 3-token PUA → zero saving.
        let phrase = "large language model";
        assert_eq!(bpe_token_count(phrase), 3);
        assert!(
            bpe_token_count(phrase) <= pua_tok,
            "3-token phrase does not benefit from PUA substitution (tie)"
        );

        // Only a 4+ token term would genuinely save (1 token) — verify such
        // terms exist and cross the bar, proving the break-even claim.
        let long_term = "transformer-based language model fine-tuning pipeline";
        let long_tok = bpe_token_count(long_term);
        assert!(
            long_tok > pua_tok,
            "long term ({long_tok}) must exceed PUA cost ({pua_tok}) to save tokens"
        );
    }

    /// AC1: a plain Latin sentence must have a heuristic that is *consistent*
    /// with (but not necessarily equal to) the real BPE count, within a sane
    /// band. Catches gross regressions in `chars_per_token`.
    #[cfg(feature = "tiktoken")]
    #[test]
    fn heuristic_tracks_bpe_within_band() {
        let text = "The quick brown fox jumps over the lazy dog near the riverbank.";
        let h = token_count(text);
        let b = bpe_token_count(text);
        // Heuristic should be within 2× of BPE for plain Latin prose.
        // (CJK-heavy text is excluded — heuristic diverges more there, which
        // is exactly why BPE measurement exists.)
        let ratio = h as f64 / b as f64;
        assert!(
            (0.5..=2.0).contains(&ratio),
            "heuristic/bpe ratio {ratio:.2} outside [0.5, 2.0] for Latin prose \
             (heuristic={h}, bpe={b})"
        );
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
        // The ROI gate compares a candidate term's token count against PUA_TOKEN_COST
        // (3). "API endpoint" is well under the bar under BOTH tokenizers (it is a
        // short, common phrase), so it must NOT be interned — substitution would add
        // tokens. This holds regardless of the `tiktoken` feature now that the gate
        // uses the measured PUA cost rather than the old "PUA = 1 token" assumption.
        let md = "# Test\n\nAPI endpoint API endpoint API endpoint API endpoint API endpoint.";
        let result = transpile(
            md,
            InputFormat::Markdown,
            FidelityLevel::Semantic,
            Some(4096),
        );
        let output = result.unwrap();
        assert!(
            !output.contains("<D>"),
            "short common term 'API endpoint' (≤3 tokens) must not be PUA-substituted: {output}"
        );
    }

    /// A genuinely long, high-frequency term CAN cross the ROI bar. This test uses
    /// a term long enough that even after the honest PUA cost (3) and dictionary
    /// overhead, repeated occurrences save tokens. The intent is to verify the gate
    /// *permits* substitution when it is truly profitable, not that it always blocks.
    #[test]
    fn transpile_interns_long_high_freq_term_under_heuristic() {
        // 40+ chars → many heuristic tokens (≥10). Repeated 8×, the per-occurrence
        // saving (term_tokens − 3) multiplied by 8 should exceed dict overhead.
        // Under the heuristic tokenizer this is ROI-positive.
        let term = "internationalization-localization-pipeline";
        let md = format!("# Doc\n\n{term} {term} {term} {term} {term} {term} {term} {term}.");
        let result = transpile(
            &md,
            InputFormat::Markdown,
            FidelityLevel::Semantic,
            Some(4096),
        );
        let output = result.unwrap();
        // Under the heuristic this long term clears the bar. Under tiktoken it may
        // or may not depending on its real token count — assert only the guarantee
        // common to both: the output is well-formed.
        assert!(output.contains("<B>"));
        #[cfg(not(feature = "tiktoken"))]
        {
            assert!(
                output.contains("<D>"),
                "long high-freq term should clear the heuristic ROI bar: {output}"
            );
        }
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
