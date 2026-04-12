# llm-transpile

[![Crates.io](https://img.shields.io/crates/v/llm-transpile.svg)](https://crates.io/crates/llm-transpile)
[![docs.rs](https://docs.rs/llm-transpile/badge.svg)](https://docs.rs/llm-transpile)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/epicsaga)

**Token-optimized document transpiler for LLM pipelines**

Raw documents (Markdown, HTML, plain text) → structured bridge format `<D>?<H><B>` — with adaptive compression that keeps you under token budget.

```
<H>
t: Software License Agreement
s: Annual license terms between licensor and licensee
k: [license, contract, software]
</H>
<B>
# Contracting Parties
This agreement is made between Licensor and Licensee.
...
</B>
```

---

## Table of Contents

- [Why](#why)
- [Installation](#installation)
- [CLI Usage](#cli-usage)
- [Library Usage](#library-usage)
- [Output Format](#output-format)
- [Fidelity Levels](#fidelity-levels)
- [Adaptive Compression](#adaptive-compression)
- [Input Formats](#input-formats)
- [Error Handling](#error-handling)
- [Performance](#performance)
- [Contributing](#contributing)
- [License](#license)

---

## Why

LLMs perform better when context is clean and dense. This library handles the mechanical work:

- **Structural parsing** — Markdown/HTML/plain text → typed IR nodes (headings, paragraphs, tables, lists, code blocks)
- **Adaptive compression** — automatically escalates through 4 stages as token budget fills up
- **Symbol substitution** — repeated domain terms → Unicode PUA characters, decoded by `<D>` dictionary header
- **Table linearization** — Markdown tables → compact `Key:Val` sequences (≤5 rows) or pipe-separated rows (`h1|h2\nv1|v2`) for larger tables
- **Streaming output** — Tokio stream delivers the first chunk immediately, minimizing TTFT

---

## Installation

### Library (Rust crate)

```toml
[dependencies]
llm-transpile = "0.1"
```

Requires **Rust 1.75+**.

### CLI binary + tool integration

```bash
# Homebrew (macOS)
brew install epicsagas/tap/llm-transpile

# Pre-built binary (faster, no compile)
cargo binstall llm-transpile

# From crates.io
cargo install llm-transpile
```

Then configure tool integrations:

```bash
transpile install
```

`transpile install` launches an interactive wizard that detects and configures whichever tools are installed:

| Tool | Integration method | What it does |
|------|--------------------|--------------|
| **Claude Code** | PostToolUse hook | Auto-compresses `.md/.html/.txt` files on Read |
| **Gemini CLI** | `SKILL.md` | LLM auto-invokes `transpile` on document file extensions |
| **Codex CLI** | `SKILL.md` | LLM auto-invokes `transpile` on document file extensions |
| **Cursor** | `.mdc` rule (`alwaysApply`) | Triggers `transpile` before reading document files |
| **OpenCode** | `SKILL.md` | LLM auto-invokes `transpile` on document file extensions |

All non-Claude tools use a skill file that teaches the LLM to run `transpile --input <file>` automatically — no size check needed, extension alone triggers it.

**Selective install / uninstall**

```bash
transpile install claude gemini    # specific tools only
transpile install --all            # everything at once
transpile install --dry-run        # preview what would change
transpile install --list           # show status of all integrations

transpile uninstall cursor         # remove one
transpile uninstall --all          # remove everything
transpile uninstall --dry-run      # preview removals
```

**Claude Code plugin** (alternative — requires Claude Code with plugin support)

```
/plugin marketplace add epicsagas/claude-plugins
/plugin install transpile@epicsagas
```

Or from source:

```bash
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo install --path .
transpile install
```

---

## CLI Usage

```
transpile [OPTIONS]

Options:
  -i, --input <FILE>       Input file path (reads from stdin if omitted)
  -f, --format <FORMAT>    Input format: markdown | html | plaintext  [default: markdown]
                           Auto-detected from file extension when --input is used
  -l, --fidelity <LEVEL>   Compression level: lossless | semantic | compressed  [default: semantic]
  -b, --budget <N>         Token budget upper limit (unlimited if omitted)
  -c, --count              Print only the input token count, then exit
  -j, --json               Output as JSON {input_tok, output_tok, reduction_pct, content}
  -q, --quiet              Suppress the stats line on stderr
      --stats              Print stats line to stdout after content (single-stream capture)
  -h, --help               Print help
  -V, --version            Print version
```

**Examples**

```bash
# Convert a Markdown file (format auto-detected from .md extension)
transpile --input doc.md

# Read from stdin — clean stdout, stats on stderr
cat doc.html | transpile --format html --fidelity compressed --budget 1024

# Pipe cleanly — suppress stats entirely
transpile --input doc.md --quiet | send_to_llm_api

# Check token count without converting
transpile --input doc.md --count

# JSON output for scripts and pipelines
transpile --input doc.md --json | jq '.reduction_pct'

# Capture content + stats in one stream (stdout)
transpile --input doc.md --stats > output_with_stats.txt

# Lossless — no compression, full content preserved (legal/audit docs)
transpile --input contract.md --fidelity lossless

# Aggressive compression into a 512-token budget
transpile --input article.md --fidelity compressed --budget 512
```

> Stats (`[273 → 150 tok  45.1% reduction]`) are written to **stderr** by default, so stdout stays clean for piping. Use `--quiet` to suppress, or `--stats` to redirect to stdout.

---

## Library Usage

### Synchronous

```rust
use llm_transpiler::{transpile, FidelityLevel, InputFormat};

let md = r#"
# Software License Agreement

This agreement is made between Licensor and Licensee.

| Item     | Cost  |
|----------|-------|
| Base fee | $800  |
| Support  | $200  |
"#;

let output = transpile(md, InputFormat::Markdown, FidelityLevel::Semantic, Some(4096))?;
println!("{}", output);
```

### Streaming (Tokio)

```rust
use llm_transpiler::{transpile_stream, FidelityLevel, InputFormat};
use futures::StreamExt;

let mut stream = transpile_stream(input, InputFormat::Markdown, FidelityLevel::Semantic, 4096).await;

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

---

## Output Format

```
<D>                  ← Symbol dictionary (omitted when no substitutions occur)
{sym}=repeated-term
</D>
<H>                  ← YAML-like metadata header
t: document title
s: one-line summary
k: [keyword1, keyword2]
</H>
<B>                  ← Document body (compressed + substituted)
...content...
</B>
```

The `<D>` block uses Unicode Private Use Area characters (`U+E000–U+F8FF`) as compact symbol handles, avoiding collision with visible text patterns. The dictionary supports up to **6,400 unique terms** per document.

---

## Fidelity Levels

| Level | Typical use case | Compression applied |
|-------|-----------------|---------------------|
| `Lossless` | Legal / audit documents | None — original content guaranteed |
| `Semantic` | General RAG pipelines | Stopword removal + low-importance pruning |
| `Compressed` | Summarization, tight budgets | Maximum compression, first-sentence extraction |

---

## Adaptive Compression

The compressor monitors budget usage in real time and escalates automatically:

| Budget usage | Stage | What happens |
|---|---|---|
| 0–60% | `StopwordOnly` | English/Korean stopwords stripped |
| 60–80% | `PruneLowImportance` | Bottom 20% of paragraphs by importance score removed |
| 80–95% | `DeduplicateAndLinearize` | Duplicate sentences removed; tables linearized |
| 95%+ | `MaxCompression` | Each paragraph truncated to first sentence |

> `Lossless` mode bypasses all compression stages unconditionally.

During streaming, when budget usage crosses 80%, remaining nodes are automatically switched to `Compressed` mode.

---

## Input Formats

| `InputFormat` | Parser |
|---|---|
| `Markdown` | [pulldown-cmark](https://crates.io/crates/pulldown-cmark) — CommonMark + GFM tables |
| `Html` | ammonia sanitization → tag stripping → plain text pipeline |
| `PlainText` | Blank-line paragraph splitting |

---

## Error Handling

```rust
use llm_transpiler::TranspileError;

match transpile(input, format, fidelity, budget) {
    Ok(output) => { /* use output */ }
    Err(TranspileError::Parse(msg))          => eprintln!("parse failed: {msg}"),
    Err(TranspileError::SymbolOverflow(e))   => eprintln!("too many unique terms: {e}"),
    Err(TranspileError::LosslessModeViolation) => eprintln!("compression in lossless mode"),
    Err(e)                                   => eprintln!("error: {e}"),
}
```

---

## Performance

Measured on release build (`cargo build --release`), Apple M-series, 48 documents across Markdown / HTML / PlainText:

| Metric | Measured | Notes |
|--------|----------|-------|
| Throughput | **10,975 tok/ms** | ≈75× faster than Python parsing baseline |
| Semantic reduction | **33.9%** (Markdown) | 15–30% target met |
| Compressed reduction | **39.7%** (Markdown) | Budget-adaptive, guaranteed ≥ PruneLowImportance |
| Lossless word coverage | **98.8% avg** | Across all formats and languages |
| HTML reduction | **97.6%** | Reflects markup overhead removal (nav/scripts/styles) |
| Multilingual support | 15 languages tested | AR/DE/ES/FR/HI/IT/JA/KO/NL/PL/PT/RU/SV/TR/ZH — 99.4% avg word coverage |

Run the evaluation suite yourself:

```bash
cargo run --release --example eval
```

---

## Contributing

Contributions are welcome — bug reports, feature requests, and pull requests.

```bash
# Clone and build
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo build

# Run tests
cargo test

# Run benchmarks (HTML report → target/criterion/)
cargo bench

# Lint and format
cargo clippy -- -D warnings
cargo fmt
```

**Guidelines**

- Keep MSRV at Rust 1.75 — avoid features introduced after that.
- New compression behavior must not affect `Lossless` mode.
- Each PR should include tests for any new logic in the relevant module (`ir`, `compressor`, `symbol`, `renderer`).
- Run `cargo clippy -- -D warnings` and `cargo fmt` before submitting.

---

## License

Apache-2.0 — see [LICENSE](LICENSE).
