# LLM Transpiler Bridge — 기술 사양서 (SPEC)

> **버전**: 0.1.0
> **작성일**: 2026-04-11
> **상태**: Draft

---

## 1. 프로젝트 개요

### 1.1 목적

Raw 문서(PDF, HTML, Markdown, Plain Text, Table 등)를 LLM 에이전트가
최소 토큰으로 최대 정보를 수신할 수 있도록 **구조화된 브릿지 포맷**으로
변환하는 고성능 Rust 라이브러리.

### 1.2 핵심 목표

| 목표 | 지표 |
|------|------|
| 파싱 속도 | Python 대비 ≥ 10× 향상 |
| 토큰 절감 | 원문 대비 15–30% 절감 |
| TTFT 개선 | 스트리밍으로 첫 청크 ≤ 50ms 전달 |
| 안전성 | 역치환 충돌 제로, 의미 손실 명시적 제어 |

### 1.3 스코프 외 항목 (Out of Scope)

- LLM API 직접 호출 (Anthropic / OpenAI SDK 연동은 사용자 책임)
- 임베딩 생성
- 벡터 DB 저장

---

## 2. 아키텍처 개요

```
┌───────────────────────────────────────────────────────┐
│                   Public API (lib.rs)                 │
│  transpile()  /  transpile_stream()  /  token_count() │ 
└───────────────────────┬───────────────────────────────┘
                        │
          ┌─────────────▼──────────────┐
          │   IncrementalParser        │  (parser.rs)
          │   lopdf / html5ever /      │
          │   pulldown-cmark           │
          └─────────────┬──────────────┘
                        │  Vec<DocNode>
          ┌─────────────▼──────────────┐
          │   IRDocument               │  (ir.rs)
          │   FidelityLevel + Budget   │
          └──────┬──────────┬──────────┘
                 │          │
    ┌────────────▼──┐  ┌────▼───────────────┐
    │  SymbolDict   │  │ AdaptiveCompressor │
    │  (symbol.rs)  │  │  (compressor.rs)   │
    └────────────┬──┘  └────┬───────────────┘
                 └────┬─────┘
          ┌───────────▼──────────────┐
          │   StreamingRenderer      │  (renderer.rs)
          │   YAML header + XML body │
          └───────────┬──────────────┘
                      │  TranspileChunk (Tokio stream)
                      ▼
               LLM API Consumer
```

---

## 3. 모듈 상세 사양

### 3.1 `ir.rs` — Intermediate Representation

#### 타입 정의

```rust
pub enum FidelityLevel {
    Lossless,   // 감사·법률: 원문 100% 보존
    Semantic,   // 일반 RAG: 의미 단위 압축
    Compressed, // 요약 파이프라인: 최대 압축
}

pub enum DocNode {
    Header   { level: u8, text: String },
    Para     { text: String, importance: f32 },
    Table    { headers: Vec<String>, rows: Vec<Vec<String>> },
    Code     { lang: Option<String>, body: String },
    List     { ordered: bool, items: Vec<String> },
    Metadata { key: String, value: String },
}

pub struct IRDocument {
    pub fidelity:     FidelityLevel,
    pub nodes:        Vec<DocNode>,
    pub token_budget: Option<usize>,
}
```

#### 불변 조건 (Invariants)

- `importance` 값 범위: `0.0..=1.0`
- `token_budget`이 `Some(n)` 이면 렌더링 결과 토큰 수 ≤ `n` 보장
- `FidelityLevel::Lossless` 에서는 `Compressed` 단계 압축 금지

---

### 3.2 `symbol.rs` — SymbolDict

#### 설계 원칙

- 치환 기호는 Unicode **Private Use Area** (`U+E000–U+F8FF`) 사용
  → 가시적 `$1`, `$2` 방식의 역치환 충돌 방지
- 전역 사전(Global Dictionary)은 문서 상단 `<D>` 태그에 1회만 출력
- `intern()` / `decode_str()` 쌍으로 encode ↔ decode 완전 대칭

#### 인터페이스

```rust
impl SymbolDict {
    pub fn new() -> Self;
    pub fn intern(&mut self, term: &str) -> char;
    pub fn decode_str(&self, input: &str) -> String;
    pub fn render_dict_header(&self) -> String;  // <D> 블록 생성
}
```

#### 제약 조건

- PUA 상한 `U+F8FF` 초과 시 `SymbolTableOverflow` 에러 반환
- 동일 term 재 intern 시 동일 기호 반환 (멱등성 보장)

---

### 3.3 `compressor.rs` — AdaptiveCompressor

#### 압축 전략 (단계별)

| 예산 소모율 | 적용 전략 |
|------------|-----------|
| 0–60%      | 불용어 제거만 |
| 60–80%     | 불용어 + 중요도 하위 20% 단락 제거 |
| 80–95%     | 위 + 중복 문장 제거 + 수치 데이터 선형화 |
| 95%+       | 위 + 모든 단락 → 1문장 요약 (Semantic only) |

#### 수치 데이터 선형화

- 행 수 ≤ 5: `Key:Val, Key:Val` 시퀀스
- 행 수 > 5: JSON Lines (`{"k":"v",...}` 1줄/행)
- Markdown 테이블 기호(`|`, `-`) 완전 제거

#### 인터페이스

```rust
pub struct CompressionConfig {
    pub budget:         usize,
    pub current_tokens: usize,
    pub fidelity:       FidelityLevel,
}

impl AdaptiveCompressor {
    pub fn compress(&self, nodes: Vec<DocNode>, cfg: &CompressionConfig)
        -> Vec<DocNode>;
}
```

---

### 3.4 `renderer.rs` — StreamingRenderer

#### 출력 포맷

```xml
<D>
t1=법률용어A
t2=전문용어B
</D>
<H>
t: 문서 제목
s: 한줄 요약
k: [키워드1, 키워드2]
</H>
<B>
... 본문 (압축·치환 적용) ...
</B>
```

- `<D>`: SymbolDict 전역 사전 (치환 없을 시 생략)
- `<H>`: YAML 직렬화 헤더 (serde-norway, YAML 1.2 준수)
- `<B>`: 본문 (줄바꿈·공백 최소화)

#### 인터페이스

```rust
pub fn render_node(node: &DocNode, dict: &SymbolDict) -> String;
pub fn render_full(doc: &IRDocument, dict: &mut SymbolDict) -> String;
```

---

### 3.5 `stream.rs` — Streaming Transpiler

#### 청크 정의

```rust
pub struct TranspileChunk {
    pub sequence:    usize,
    pub content:     String,
    pub token_count: usize,   // tiktoken-rs 사전 계산
    pub is_final:    bool,
}
```

#### 스트리밍 파이프라인

```rust
pub async fn transpile_stream(
    source:  impl AsyncRead + Unpin + Send + 'static,
    budget:  usize,
    fidelity: FidelityLevel,
) -> impl Stream<Item = Result<TranspileChunk>>;
```

- `Tokio` 기반 비동기 스트림
- 의미 단위(문단/섹션) 경계에서 청크 분리
- 예산 80% 도달 시 자동 `Compressed` 전환
- 첫 번째 청크는 항상 `<D>` + `<H>` 포함

---

## 4. 공개 API (`lib.rs`)

```rust
/// 동기 변환 — 전체 문서를 한 번에 처리
pub fn transpile(
    input:    &str,
    format:   InputFormat,
    fidelity: FidelityLevel,
    budget:   Option<usize>,
) -> Result<String, TranspileError>;

/// 비동기 스트리밍 변환
pub async fn transpile_stream(
    source:   impl AsyncRead + Unpin + Send + 'static,
    fidelity: FidelityLevel,
    budget:   usize,
) -> impl Stream<Item = Result<TranspileChunk>>;

/// 토큰 수 사전 계산 유틸리티
pub fn token_count(text: &str, model: TokenModel) -> usize;

pub enum InputFormat { PlainText, Markdown, Html, Pdf }
pub enum TokenModel  { Gpt4, Gpt35, Llama3, Claude3 }
```

---

## 5. 에러 타입

```rust
#[derive(Debug, thiserror::Error)]
pub enum TranspileError {
    #[error("파싱 실패: {0}")]
    ParseError(String),

    #[error("심볼 테이블 초과 (최대 {max} 기호)")]
    SymbolTableOverflow { max: usize },

    #[error("토큰 예산 초과: 필요 {required}, 예산 {budget}")]
    BudgetExceeded { required: usize, budget: usize },

    #[error("Lossless 모드에서 압축 시도")]
    LosslessModeViolation,

    #[error("IO 에러: {0}")]
    Io(#[from] std::io::Error),
}
```

---

## 6. 의존성 (Cargo.toml)

```toml
[dependencies]
# 파싱
lopdf          = "0.31"
html5ever      = "0.27"
pulldown-cmark = "0.11"

# 직렬화
serde          = { version = "1", features = ["derive"] }
serde_json     = "1"
serde-norway   = "0.9"   # YAML 1.2 준수 (serde_yaml 대체)

# 토큰 계산
tiktoken-rs    = "0.5"
tokenizers     = "0.19"

# 비동기
tokio          = { version = "1", features = ["full"] }
tokio-stream   = "0.1"
futures        = "0.3"

# 유틸
regex          = "1"
once_cell      = "1"
itertools      = "0.12"
rayon          = "1.8"
thiserror      = "1"

[dev-dependencies]
tokio-test     = "0.4"
criterion      = { version = "0.5", features = ["html_reports"] }
```

---

## 7. 비기능 요구사항

| 항목 | 요구사항 |
|------|----------|
| 스레드 안전성 | `SymbolDict`는 단일 문서 전용; 병렬 처리 시 문서당 독립 인스턴스 |
| 메모리 | 1MB 입력 문서 처리 시 힙 사용 ≤ 10MB |
| 테스트 커버리지 | 핵심 모듈(ir, symbol, compressor) ≥ 80% |
| MSRV | Rust 1.75+ (async fn in traits stable) |

---

## 8. 구현 로드맵

| 단계 | 작업 | 상태 |
|------|------|------|
| 1 | Cargo 프로젝트 초기화 + Cargo.toml | 🔲 |
| 2 | `ir.rs` 핵심 타입 | 🔲 |
| 3 | `symbol.rs` SymbolDict | 🔲 |
| 4 | `renderer.rs` 노드 렌더러 | 🔲 |
| 5 | `compressor.rs` AdaptiveCompressor | 🔲 |
| 6 | `stream.rs` Tokio 스트리밍 | 🔲 |
| 7 | `lib.rs` 공개 API 통합 | 🔲 |
| 8 | 단위 테스트 + 벤치마크 | 🔲 |
