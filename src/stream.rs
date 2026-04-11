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
    /// Approximate token count (character-count / 4 heuristic).
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

/// Approximate token count (heuristic for use without tiktoken).
///
/// Applies a chars-per-token weight based on each character's Unicode script range,
/// sums `1/cpt`, then takes the ceiling.
///
/// Replace with `tiktoken-rs` or the `tokenizers` crate for production use.
pub fn estimate_tokens(text: &str) -> usize {
    let mut total = 0.0f64;
    for c in text.chars() {
        let cpt = chars_per_token(c);
        total += 1.0 / cpt as f64;
    }
    (total.ceil() as usize).max(1)
}

/// Returns the chars-per-token value based on the Unicode codepoint range.
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
        _ => 4,                 // Latin and other scripts
    }
}

// ────────────────────────────────────────────────
// 2. StreamingTranspiler
// ────────────────────────────────────────────────

/// Tokio channel-based streaming transpiler.
pub struct StreamingTranspiler {
    compressor: AdaptiveCompressor,
    budget: usize,
    fidelity: FidelityLevel,
}

impl StreamingTranspiler {
    /// Creates a new transpiler.
    pub fn new(budget: usize, fidelity: FidelityLevel) -> Self {
        Self {
            compressor: AdaptiveCompressor::new(),
            budget,
            fidelity,
        }
    }

    /// Converts an `IRDocument` into a chunk stream.
    ///
    /// The first chunk always contains `<D>` + `<H>`.
    /// Automatically switches to `Compressed` mode when 80% of the budget is reached.
    pub fn transpile(
        self,
        doc: IRDocument,
    ) -> Pin<Box<dyn Stream<Item = Result<TranspileChunk, StreamError>> + Send>> {
        let (tx, rx) = mpsc::channel::<Result<TranspileChunk, StreamError>>(32);
        let stream = ReceiverStream::new(rx);

        tokio::spawn(async move {
            if let Err(e) =
                Self::run_pipeline(doc, self.budget, self.fidelity, &self.compressor, tx).await
            {
                // Error already sent over the channel; ignore at spawn level
                let _ = e;
            }
        });

        Box::pin(stream)
    }

    async fn run_pipeline(
        doc: IRDocument,
        budget: usize,
        fidelity: FidelityLevel,
        compressor: &AdaptiveCompressor,
        tx: mpsc::Sender<Result<TranspileChunk, StreamError>>,
    ) -> Result<(), StreamError> {
        // NOTE: SymbolDict remains empty in the streaming path.
        // Symbol substitution (PUA encoding) is not currently supported because the single-pass
        // design cannot know all terms before the stream starts.
        // Use the synchronous `transpile()` if full symbol substitution is required.
        let dict = SymbolDict::new();
        let mut accumulated_tokens: usize = 0;
        let mut sequence: usize = 0;

        // ── Chunk 0: header (always first) ──────────
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

        // ── Stream body nodes ────────────────────────
        let body_nodes: Vec<DocNode> = doc
            .nodes
            .into_iter()
            .filter(|n| !matches!(n, crate::ir::DocNode::Metadata { .. }))
            .collect();

        let body_len = body_nodes.len();
        for (idx, node) in body_nodes.into_iter().enumerate() {
            let is_last = idx == body_len - 1;

            // Switch to Compressed at 80% budget usage.
            // If budget=0, 0/0 = NaN → NaN >= 0.80 is false so the branch never triggers.
            // budget=0 is not a valid value for the public API (transpile_stream); caller's responsibility.
            let usage = if budget > 0 {
                accumulated_tokens as f64 / budget as f64
            } else {
                1.0 // budget=0: immediately switch to Compressed
            };
            let effective_fidelity = if fidelity != FidelityLevel::Lossless && usage >= 0.80 {
                FidelityLevel::Compressed
            } else {
                fidelity
            };

            // Apply compression to a single node
            let cfg = CompressionConfig {
                budget,
                current_tokens: accumulated_tokens,
                fidelity: effective_fidelity,
            };
            let compressed = compressor.compress(vec![node], &cfg);

            let chunk_text: String = compressed
                .iter()
                .map(|n| render_node(n, &dict))
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n");

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
// 3. Helper functions
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
// 4. Error type
// ────────────────────────────────────────────────

/// Streaming transpile error.
#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    #[error("stream channel closed")]
    ChannelClosed,

    #[error("parse failed: {0}")]
    Parse(String),
}

// ────────────────────────────────────────────────
// 5. Unit tests
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
    fn estimate_tokens_never_zero_for_nonempty() {
        for text in &["a", "안", "あ", "ع", "क", "ก"] {
            assert!(
                estimate_tokens(text) >= 1,
                "'{text}' must be at least 1 token"
            );
        }
    }
}
