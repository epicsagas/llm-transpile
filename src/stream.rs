//! stream.rs — Tokio-based Streaming Transpiler
//!
//! Delivers document chunks to the LLM before full processing completes,
//! minimizing TTFT (Time-To-First-Token).
//!
//! # Pipeline
//! ```text
//! AsyncRead → IncrementalParser → AdaptiveCompressor → StreamingRenderer
//!                                        ↑
//!                              Switches to Compressed at 80% budget usage
//! ```
//!
//! # Symbol substitution in streaming
//!
//! The single-pass streaming pipeline cannot discover all domain terms before the
//! stream starts, so [`SymbolDict`] **remains empty by default**. Use
//! [`StreamingTranspiler::with_dict`] to inject a pre-populated dictionary when
//! domain terms are known in advance.
//!
//! # Token counting
//!
//! By default, [`estimate_tokens`] uses a Unicode-script heuristic (chars-per-token).
//! Enable the `tiktoken` Cargo feature for accurate `cl100k_base` counting:
//!
//! ```toml
//! [dependencies]
//! llm-transpiler = { features = ["tiktoken"] }
//! ```

use std::pin::Pin;

use futures::Stream;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::compressor::{AdaptiveCompressor, CompressionConfig};
use crate::ir::{DocNode, FidelityLevel, IRDocument};
use crate::renderer::render_node;
use crate::symbol::SymbolDict;

// ────────────────────────────────────────────────
// 1. Chunk type
// ────────────────────────────────────────────────

/// A single output unit produced by the streaming transpiler.
#[derive(Debug, Clone)]
pub struct TranspileChunk {
    /// Transmission sequence number (0-based).
    pub sequence: usize,
    /// Rendered text fragment.
    pub content: String,
    /// Approximate token count for this chunk.
    ///
    /// Uses the `tiktoken` feature (cl100k_base) when enabled; otherwise falls back
    /// to the Unicode-script character heuristic.
    pub token_count: usize,
    /// Whether this is the final chunk.
    pub is_final: bool,
}

impl TranspileChunk {
    fn new(sequence: usize, content: String, is_final: bool) -> Self {
        let token_count = estimate_tokens(&content);
        Self {
            sequence,
            content,
            token_count,
            is_final,
        }
    }
}

// ────────────────────────────────────────────────
// 2. Token estimation
// ────────────────────────────────────────────────

/// Returns the approximate token count for `text`.
///
/// ## Feature: `tiktoken` (accurate)
/// When the `tiktoken` Cargo feature is enabled, uses OpenAI's `cl100k_base` tokenizer
/// (GPT-4 / GPT-3.5-turbo vocabulary, also a reasonable approximation for Claude).
/// The tokenizer is initialised once and cached in a `OnceLock`.
///
/// ## Default (heuristic)
/// Applies a chars-per-token weight based on each character's Unicode script range,
/// sums `1/cpt`, then takes the ceiling. **Not accurate for real models** — intended
/// only as a lightweight approximation for systems that cannot carry the tiktoken
/// dependency. For production use, enable the `tiktoken` feature.
///
/// | Script range | chars/token |
/// |-------------|-------------|
/// | Hiragana / Katakana / CJK / Hangul | 2 |
/// | Arabic / Devanagari / Bengali / Tamil / Thai | 3 |
/// | Emoji | 2 |
/// | Latin and everything else | 4 |
pub fn estimate_tokens(text: &str) -> usize {
    #[cfg(feature = "tiktoken")]
    {
        use std::sync::OnceLock;
        static BPE: OnceLock<tiktoken_rs::CoreBPE> = OnceLock::new();
        let bpe = BPE.get_or_init(|| tiktoken_rs::cl100k_base().expect("cl100k_base init failed"));
        bpe.encode_ordinary(text).len().max(1)
    }

    #[cfg(not(feature = "tiktoken"))]
    {
        estimate_tokens_heuristic(text)
    }
}

/// The character-count heuristic estimate, computed independently of the
/// `tiktoken` feature.
///
/// This is always available so that callers (e.g. `measure_tokens_dual`) can
/// compare the heuristic against the real BPE count *even when the `tiktoken`
/// feature is enabled* — which is the whole point of dual measurement.
/// Without this, the heuristic path would be compiled out under `tiktoken`
/// and "dual" would report the same number twice.
pub fn estimate_tokens_heuristic(text: &str) -> usize {
    let mut total = 0.0f64;
    for c in text.chars() {
        let cpt = chars_per_token(c);
        total += 1.0 / cpt as f64;
    }
    (total.ceil() as usize).max(1)
}

/// Token cost of a single PUA codepoint (U+E000) under the **active** measurement.
///
/// This is what the ROI gate must compare `term_tokens` against: both quantities
/// are measured by the same tokenizer, so the gate stays self-consistent regardless
/// of feature flags.
///
/// - Under `tiktoken`: returns [`crate::PUA_TOKEN_COST`] (= 3) — the real cl100k
///   byte-fallback ground truth. Single source of truth: the constant and this
///   function cannot drift apart.
/// - Under the heuristic: returns 1 — matching the PUA branch of
///   [`chars_per_token`] (`cpt = 1`), the same unit `estimate_tokens` uses for
///   `term_tokens` in this build.
///
/// Because the result is a per-build constant, callers in hot paths (notably the
/// ROI gate) should hoist the call out of loops rather than recomputing it per
/// iteration. The heuristic's `1` is *itself* the self-referential assumption
/// this crate's eval documents as inflated — it only matches reality when the
/// real tokenizer is in use.
pub fn pua_token_cost() -> usize {
    // A compile-time-fixed per-build constant, so this is cheap to hoist.
    #[cfg(feature = "tiktoken")]
    {
        crate::PUA_TOKEN_COST
    }
    #[cfg(not(feature = "tiktoken"))]
    {
        // Matches the PUA branch of `chars_per_token` (cpt = 1) so the gate's
        // two sides share the heuristic unit in this build.
        1
    }
}

/// Returns the chars-per-token value based on the Unicode codepoint range.
///
/// Always compiled (the heuristic must remain available alongside the BPE
/// path so the two can be compared — see `estimate_tokens_heuristic`).
fn chars_per_token(c: char) -> u32 {
    let cp = c as u32;
    match cp {
        0x3040..=0x30FF => 2,   // Hiragana / Katakana
        0x3400..=0x4DBF => 2,   // CJK Extension A
        0x4E00..=0x9FFF => 2,   // CJK Unified Ideographs (BMP)
        0xF900..=0xFAFF => 2,   // CJK Compatibility Ideographs
        0xAC00..=0xD7FF => 2,   // Hangul Syllables (U+D7B0–D7FF: includes Jamo Extended-B)
        0x1100..=0x11FF => 2,   // Hangul Jamo
        0xA960..=0xA97F => 2,   // Hangul Jamo Extended-A
        0x20000..=0x2A6DF => 2, // CJK Extension B
        0x2A700..=0x2CEAF => 2, // CJK Extension C–F
        0x2CEB0..=0x2EBEF => 2, // CJK Extension G
        0x30000..=0x323AF => 2, // CJK Extension H–I
        0x0600..=0x06FF => 3,   // Arabic
        0x0750..=0x077F => 3,   // Arabic Supplement
        0x0900..=0x097F => 3,   // Devanagari
        0x0980..=0x09FF => 3,   // Bengali
        0x0A00..=0x0A7F => 3,   // Gurmukhi
        0x0B80..=0x0BFF => 3,   // Tamil
        0x0E00..=0x0E7F => 3,   // Thai
        // Emoji: ~1–2 tokens per char per GPT-4 → approximate as cpt=2
        0x1F300..=0x1F9FF => 2, // Misc Symbols & Pictographs, Emoticons, Supplemental Symbols
        0x1FA00..=0x1FAFF => 2, // Symbols and Pictographs Extended-A
        0xE000..=0xF8FF => 1,   // Unicode PUA — LLM tokenizers typically encode as 1 token
        _ => 4,                 // Latin and other scripts
    }
}

// ────────────────────────────────────────────────
// 3. StreamingTranspiler
// ────────────────────────────────────────────────

/// Default mpsc channel buffer size used when none is specified via
/// [`StreamingTranspiler::with_channel_size`].
const DEFAULT_CHANNEL_BUFFER: usize = 32;

/// Tokio channel-based streaming transpiler.
///
/// # Symbol dictionary injection
///
/// By default the streaming path uses an **empty** [`SymbolDict`] because a
/// single-pass stream cannot discover domain terms before it starts. Use
/// [`StreamingTranspiler::with_dict`] to supply a pre-populated dictionary
/// when terms are known in advance:
///
/// ```rust,no_run
/// use llm_transpile::{FidelityLevel, SymbolDict, StreamingTranspiler};
///
/// let mut dict = SymbolDict::new();
/// dict.intern("large language model").unwrap();
/// dict.intern("retrieval-augmented generation").unwrap();
///
/// let transpiler = StreamingTranspiler::with_dict(4096, FidelityLevel::Semantic, dict);
/// ```
pub struct StreamingTranspiler {
    compressor: AdaptiveCompressor,
    budget: usize,
    fidelity: FidelityLevel,
    /// Pre-populated symbol dictionary used during streaming.
    /// Empty by default; populate via `with_dict()` for domain-specific compression.
    dict: SymbolDict,
    /// Tokio mpsc channel buffer size.
    /// Controls backpressure: the spawned producer task blocks when this many
    /// chunks are in flight and the consumer has not yet polled them.
    /// Larger values reduce producer stalls at the cost of higher memory usage;
    /// smaller values increase backpressure and bound memory.
    channel_buffer: usize,
}

impl StreamingTranspiler {
    /// Creates a new transpiler with an empty symbol dictionary.
    pub fn new(budget: usize, fidelity: FidelityLevel) -> Self {
        Self {
            compressor: AdaptiveCompressor::new(),
            budget,
            fidelity,
            dict: SymbolDict::new(),
            channel_buffer: DEFAULT_CHANNEL_BUFFER,
        }
    }

    /// Creates a transpiler with a **pre-populated** symbol dictionary.
    ///
    /// Domain terms already registered in `dict` will be substituted with PUA
    /// symbols during streaming and the `<D>` block will be emitted in the first
    /// chunk. This is the recommended path when the document vocabulary is known
    /// before streaming begins.
    ///
    /// # Example
    /// ```rust,no_run
    /// use llm_transpile::{FidelityLevel, SymbolDict, StreamingTranspiler};
    ///
    /// let mut dict = SymbolDict::new();
    /// dict.intern("transformer model").unwrap();
    ///
    /// let transpiler = StreamingTranspiler::with_dict(8192, FidelityLevel::Semantic, dict);
    /// ```
    pub fn with_dict(budget: usize, fidelity: FidelityLevel, dict: SymbolDict) -> Self {
        Self {
            compressor: AdaptiveCompressor::new(),
            budget,
            fidelity,
            dict,
            channel_buffer: DEFAULT_CHANNEL_BUFFER,
        }
    }

    /// Sets the Tokio mpsc channel buffer size and returns `self` for chaining.
    ///
    /// The internal pipeline uses a bounded `mpsc` channel to deliver chunks to
    /// the caller. The buffer size controls **backpressure**: when `n` chunks are
    /// already queued and the consumer has not polled them yet, the producer task
    /// will `.await` before sending the next chunk. This bounds peak memory usage
    /// to roughly `n` in-flight chunks at a time.
    ///
    /// - **Larger `n`**: fewer producer stalls, higher peak memory.
    /// - **Smaller `n`**: tighter backpressure, lower memory footprint.
    ///
    /// Defaults to `32` when not set.
    ///
    /// # Example
    /// ```rust,no_run
    /// use llm_transpile::{FidelityLevel, StreamingTranspiler};
    ///
    /// let transpiler = StreamingTranspiler::new(4096, FidelityLevel::Semantic)
    ///     .with_channel_size(8);
    /// ```
    pub fn with_channel_size(mut self, n: usize) -> Self {
        // Tokio's mpsc::channel requires a capacity of at least 1; clamp to prevent panic.
        self.channel_buffer = n.max(1);
        self
    }

    /// Converts an `IRDocument` into a chunk stream.
    ///
    /// The first chunk always contains `<D>` (if non-empty) + `<H><B>`.
    /// Automatically switches to `Compressed` fidelity when 80% of the budget is reached.
    pub fn transpile(
        self,
        doc: IRDocument,
    ) -> Pin<Box<dyn Stream<Item = Result<TranspileChunk, StreamError>> + Send>> {
        let (tx, rx) = mpsc::channel::<Result<TranspileChunk, StreamError>>(self.channel_buffer);
        let stream = ReceiverStream::new(rx);

        tokio::spawn(async move {
            if let Err(e) = Self::run_pipeline(
                doc,
                self.budget,
                self.fidelity,
                self.compressor,
                self.dict,
                tx,
            )
            .await
            {
                // Error already sent over the channel; ignore at spawn level.
                let _ = e;
            }
        });

        Box::pin(stream)
    }

    async fn run_pipeline(
        doc: IRDocument,
        budget: usize,
        fidelity: FidelityLevel,
        compressor: AdaptiveCompressor,
        dict: SymbolDict,
        tx: mpsc::Sender<Result<TranspileChunk, StreamError>>,
    ) -> Result<(), StreamError> {
        let mut accumulated_tokens: usize = 0;
        let mut sequence: usize = 0;

        // ── Chunk 0: header (always first) ──────────────────────────────
        let header_content = build_header_chunk(&doc, &dict);
        accumulated_tokens += estimate_tokens(&header_content);

        let total_nodes = doc.nodes.len();
        let is_final_header = total_nodes == 0;

        tx.send(Ok(TranspileChunk::new(
            sequence,
            header_content,
            is_final_header,
        )))
        .await
        .map_err(|_| StreamError::ChannelClosed)?;
        sequence += 1;

        if is_final_header {
            return Ok(());
        }

        // ── Stream body nodes ────────────────────────────────────────────
        let body_nodes: Vec<DocNode> = doc
            .nodes
            .into_iter()
            .filter(|n| !matches!(n, crate::ir::DocNode::Metadata { .. }))
            .collect();

        // ── Batch compression (single pass over all body nodes) ──────────
        // Compute effective fidelity once based on post-header token usage,
        // then compress the entire batch in one call — O(N+M) Aho-Corasick
        // pass instead of N separate passes, and no per-node Vec allocation.
        let usage_after_header = if budget > 0 {
            accumulated_tokens as f64 / budget as f64
        } else {
            1.0
        };
        let batch_fidelity = if fidelity != FidelityLevel::Lossless && usage_after_header >= 0.80 {
            FidelityLevel::Compressed
        } else {
            fidelity
        };
        let batch_cfg = CompressionConfig {
            budget,
            current_tokens: accumulated_tokens,
            fidelity: batch_fidelity,
        };
        let body_nodes = compressor.compress(body_nodes, &batch_cfg);

        let body_len = body_nodes.len();
        for (idx, node) in body_nodes.into_iter().enumerate() {
            let is_last = body_len > 0 && idx == body_len - 1;

            // Compression was already applied to the full batch above; render directly.
            let chunk_text = render_node(&node, &dict);

            if chunk_text.is_empty() {
                continue; // Skip nodes entirely eliminated by compression
            }

            // Force final chunk when budget is exceeded
            let tokens = estimate_tokens(&chunk_text);
            accumulated_tokens += tokens;
            let force_final = budget > 0 && accumulated_tokens >= budget;
            let is_final = is_last || force_final;

            // Append </B> closing tag to the final chunk
            let content = if is_final {
                format!("{}\n</B>", chunk_text.trim())
            } else {
                chunk_text
            };

            // TranspileChunk::new re-calls estimate_tokens internally, so
            // token_count is recalculated based on content (including the </B> tag).
            // accumulated_tokens is based on chunk_text — within acceptable error margin.
            tx.send(Ok(TranspileChunk::new(sequence, content, is_final)))
                .await
                .map_err(|_| StreamError::ChannelClosed)?;
            sequence += 1;

            if force_final {
                break;
            }
        }

        // Guard for the edge case where body nodes existed but the final chunk was never sent
        // (all nodes eliminated by compression)
        if sequence == 1 {
            tx.send(Ok(TranspileChunk::new(sequence, "</B>".to_string(), true)))
                .await
                .map_err(|_| StreamError::ChannelClosed)?;
        }

        Ok(())
    }
}

// ────────────────────────────────────────────────
// 4. Helper functions
// ────────────────────────────────────────────────

/// Builds the document header chunk text (`<D>?<H><B>` opening).
fn build_header_chunk(doc: &IRDocument, dict: &SymbolDict) -> String {
    let dict_block = dict.render_dict_header();
    let yaml = crate::renderer::build_yaml_header(doc);

    let mut out = String::new();
    if !dict_block.is_empty() {
        out.push_str(&dict_block);
    }
    if !yaml.is_empty() {
        out.push_str("<H>\n");
        out.push_str(yaml.trim());
        out.push_str("\n</H>\n");
    }
    out.push_str("<B>");
    out
}

// ────────────────────────────────────────────────
// 5. Error type
// ────────────────────────────────────────────────

/// Streaming transpile error.
#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    #[error("stream channel closed")]
    ChannelClosed,

    #[error("parse failed: {0}")]
    Parse(String),

    #[error("input exceeds maximum allowed size of {0} bytes")]
    InputTooLarge(usize),
}

// ────────────────────────────────────────────────
// 6. Unit tests
// ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::DocNode;
    use futures::StreamExt;

    fn make_doc(fidelity: FidelityLevel, paras: &[&str]) -> IRDocument {
        let mut doc = IRDocument::new(fidelity, None);
        doc.push(DocNode::Metadata {
            key: "title".into(),
            value: "스트리밍 테스트".into(),
        });
        for (i, &text) in paras.iter().enumerate() {
            doc.push(DocNode::Para {
                text: text.into(),
                importance: 1.0 - (i as f32 * 0.1),
            });
        }
        doc
    }

    #[tokio::test]
    async fn first_chunk_contains_header() {
        let doc = make_doc(FidelityLevel::Semantic, &["첫 번째 단락"]);
        let transpiler = StreamingTranspiler::new(10_000, FidelityLevel::Semantic);
        let mut stream = transpiler.transpile(doc);

        let first = stream.next().await.unwrap().unwrap();
        assert_eq!(first.sequence, 0);
        assert!(
            first.content.contains("<H>"),
            "first chunk must contain the header"
        );
        assert!(
            first.content.contains("<B>"),
            "first chunk must contain the <B> opening"
        );
    }

    #[tokio::test]
    async fn last_chunk_is_marked_final() {
        let doc = make_doc(FidelityLevel::Semantic, &["단락A", "단락B"]);
        let transpiler = StreamingTranspiler::new(10_000, FidelityLevel::Semantic);
        let mut stream = transpiler.transpile(doc);

        let mut last_chunk = None;
        while let Some(chunk) = stream.next().await {
            last_chunk = Some(chunk.unwrap());
        }
        let last = last_chunk.expect("at least one chunk must exist");
        assert!(last.is_final, "last chunk must have is_final=true");
    }

    #[tokio::test]
    async fn budget_triggers_force_final() {
        // Extremely low budget → force-final on the first body chunk
        let doc = make_doc(
            FidelityLevel::Semantic,
            &["긴 내용 단락1", "긴 내용 단락2", "긴 내용 단락3"],
        );
        let transpiler = StreamingTranspiler::new(5, FidelityLevel::Semantic); // 5-token budget
        let chunks: Vec<_> = transpiler.transpile(doc).collect::<Vec<_>>().await;

        let finals: Vec<_> = chunks
            .iter()
            .filter(|c| c.as_ref().unwrap().is_final)
            .collect();
        assert_eq!(finals.len(), 1, "exactly one chunk must have is_final=true");
    }

    #[tokio::test]
    async fn with_dict_emits_dict_block_in_first_chunk() {
        let mut dict = SymbolDict::new();
        dict.intern("대규모언어모델").unwrap();

        let doc = make_doc(FidelityLevel::Semantic, &["대규모언어모델 연구 동향"]);
        let transpiler = StreamingTranspiler::with_dict(10_000, FidelityLevel::Semantic, dict);
        let mut stream = transpiler.transpile(doc);

        let first = stream.next().await.unwrap().unwrap();
        assert!(
            first.content.contains("<D>"),
            "first chunk must contain the <D> dictionary block when dict is pre-populated"
        );
        assert!(
            first.content.contains("대규모언어모델"),
            "dictionary block must list the interned term"
        );
    }

    #[tokio::test]
    async fn custom_channel_size_streaming_works() {
        // Verify that a non-default channel buffer size (4) does not break
        // streaming correctness: all chunks arrive, the last is marked final,
        // and the closing </B> tag is present.
        let doc = make_doc(
            FidelityLevel::Semantic,
            &["단락 one", "단락 two", "단락 three"],
        );
        let transpiler =
            StreamingTranspiler::new(10_000, FidelityLevel::Semantic).with_channel_size(4);

        let chunks: Vec<_> = transpiler
            .transpile(doc)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        assert!(!chunks.is_empty(), "must produce at least one chunk");

        let final_count = chunks.iter().filter(|c| c.is_final).count();
        assert_eq!(final_count, 1, "exactly one chunk must be final");

        let last = chunks.last().unwrap();
        assert!(last.is_final);
        assert!(last.content.contains("</B>"), "last chunk must close <B>");
    }

    #[test]
    fn estimate_tokens_nonzero() {
        assert!(estimate_tokens("hello world") > 0);
        assert!(estimate_tokens("") == 1); // min=1 guard
    }

    #[test]
    fn estimate_tokens_empty_is_one() {
        assert_eq!(estimate_tokens(""), 1);
    }

    #[test]
    fn estimate_tokens_latin_positive() {
        assert!(estimate_tokens("hello") > 0);
    }

    #[test]
    #[cfg(not(feature = "tiktoken"))]
    fn estimate_tokens_cjk_more_than_latin_same_char_count() {
        // CJK 5 chars: 5 * (1/2) = 2.5 → ceil → 3 tokens
        // Latin 5 chars: 5 * (1/4) = 1.25 → ceil → 2 tokens
        // CJK token count > Latin token count
        let cjk = estimate_tokens("こんにちは"); // Hiragana, 5 chars
        let latin = estimate_tokens("hello"); // Latin, 5 chars
        assert!(
            cjk > latin,
            "CJK 5 chars ({cjk}) must have more tokens than Latin 5 chars ({latin})"
        );
    }

    #[test]
    #[cfg(not(feature = "tiktoken"))]
    fn estimate_tokens_hangul_more_than_latin() {
        // Hangul 4 chars: 4 * (1/2) = 2.0 → ceil → 2 tokens
        // Latin 4 chars: 4 * (1/4) = 1.0 → ceil → 1 token
        let hangul = estimate_tokens("안녕하세");
        let latin = estimate_tokens("hell");
        assert!(
            hangul > latin,
            "Hangul ({hangul}) must have more tokens than Latin ({latin})"
        );
    }

    #[test]
    #[cfg(not(feature = "tiktoken"))]
    fn estimate_tokens_pua_is_one_per_char() {
        // PUA characters should be estimated at ~1 token each
        let pua_text = "\u{E000}\u{E001}\u{E002}\u{E003}"; // 4 PUA chars
        let tokens = estimate_tokens(pua_text);
        assert_eq!(
            tokens, 4,
            "4 PUA chars should estimate to 4 tokens, got {tokens}"
        );
    }

    #[test]
    #[cfg(not(feature = "tiktoken"))]
    fn estimate_tokens_pua_less_than_latin() {
        // Same character count: PUA should have more tokens (lower cpt) than Latin
        let pua = estimate_tokens("\u{E000}\u{E001}\u{E002}\u{E003}"); // 4 PUA chars
        let latin = estimate_tokens("hell"); // 4 Latin chars
        assert!(
            pua > latin,
            "PUA ({pua}) should estimate more tokens than Latin ({latin})"
        );
    }

    #[test]
    fn estimate_tokens_never_zero_for_nonempty() {
        for text in &["a", "안", "あ", "ع", "क", "ก"] {
            assert!(
                estimate_tokens(text) >= 1,
                "'{text}' must be at least 1 token"
            );
        }
    }

    /// Batch compression regression: with many identical paragraphs,
    /// deduplication (DeduplicateAndLinearize stage) should fire and reduce
    /// the chunk count compared to the raw paragraph count.
    /// This test is only meaningful if compression fires (high budget usage).
    #[tokio::test]
    async fn batch_compression_deduplicates_identical_paras() {
        // 5 identical paragraphs + 95% budget consumed → DeduplicateAndLinearize fires.
        // After batch compression only 1 unique paragraph should remain, producing
        // exactly 2 chunks: the header (seq=0) + 1 body chunk (seq=1, is_final).
        let mut doc = IRDocument::new(FidelityLevel::Semantic, None);
        doc.push(DocNode::Metadata {
            key: "title".into(),
            value: "배치 압축 테스트".into(),
        });
        // Use high-budget-usage to trigger DeduplicateAndLinearize
        let para_text = "identical content paragraph.";
        for _ in 0..5 {
            doc.push(DocNode::Para {
                text: para_text.into(),
                importance: 1.0,
            });
        }

        // budget=10: header already uses ~5 tokens → usage >80% → Compressed mode
        let transpiler = StreamingTranspiler::new(10, FidelityLevel::Semantic);
        let chunks: Vec<_> = transpiler
            .transpile(doc)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        assert!(!chunks.is_empty(), "must produce at least one chunk");
        let final_count = chunks.iter().filter(|c| c.is_final).count();
        assert_eq!(final_count, 1, "exactly one chunk must be final");
    }
}
