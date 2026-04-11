# llm-transpiler

**Token-optimized document transpiler for LLM pipelines**

Raw documents (Markdown, HTML, plain text) → structured bridge format — with adaptive compression that keeps you under token budget.

```
<H>
t: Software License Agreement
s: Annual license terms between licensor and licensee
k: [license, contract, software]
</H>
<B>
# 계약 당사자
본 계약은 갑(라이선서)과 을(라이선시) 사이에 체결됩니다.
...
</B>
```

## Why

LLMs perform better when context is clean and dense. This library handles the mechanical work:

- **Structural parsing** — Markdown/HTML/plain text → typed IR nodes (headings, paragraphs, tables, lists, code blocks)
- **Adaptive compression** — automatically escalates through 4 compression stages as token budget fills up
- **Symbol substitution** — repeated domain terms → Unicode PUA characters, decoded by `<D>` dictionary header
- **Table linearization** — Markdown tables → compact `Key:Val` sequences (≤5 rows) or JSON Lines (>5 rows)
- **Streaming output** — Tokio stream delivers the first chunk (header) immediately, minimizing TTFT

## Installation

```toml
[dependencies]
llm-transpiler = "0.1"
```

Requires Rust 1.75+.

## Usage

### Synchronous

```rust
use llm_transpiler::{transpile, FidelityLevel, InputFormat};

let md = r#"
# Software License Agreement

This agreement is made between Licensor and Licensee.

| Item     | Cost       |
|----------|------------|
| Base fee | $800       |
| Support  | $200       |
"#;

let output = transpile(md, InputFormat::Markdown, FidelityLevel::Semantic, Some(4096))?;
println!("{}", output);
```

### Streaming (Tokio)

```rust
use llm_transpiler::{transpile_stream, FidelityLevel};
use futures::StreamExt;

let mut stream = transpile_stream(input, FidelityLevel::Semantic, 4096).await;

while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    print!("{}", chunk.content);
    if chunk.is_final { break; }
}
```

### Token count estimate

```rust
let n = llm_transpiler::token_count("Hello, world!");
```

## Fidelity levels

| Level | Use case | Compression |
|-------|----------|-------------|
| `Lossless` | Legal / audit documents | None — original content guaranteed |
| `Semantic` | General RAG pipelines | Stopword removal, low-importance paragraph pruning |
| `Compressed` | Summarization pipelines | Maximum compression, first-sentence extraction |

## Adaptive compression stages

The compressor monitors token budget usage in real time and escalates automatically:

| Budget usage | Stage | What happens |
|---|---|---|
| 0–60% | Stopword removal | English/Korean stopwords stripped |
| 60–80% | Prune low importance | Bottom 20% of paragraphs by importance score removed |
| 80–95% | Deduplicate + linearize | Duplicate paragraphs removed; tables linearized |
| 95%+ | Max compression | Each paragraph truncated to first sentence (`Semantic`/`Compressed` only) |

During streaming, when budget reaches 80%, the compressor automatically switches to `Compressed` mode for remaining nodes.

## Output format

```
<D>              ← symbol dictionary (omitted if no substitutions)
{PUA_char}=term
</D>
<H>              ← YAML-like metadata header
t: document title
s: one-line summary
k: [keyword1, keyword2]
</H>
<B>              ← document body
...content...
</B>
```

The `<D>` block uses Unicode Private Use Area characters (`U+E000–U+F8FF`) as substitution symbols, avoiding collisions with visible patterns like `$1`, `$2`. The dictionary supports up to 6,400 unique terms per document.

## Input formats

| `InputFormat` | Parser |
|---|---|
| `Markdown` | pulldown-cmark (CommonMark + tables) |
| `Html` | Tag stripping + entity decoding → plain text pipeline |
| `PlainText` | Blank-line paragraph splitting |

## Supported metadata keys

Add `Metadata` nodes to populate the `<H>` header:

```rust
doc.push(DocNode::Metadata { key: "title".into(),    value: "My Document".into() });
doc.push(DocNode::Metadata { key: "summary".into(),  value: "One-line summary".into() });
doc.push(DocNode::Metadata { key: "keywords".into(), value: "key1, key2".into() });
```

## Error types

```rust
pub enum TranspileError {
    Parse(String),                                // parser failure
    SymbolOverflow(SymbolOverflowError),          // >6400 unique terms
    Stream(StreamError),                          // channel closed
    LosslessModeViolation,                        // compression attempted on Lossless doc
}
```

## Performance targets

| Metric | Target |
|--------|--------|
| Parse speed | ≥10× faster than Python baseline |
| Token reduction | 15–30% vs. raw input |
| Streaming TTFT | First chunk ≤ 50ms |
| Heap usage | ≤ 10MB for 1MB input document |

## Development

```bash
cargo test          # run all tests
cargo bench         # run benchmarks (HTML report → target/criterion/)
cargo clippy -- -D warnings
cargo fmt
```

## License

MIT
