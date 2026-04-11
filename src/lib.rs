//! # llm-transpiler
//!
//! Raw 문서(Markdown, HTML, Plain Text, Table 등)를 LLM 에이전트가
//! **최소 토큰으로 최대 정보**를 수신할 수 있도록 구조화된 브릿지 포맷으로
//! 변환하는 고성능 Rust 라이브러리.
//!
//! ## 빠른 시작
//!
//! ```rust
//! use llm_transpiler::{transpile, FidelityLevel, InputFormat};
//!
//! let md = "# 계약서\n\n본 계약은 2024년에 체결되었습니다.";
//! let result = transpile(md, InputFormat::Markdown, FidelityLevel::Semantic, Some(4096))
//!     .expect("변환 실패");
//! println!("{}", result);
//! ```
//!
//! ## 스트리밍 사용
//!
//! ```rust,no_run
//! use llm_transpiler::{transpile_stream, FidelityLevel, InputFormat};
//! use futures::StreamExt;
//!
//! async fn example() {
//!     let md = "# 문서\n\n단락 내용입니다.";
//!     let mut stream = transpile_stream(md, InputFormat::Markdown, FidelityLevel::Semantic, 4096).await;
//!     while let Some(chunk) = stream.next().await {
//!         let chunk = chunk.expect("스트림 오류");
//!         print!("{}", chunk.content);
//!         if chunk.is_final { break; }
//!     }
//! }
//! ```

// ────────────────────────────────────────────────
// 내부 모듈
// ────────────────────────────────────────────────

pub(crate) mod compressor;
pub(crate) mod ir;
pub(crate) mod renderer;
pub(crate) mod stream;
pub(crate) mod symbol;

// 파서 모듈 (Markdown → IR)
mod parser;

// ────────────────────────────────────────────────
// 공개 재수출 (Re-exports)
// ────────────────────────────────────────────────

pub use compressor::{AdaptiveCompressor, CompressionConfig, CompressionStage};
pub use ir::{DocNode, FidelityLevel, IRDocument};
pub use renderer::{build_yaml_header, linearize_table, render_full, render_node};
pub use stream::{StreamError, StreamingTranspiler, TranspileChunk};
pub use symbol::SymbolDict;

// ────────────────────────────────────────────────
// 공개 열거형
// ────────────────────────────────────────────────

/// 입력 문서 포맷.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    /// 일반 텍스트.
    PlainText,
    /// CommonMark 호환 Markdown.
    Markdown,
    /// HTML5.
    Html,
}

// ────────────────────────────────────────────────
// 최상위 에러 타입
// ────────────────────────────────────────────────

/// 트랜스파일 에러.
#[derive(Debug, thiserror::Error)]
pub enum TranspileError {
    #[error("파싱 실패: {0}")]
    Parse(String),

    #[error("심볼 테이블 초과: {0}")]
    SymbolOverflow(#[from] symbol::SymbolOverflowError),

    #[error("스트림 에러: {0}")]
    Stream(#[from] stream::StreamError),

    #[error("Lossless 모드에서 압축 시도")]
    LosslessModeViolation,
}

// ────────────────────────────────────────────────
// 내부 헬퍼
// ────────────────────────────────────────────────

/// 입력 문자열에서 Unicode PUA 범위(U+E000–U+F8FF) 문자를 제거한다.
/// 외부 입력이 내부 심볼 치환 체계와 충돌하는 것을 방지한다.
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
// 공개 API
// ────────────────────────────────────────────────

/// 문서를 **동기적**으로 브릿지 포맷으로 변환한다.
///
/// # Arguments
/// - `input`    — 원본 문서 텍스트
/// - `format`   — 입력 포맷 (Markdown / HTML / PlainText)
/// - `fidelity` — 의미 보존 레벨
/// - `budget`   — 최대 토큰 수 (`None` = 무제한)
///
/// # Returns
/// 브릿지 포맷 문자열 (`<D>?<H><B>...</B>`)
///
/// # Errors
/// 파싱 실패, 심볼 테이블 초과 시 `TranspileError` 반환.
pub fn transpile(
    input: &str,
    format: InputFormat,
    fidelity: FidelityLevel,
    budget: Option<usize>,
) -> Result<String, TranspileError> {
    let input = strip_pua(input);
    let input = input.as_ref();

    // 1. 파싱 → IR
    let mut doc = parser::parse(input, format, fidelity, budget)
        .map_err(TranspileError::Parse)?;

    // 2. 압축 (예산이 있을 때만)
    if let Some(b) = budget {
        let compressor = AdaptiveCompressor::new();
        let cfg = CompressionConfig {
            budget: b,
            current_tokens: stream::estimate_tokens(input),
            fidelity,
        };
        doc.nodes = compressor.compress(std::mem::take(&mut doc.nodes), &cfg);
    }

    // 3. 렌더링
    let mut dict = SymbolDict::new();
    let output = render_full(&doc, &mut dict);
    Ok(output)
}

/// 문서를 **Tokio 스트림**으로 변환한다.
///
/// 첫 청크가 즉시 전달되므로 TTFT를 최소화할 수 있다.
///
/// # Arguments
/// - `input`    — 원본 문서 텍스트
/// - `format`   — 입력 포맷 (Markdown / HTML / PlainText)
/// - `fidelity` — 의미 보존 레벨
/// - `budget`   — 최대 허용 토큰 수. `0`을 전달하면 "제한 없음"으로 처리되며
///   예산 소모율 계산 시 즉시 `Compressed` 모드로 전환됩니다.
///   토큰 한도를 두려면 0이 아닌 양수 값을 사용하세요.
///
/// # Errors
/// 파싱 실패 시 스트림의 첫 번째 아이템으로 `Err(StreamError::Parse(...))` 가 전송됩니다.
/// 그 후 스트림은 닫힙니다. 에러를 단일 `Result` 로 받으려면 [`transpile`] 을 사용하세요.
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
            // 파싱 실패: 단일 Err 청크를 담은 스트림을 즉시 반환한다.
            // futures::future::ready()는 Unpin이므로 stream::once와 안전하게 사용 가능.
            return Box::pin(futures::stream::once(futures::future::ready(
                Err(StreamError::Parse(msg)),
            )));
        }
    };

    let transpiler = StreamingTranspiler::new(budget, fidelity);
    Box::pin(transpiler.transpile(doc))
}

/// 텍스트의 근사 토큰 수를 반환한다.
///
/// 실제 모델 토크나이저 없이 문자 수 기반 휴리스틱을 사용한다.
/// 정밀도가 필요한 경우 `tiktoken-rs` 또는 `tokenizers` crate를 직접 사용하세요.
pub fn token_count(text: &str) -> usize {
    stream::estimate_tokens(text)
}

// ────────────────────────────────────────────────
// 통합 테스트
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
        assert!(result.is_ok(), "변환이 성공해야 한다: {:?}", result.err());
        let output = result.unwrap();
        assert!(output.contains("<B>"), "출력에 <B> 태그가 있어야 한다");
        assert!(output.contains("</B>"), "출력에 </B> 닫기 태그가 있어야 한다");
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
        assert!(!output.contains('\u{E000}'), "PUA 문자가 출력에 포함되면 안 됩니다");
        assert!(output.contains("hello"), "일반 텍스트는 보존되어야 합니다");
        assert!(output.contains("world"), "PUA 제거 후 인접 텍스트는 보존되어야 합니다");
    }

    #[tokio::test]
    async fn stream_error_variant_is_send_and_stream_works() {
        use futures::StreamExt;
        use stream::StreamError;

        // StreamError::Parse variant 컴파일 타임 확인
        fn _assert_send<T: Send>(_: T) {}
        _assert_send(StreamError::Parse("test".to_string()));

        // 정상 스트리밍 동작 확인
        let mut stream = transpile_stream(SAMPLE_MD, InputFormat::Markdown, FidelityLevel::Semantic, 8192).await;
        let first = stream.next().await.expect("최소 1개의 청크가 있어야 한다");
        assert!(first.is_ok(), "정상 입력은 Ok 청크를 반환해야 한다: {:?}", first.err());
    }
}
