# LLM Transpiler Bridge — Technical Specification (SPEC)

> **Version**: 0.1.0
> **Date**: 2026-04-11
> **Status**: Draft

---

## 1. Project Overview

### 1.1 Purpose

A high-performance Rust library that converts raw documents (PDF, HTML, Markdown, Plain Text, Tables, etc.)
into a **structured bridge format** that allows LLM agents to receive the maximum
amount of information with the minimum number of tokens.

### 1.2 Core Goals

| Goal | Metric |
|------|------|
| Parse speed | ≥ 10× improvement over Python |
| Token reduction | 15–30% reduction vs. raw input |
| TTFT improvement | First chunk delivered in ≤ 50ms via streaming |
| Safety | Zero back-substitution collisions, explicit control over semantic loss |

### 1.3 Out of Scope

- Direct LLM API calls (Anthropic / OpenAI SDK integration is the user's responsibility)
- Embedding generation
- Vector DB storage

---

## 2. Architecture Overview

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

## 3. Module Specifications

### 3.1 `ir.rs` — Intermediate Representation

#### Type Definitions

```rust
pub enum FidelityLevel {
    Lossless,   // Audit / legal: 100% original content preserved
    Semantic,   // General RAG: semantic-unit compression
    Compressed, // Summarization pipelines: maximum compression
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

#### Invariants

- `importance` value range: `0.0..=1.0`
- If `token_budget` is `Some(n)`, the rendered output token count is guaranteed to be ≤ `n`
- `Compressed`-stage compression is forbidden at `FidelityLevel::Lossless`

---

### 3.2 `symbol.rs` — SymbolDict

#### Design Principles

- Substitution symbols use Unicode **Private Use Area** (`U+E000–U+F8FF`)
  → Prevents back-substitution collisions with visible patterns like `$1`, `$2`
- The global dictionary is output only once at the top of the document in the `<D>` tag
- `intern()` / `decode_str()` pair provides fully symmetric encode ↔ decode

#### Interface

```rust
impl SymbolDict {
    pub fn new() -> Self;
    pub fn intern(&mut self, term: &str) -> char;
    pub fn decode_str(&self, input: &str) -> String;
    pub fn render_dict_header(&self) -> String;  // generates <D> block
}
```

#### Constraints

- Returns `SymbolTableOverflow` error when exceeding the PUA upper limit `U+F8FF`
- Re-interning the same term returns the same symbol (idempotency guaranteed)

---

### 3.3 `compressor.rs` — AdaptiveCompressor

#### Compression Strategy (by stage)

| Budget usage rate | Strategy applied |
|------------|-----------|
| 0–60%      | Stopword removal only |
| 60–80%     | Stopwords + remove bottom 20% paragraphs by importance |
| 80–95%     | Above + duplicate sentence removal + numeric data linearization |
| 95%+       | Above + all paragraphs → 1-sentence summary (Semantic only) |

#### Numeric Data Linearization

- Row count ≤ 5: `Key:Val, Key:Val` sequence
- Row count > 5: JSON Lines (`{"k":"v",...}` 1 line/row)
- Markdown table symbols (`|`, `-`) fully removed

#### Interface

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

#### Output Format

```xml
<D>
t1=legal-termA
t2=domain-termB
</D>
<H>
t: document title
s: one-line summary
k: [keyword1, keyword2]
</H>
<B>
... body (compression and substitution applied) ...
</B>
```

- `<D>`: SymbolDict global dictionary (omitted if no substitutions)
- `<H>`: YAML serialized header (serde-norway, YAML 1.2 compliant)
- `<B>`: Body (line breaks and whitespace minimized)

#### Interface

```rust
pub fn render_node(node: &DocNode, dict: &SymbolDict) -> String;
pub fn render_full(doc: &IRDocument, dict: &mut SymbolDict) -> String;
```

---

### 3.5 `stream.rs` — Streaming Transpiler

#### Chunk Definition

```rust
pub struct TranspileChunk {
    pub sequence:    usize,
    pub content:     String,
    pub token_count: usize,   // pre-calculated by tiktoken-rs
    pub is_final:    bool,
}
```

#### Streaming Pipeline

```rust
pub async fn transpile_stream(
    source:  impl AsyncRead + Unpin + Send + 'static,
    budget:  usize,
    fidelity: FidelityLevel,
) -> impl Stream<Item = Result<TranspileChunk>>;
```

- Tokio-based async stream
- Chunks split at semantic unit (paragraph/section) boundaries
- Automatically switches to `Compressed` mode when budget reaches 80%
- First chunk always includes `<D>` + `<H>`

---

## 4. Public API (`lib.rs`)

```rust
/// Synchronous conversion — processes the entire document at once
pub fn transpile(
    input:    &str,
    format:   InputFormat,
    fidelity: FidelityLevel,
    budget:   Option<usize>,
) -> Result<String, TranspileError>;

/// Asynchronous streaming conversion
pub async fn transpile_stream(
    source:   impl AsyncRead + Unpin + Send + 'static,
    fidelity: FidelityLevel,
    budget:   usize,
) -> impl Stream<Item = Result<TranspileChunk>>;

/// Token count pre-calculation utility
pub fn token_count(text: &str, model: TokenModel) -> usize;

pub enum InputFormat { PlainText, Markdown, Html, Pdf }
pub enum TokenModel  { Gpt4, Gpt35, Llama3, Claude3 }
```

---

## 5. Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum TranspileError {
    #[error("Parse failed: {0}")]
    ParseError(String),

    #[error("Symbol table overflow (max {max} symbols)")]
    SymbolTableOverflow { max: usize },

    #[error("Token budget exceeded: required {required}, budget {budget}")]
    BudgetExceeded { required: usize, budget: usize },

    #[error("Compression attempted in Lossless mode")]
    LosslessModeViolation,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

---

## 6. Dependencies (Cargo.toml)

```toml
[dependencies]
# Parsing
lopdf          = "0.31"
html5ever      = "0.27"
pulldown-cmark = "0.11"

# Serialization
serde          = { version = "1", features = ["derive"] }
serde_json     = "1"
serde-norway   = "0.9"   # YAML 1.2 compliant (replaces serde_yaml)

# Token counting
tiktoken-rs    = "0.5"
tokenizers     = "0.19"

# Async
tokio          = { version = "1", features = ["full"] }
tokio-stream   = "0.1"
futures        = "0.3"

# Utilities
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

## 7. Non-Functional Requirements

| Item | Requirement |
|------|----------|
| Thread safety | `SymbolDict` is single-document only; independent instance per document when processing in parallel |
| Memory | Heap usage ≤ 10MB when processing a 1MB input document |
| Test coverage | Core modules (ir, symbol, compressor) ≥ 80% |
| MSRV | Rust 1.75+ (async fn in traits stable) |

---

## 8. Implementation Roadmap

| Stage | Task | Status |
|------|------|------|
| 1 | Cargo project initialization + Cargo.toml | 🔲 |
| 2 | `ir.rs` core types | 🔲 |
| 3 | `symbol.rs` SymbolDict | 🔲 |
| 4 | `renderer.rs` node renderer | 🔲 |
| 5 | `compressor.rs` AdaptiveCompressor | 🔲 |
| 6 | `stream.rs` Tokio streaming | 🔲 |
| 7 | `lib.rs` public API integration | 🔲 |
| 8 | Unit tests + benchmarks | 🔲 |
