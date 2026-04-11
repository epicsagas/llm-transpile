# LLM Transpiler — Technical Specification (SPEC)

> **Version**: 0.2.0
> **Date**: 2026-04-11
> **Status**: Current (reflects v0.1.x implementation)

---

## 1. Project Overview

### 1.1 Purpose

A high-performance Rust library that converts raw documents (HTML, Markdown, Plain Text)
into a **structured bridge format** that allows LLM agents to receive the maximum
amount of information with the minimum number of tokens.

### 1.2 Core Goals

| Goal | Metric |
|------|------|
| Parse speed | ≥ 10× improvement over Python |
| Token reduction | 15–30% reduction vs. raw input |
| TTFT improvement | First chunk delivered in ≤ 50ms via streaming |
| Safety | Zero back-substitution collisions, explicit control over semantic loss |

### 1.3 Out of Scope (current version)

- PDF input — requires pre-conversion to Markdown or Plain Text
- Direct LLM API calls — the caller integrates the output with any LLM SDK
- Embedding generation or vector DB storage

---

## 2. Architecture Overview

```
┌───────────────────────────────────────────────────────┐
│                   Public API (lib.rs)                 │
│  transpile()  /  transpile_stream()  /  token_count() │
└───────────────────────┬───────────────────────────────┘
                        │
          ┌─────────────▼──────────────┐
          │   parser.rs                │
          │   pulldown-cmark (MD)      │
          │   ammonia (HTML strip)     │
          │   PlainText splitter       │
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
          │   renderer.rs            │
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
    Para     { text: String, importance: f32 },  // importance ∈ [0.1, 1.0]
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

- `importance` value range: `0.1..=1.0`
- `FidelityLevel::Lossless` forbids all compression stages

---

### 3.2 `parser.rs` — Input Parsers

#### Supported formats

| Format | Parser | Notes |
|--------|--------|-------|
| `Markdown` | `pulldown-cmark` (CommonMark + GFM tables) | Primary format |
| `Html` | `ammonia` tag-stripping → PlainText delegate | Tag-safe HTML sanitisation |
| `PlainText` | Blank-line paragraph splitter | `# ` / `## ` prefix → headings |

> **PDF**: Not supported. Pre-convert to Markdown or Plain Text before calling the API.

#### Paragraph importance scoring

Each `DocNode::Para` receives an importance score computed from three signals:

| Signal | Weight | Rationale |
|--------|--------|-----------|
| Position index | 50 % | Inverted-pyramid — earlier paragraphs introduce the topic |
| Character length | 40 % | Short paragraphs (< 40 chars) are likely captions or footnotes |
| Heading proximity | 10 % | Paragraph immediately after a heading is a section intro |

Score formula: `clamp(position × 0.5 + length × 0.4 + heading_bonus × 0.1, 0.1, 1.0)`

---

### 3.3 `symbol.rs` — SymbolDict

#### Design Principles

- Substitution symbols use Unicode **Private Use Area** (`U+E000–U+F8FF`) — 6 144 slots
- Zero reverse-substitution collisions vs. visible patterns like `$1`, `$2`
- The global dictionary is output only once in the `<D>` tag
- `intern()` / `decode_str()` pair provides fully symmetric encode ↔ decode
- Internal Aho-Corasick automaton cache uses `std::sync::RwLock` → type is `Send + Sync`

#### Interface

```rust
impl SymbolDict {
    pub fn new() -> Self;
    pub fn intern(&mut self, term: &str) -> Result<char, SymbolOverflowError>;
    pub fn encode_str(&self, input: &str) -> String;
    pub fn decode_str(&self, input: &str) -> String;  // test-only
    pub fn render_dict_header(&self) -> String;        // generates <D> block
}
```

#### Thread safety

`SymbolDict` is `Send + Sync`. For concurrent mutation use `Arc<Mutex<SymbolDict>>`.

---

### 3.4 `compressor.rs` — AdaptiveCompressor

#### Compression Strategy (by stage)

| Budget usage rate | Strategy applied |
|------------|-----------|
| 0–60% | Stopword removal only |
| 60–80% | Stopwords + prune bottom-20% paragraphs by importance |
| 80–95% | Above + duplicate sentence removal + numeric data linearisation |
| 95%+ | Above + all paragraphs → first sentence only (Semantic+) |

#### Stopword matching

- **ASCII stopwords**: `\b word \b` regex (case-insensitive) — correct word-boundary semantics.
- **Non-ASCII stopwords** (Korean connectives, CJK, Arabic, …): exact whitespace-token
  matching — avoids `\b` Unicode-boundary issues.
- **Default list**: common English function words + Korean standalone connectives.
  No Korean grammatical particles (은/는/이/가/…) — morphological analysis would
  be required and is out of scope.

#### Interface

```rust
pub struct CompressionConfig {
    pub budget:         usize,
    pub current_tokens: usize,
    pub fidelity:       FidelityLevel,
}

impl AdaptiveCompressor {
    pub fn new() -> Self;                                         // default stopwords
    pub fn with_stopwords(stopwords: Vec<String>) -> Self;        // custom stopwords
    pub fn compress(&self, nodes: Vec<DocNode>, cfg: &CompressionConfig) -> Vec<DocNode>;
}
```

---

### 3.5 `renderer.rs` — Renderer

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

- `<D>`: SymbolDict global dictionary (omitted when empty)
- `<H>`: YAML-like header block
- `<B>`: Body content

#### Interface

```rust
pub fn render_node(node: &DocNode, dict: &SymbolDict) -> String;
pub fn render_full(doc: &IRDocument, dict: &mut SymbolDict) -> String;
pub fn build_yaml_header(doc: &IRDocument) -> String;
pub fn linearize_table(headers: &[String], rows: &[Vec<String>]) -> String;
```

---

### 3.6 `stream.rs` — Streaming Transpiler

#### Chunk definition

```rust
pub struct TranspileChunk {
    pub sequence:    usize,
    pub content:     String,
    pub token_count: usize,   // heuristic (default) or tiktoken cl100k_base (feature flag)
    pub is_final:    bool,
}
```

#### StreamingTranspiler

```rust
impl StreamingTranspiler {
    /// Default — empty symbol dictionary.
    pub fn new(budget: usize, fidelity: FidelityLevel) -> Self;

    /// Pre-populated symbol dictionary for domain-specific streaming compression.
    pub fn with_dict(budget: usize, fidelity: FidelityLevel, dict: SymbolDict) -> Self;

    pub fn transpile(self, doc: IRDocument)
        -> Pin<Box<dyn Stream<Item = Result<TranspileChunk, StreamError>> + Send>>;
}
```

#### Streaming behaviour

- First chunk always contains `<D>` (if non-empty) + `<H><B>` — minimises TTFT.
- Automatically switches to `Compressed` fidelity at 80% budget usage.
- Symbol substitution is **available** when a pre-populated dict is supplied via `with_dict`.

---

## 4. Public API (`lib.rs`)

```rust
/// Synchronous conversion.
pub fn transpile(
    input:    &str,
    format:   InputFormat,
    fidelity: FidelityLevel,
    budget:   Option<usize>,
) -> Result<String, TranspileError>;

/// Asynchronous streaming conversion.
pub async fn transpile_stream(
    input:    &str,
    format:   InputFormat,
    fidelity: FidelityLevel,
    budget:   usize,
) -> Pin<Box<dyn Stream<Item = Result<TranspileChunk, StreamError>> + Send>>;

/// Token count utility (heuristic by default; accurate with `tiktoken` feature).
pub fn token_count(text: &str) -> usize;

pub enum InputFormat { PlainText, Markdown, Html }
```

---

## 5. Error Types

```rust
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
```

---

## 6. Dependencies (Cargo.toml)

### Core (always compiled)

```toml
pulldown-cmark = "0.11"      # Markdown parsing (CommonMark + GFM tables)
ammonia        = "4"         # HTML tag stripping
serde          = { version = "1", features = ["derive"] }
serde_json     = "1"
tokio          = { version = "1", features = ["full"] }
tokio-stream   = "0.1"
futures        = "0.3"
aho-corasick   = "1"         # Multi-pattern string matching for SymbolDict
regex          = "1"
once_cell      = "1"
itertools      = "0.13"
thiserror      = "1"
clap           = { version = "4", features = ["derive"] }  # CLI binary
```

### Optional features

```toml
[features]
default  = []
tiktoken = ["dep:tiktoken-rs"]   # Accurate token counting (cl100k_base)

[dependencies]
tiktoken-rs = { version = "0.5", optional = true }
```

> **Note**: `tiktoken-rs` adds ~5 MB to the binary and significantly increases
> compile time. Enable only when token-budget accuracy is critical in production.

---

## 7. Non-Functional Requirements

| Item | Requirement |
|------|-------------|
| Thread safety | `SymbolDict` is `Send + Sync`; independent instance recommended per document |
| Memory | Heap usage ≤ 10 MB when processing a 1 MB input document |
| Test coverage | Core modules (ir, symbol, compressor, parser) ≥ 80 % |
| MSRV | Rust 1.92 (`std::sync::OnceLock`, `async fn` in traits stable) |
| Warnings | Zero compiler warnings (`cargo clippy -- -D warnings`) |

---

## 8. Known Limitations

| Limitation | Detail | Mitigation |
|-----------|--------|------------|
| Token counting accuracy | Default heuristic can be 2–3× off for Korean/CJK | Enable `tiktoken` feature |
| Korean morphological analysis | Grammatical particles (은/는/이/가…) not stripped | Use `with_stopwords` + KoNLP pre-processing |
| Streaming symbol substitution | Only pre-populated dictionaries work in streaming | Use `StreamingTranspiler::with_dict` |
| PDF input | Not supported | Pre-convert with `pdf2md` or `pdftotext` |
| Lossless integrity | 90.9 % on evaluation corpus (Apache licence misclassified) | Under investigation |

---

## 9. Roadmap

| Priority | Task | Notes |
|----------|------|-------|
| P0 | Lossless mode 100 % integrity | Fix licence-header misclassification |
| P1 | Korean morphological stop-list | Integrate `lindera` or `mecab-ko` |
| P1 | Streaming two-pass symbol analysis | Collect terms → encode on second pass |
| P2 | PDF input support | Integrate `pdf-extract` or `lopdf` |
| P2 | MSRV bump to 1.80 → replace `once_cell` with `std::sync::OnceLock` | |
| P3 | Per-language token heuristic calibration | Benchmark against real tokenisers |
