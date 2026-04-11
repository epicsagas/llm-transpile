//! stream.rs — Tokio 기반 Streaming Transpiler
//!
//! 문서를 전체 처리가 완료되기 전에 청크 단위로 LLM에 전달하여
//! TTFT(Time-To-First-Token)를 최소화한다.
//!
//! # 파이프라인
//! ```text
//! AsyncRead → IncrementalParser → AdaptiveCompressor → StreamingRenderer
//!                                        ↑
//!                              예산 80% 도달 시 Compressed 전환
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
// 1. 청크 타입
// ────────────────────────────────────────────────

/// 스트리밍 트랜스파일러가 생성하는 단일 출력 단위.
#[derive(Debug, Clone)]
pub struct TranspileChunk {
    /// 전송 순서 (0부터 시작).
    pub sequence: usize,
    /// 렌더링된 텍스트 조각.
    pub content: String,
    /// 근사 토큰 수 (문자 수 / 4 휴리스틱).
    pub token_count: usize,
    /// 마지막 청크 여부.
    pub is_final: bool,
}

impl TranspileChunk {
    fn new(sequence: usize, content: String, is_final: bool) -> Self {
        let token_count = estimate_tokens(&content);
        Self { sequence, content, token_count, is_final }
    }
}

/// 토큰 수 근사치 (tiktoken 없이 사용 시 휴리스틱).
///
/// 각 문자의 Unicode 스크립트 범위에 따라 chars-per-token 가중치를 적용하여
/// `1/cpt` 합산 후 ceil한다.
///
/// 실제 배포 환경에서는 `tiktoken-rs` 또는 `tokenizers` crate로 대체하세요.
pub fn estimate_tokens(text: &str) -> usize {
    let mut total = 0.0f64;
    for c in text.chars() {
        let cpt = chars_per_token(c);
        total += 1.0 / cpt as f64;
    }
    (total.ceil() as usize).max(1)
}

/// Unicode 코드포인트 범위에 따라 chars-per-token 값을 반환한다.
fn chars_per_token(c: char) -> u32 {
    let cp = c as u32;
    match cp {
        0x3040..=0x30FF   => 2,  // Hiragana / Katakana
        0x3400..=0x4DBF   => 2,  // CJK Extension A
        0x4E00..=0x9FFF   => 2,  // CJK Unified Ideographs (BMP)
        0xF900..=0xFAFF   => 2,  // CJK Compatibility Ideographs
        0xAC00..=0xD7FF   => 2,  // Hangul Syllables (U+D7B0–D7FF: Jamo Extended-B 포함)
        0x1100..=0x11FF   => 2,  // Hangul Jamo
        0xA960..=0xA97F   => 2,  // Hangul Jamo Extended-A
        0x20000..=0x2A6DF => 2,  // CJK Extension B
        0x2A700..=0x2CEAF => 2,  // CJK Extension C–F
        0x2CEB0..=0x2EBEF => 2,  // CJK Extension G
        0x30000..=0x323AF => 2,  // CJK Extension H–I
        0x0600..=0x06FF   => 3,  // Arabic
        0x0750..=0x077F   => 3,  // Arabic Supplement
        0x0900..=0x097F   => 3,  // Devanagari
        0x0980..=0x09FF   => 3,  // Bengali
        0x0A00..=0x0A7F   => 3,  // Gurmukhi
        0x0B80..=0x0BFF   => 3,  // Tamil
        0x0E00..=0x0E7F   => 3,  // Thai
        // 이모지: GPT-4 기준 1자 ≈ 1–2토큰 → cpt=2로 근사
        0x1F300..=0x1F9FF => 2,  // Misc Symbols & Pictographs, Emoticons, Supplemental Symbols
        0x1FA00..=0x1FAFF => 2,  // Symbols and Pictographs Extended-A
        _                 => 4,  // Latin 및 기타
    }
}

// ────────────────────────────────────────────────
// 2. StreamingTranspiler
// ────────────────────────────────────────────────

/// Tokio 채널 기반 스트리밍 트랜스파일러.
pub struct StreamingTranspiler {
    compressor: AdaptiveCompressor,
    budget: usize,
    fidelity: FidelityLevel,
}

impl StreamingTranspiler {
    /// 새 트랜스파일러를 생성한다.
    pub fn new(budget: usize, fidelity: FidelityLevel) -> Self {
        Self {
            compressor: AdaptiveCompressor::new(),
            budget,
            fidelity,
        }
    }

    /// `IRDocument`를 청크 스트림으로 변환한다.
    ///
    /// 첫 청크는 항상 `<D>` + `<H>` 를 포함한다.
    /// 예산 80% 도달 시 자동으로 `Compressed` 모드로 전환한다.
    pub fn transpile(
        self,
        doc: IRDocument,
    ) -> Pin<Box<dyn Stream<Item = Result<TranspileChunk, StreamError>> + Send>> {
        let (tx, rx) = mpsc::channel::<Result<TranspileChunk, StreamError>>(32);
        let stream = ReceiverStream::new(rx);

        tokio::spawn(async move {
            if let Err(e) = Self::run_pipeline(doc, self.budget, self.fidelity, &self.compressor, tx).await {
                // 에러는 이미 채널로 전송됨; spawn 레벨에서 무시
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
        // NOTE: 스트리밍 경로에서는 SymbolDict가 빈 채로 유지됩니다.
        // 심볼 치환(PUA 인코딩)은 단일 패스 설계상 스트림 시작 전에 모든 용어를 알 수 없으므로
        // 현재 지원하지 않습니다. 완전한 심볼 치환이 필요하면 동기 `transpile()`을 사용하세요.
        let dict = SymbolDict::new();
        let mut accumulated_tokens: usize = 0;
        let mut sequence: usize = 0;

        // ── 청크 0: 헤더 (항상 첫 번째) ────────────
        let header_content = build_header_chunk(&doc, &dict);
        accumulated_tokens += estimate_tokens(&header_content);

        let total_nodes = doc.nodes.len();
        let is_final_header = total_nodes == 0;

        tx.send(Ok(TranspileChunk::new(sequence, header_content, is_final_header)))
            .await
            .map_err(|_| StreamError::ChannelClosed)?;
        sequence += 1;

        if is_final_header {
            return Ok(());
        }

        // ── 본문 노드 스트리밍 ─────────────────────
        let body_nodes: Vec<DocNode> = doc
            .nodes
            .into_iter()
            .filter(|n| !matches!(n, crate::ir::DocNode::Metadata { .. }))
            .collect();

        let body_len = body_nodes.len();
        for (idx, node) in body_nodes.into_iter().enumerate() {
            let is_last = idx == body_len - 1;

            // 예산 80% 도달 시 Compressed 전환
            // budget=0 이면 0/0 = NaN → NaN >= 0.80 은 false이므로 분기가 발생하지 않는다.
            // budget=0 은 공개 API(transpile_stream)에서 허용되지 않는 값이며, 호출자 책임.
            let usage = if budget > 0 {
                accumulated_tokens as f64 / budget as f64
            } else {
                1.0 // budget=0: 즉시 Compressed 전환
            };
            let effective_fidelity = if fidelity != FidelityLevel::Lossless
                && usage >= 0.80
            {
                FidelityLevel::Compressed
            } else {
                fidelity
            };

            // 단일 노드 압축 적용
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
                continue; // 압축으로 완전히 제거된 노드 건너뜀
            }

            // 예산 초과 시 마지막 청크로 강제 종료
            let tokens = estimate_tokens(&chunk_text);
            accumulated_tokens += tokens;
            let force_final = budget > 0 && accumulated_tokens >= budget;
            let is_final = is_last || force_final;

            // 마지막 청크에 </B> 닫기 태그 추가
            let content = if is_final {
                format!("{}\n</B>", chunk_text.trim())
            } else {
                chunk_text
            };

            // TranspileChunk::new 내부에서 estimate_tokens를 재호출하므로
            // token_count는 content 기준으로 재계산된다 (</B> 태그 포함).
            // accumulated_tokens는 chunk_text 기준 — 허용 오차 범위 내.
            tx.send(Ok(TranspileChunk::new(sequence, content, is_final)))
                .await
                .map_err(|_| StreamError::ChannelClosed)?;
            sequence += 1;

            if force_final {
                break;
            }
        }

        // 본문 노드가 있었지만 마지막 청크 발송이 안 된 경우 방어
        // (모든 노드가 압축으로 제거된 극단 케이스)
        if sequence == 1 {
            tx.send(Ok(TranspileChunk::new(sequence, "</B>".to_string(), true)))
                .await
                .map_err(|_| StreamError::ChannelClosed)?;
        }

        Ok(())
    }
}

// ────────────────────────────────────────────────
// 3. 헬퍼 함수
// ────────────────────────────────────────────────

/// 문서 헤더 청크 텍스트를 생성한다 (`<D>?<H><B>` 오프닝).
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
// 4. 에러 타입
// ────────────────────────────────────────────────

/// 스트리밍 트랜스파일 에러.
#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    #[error("스트림 채널이 닫혔습니다")]
    ChannelClosed,

    #[error("파싱 실패: {0}")]
    Parse(String),
}

// ────────────────────────────────────────────────
// 5. 단위 테스트
// ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::DocNode;
    use futures::StreamExt;

    fn make_doc(fidelity: FidelityLevel, paras: &[&str]) -> IRDocument {
        let mut doc = IRDocument::new(fidelity, None);
        doc.push(DocNode::Metadata { key: "title".into(), value: "스트리밍 테스트".into() });
        for (i, &text) in paras.iter().enumerate() {
            doc.push(DocNode::Para { text: text.into(), importance: 1.0 - (i as f32 * 0.1) });
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
        assert!(first.content.contains("<H>"), "첫 청크는 헤더를 포함해야 한다");
        assert!(first.content.contains("<B>"), "첫 청크는 <B> 오프닝을 포함해야 한다");
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
        let last = last_chunk.expect("최소 1개의 청크가 있어야 한다");
        assert!(last.is_final, "마지막 청크는 is_final=true 여야 한다");
    }

    #[tokio::test]
    async fn budget_triggers_force_final() {
        // 극도로 낮은 예산 → 첫 본문 청크에서 강제 종료
        let doc = make_doc(FidelityLevel::Semantic, &["긴 내용 단락1", "긴 내용 단락2", "긴 내용 단락3"]);
        let transpiler = StreamingTranspiler::new(5, FidelityLevel::Semantic); // 5토큰 예산
        let chunks: Vec<_> = transpiler
            .transpile(doc)
            .collect::<Vec<_>>()
            .await;

        let finals: Vec<_> = chunks.iter().filter(|c| c.as_ref().unwrap().is_final).collect();
        assert_eq!(finals.len(), 1, "is_final=true 청크는 정확히 1개여야 한다");
    }

    #[test]
    fn estimate_tokens_nonzero() {
        assert!(estimate_tokens("hello world") > 0);
        assert!(estimate_tokens("") == 1); // min=1 방어
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
        // CJK 5글자: 5 * (1/2) = 2.5 → ceil → 3 tokens
        // Latin 5글자: 5 * (1/4) = 1.25 → ceil → 2 tokens
        // CJK token 수 > Latin token 수
        let cjk = estimate_tokens("こんにちは"); // Hiragana 5자
        let latin = estimate_tokens("hello");    // Latin 5자
        assert!(
            cjk > latin,
            "CJK 5글자({cjk}) 는 Latin 5글자({latin}) 보다 token 수가 많아야 한다"
        );
    }

    #[test]
    fn estimate_tokens_hangul_more_than_latin() {
        // 한글 4글자: 4 * (1/2) = 2.0 → ceil → 2 tokens
        // Latin 4글자: 4 * (1/4) = 1.0 → ceil → 1 token
        let hangul = estimate_tokens("안녕하세");
        let latin = estimate_tokens("hell");
        assert!(
            hangul > latin,
            "Hangul({hangul}) 은 Latin({latin}) 보다 token 수가 많아야 한다"
        );
    }

    #[test]
    fn estimate_tokens_never_zero_for_nonempty() {
        for text in &["a", "안", "あ", "ع", "क", "ก"] {
            assert!(estimate_tokens(text) >= 1, "'{text}' 은 최소 1 token 이어야 한다");
        }
    }
}
