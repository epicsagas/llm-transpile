# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`llm-transpiler` — A high-performance Rust library that converts raw documents (Markdown, HTML, PlainText) into a structured bridge format (`<D>?<H><B>`) that LLMs can consume with minimal tokens.

- MSRV: Rust 1.92+
- Goal: ≥10× parsing speed vs. Python, 15–30% token reduction vs. raw input

## Commands

```bash
# Build
cargo build
cargo build --release

# Run all tests
cargo test

# Test specific modules
cargo test --lib ir::tests
cargo test --lib symbol::tests
cargo test --lib compressor::tests
cargo test --lib renderer::tests

# Run a single test function
cargo test intern_idempotent

# Benchmarks (HTML report: target/criterion/)
cargo bench

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt
```

## Architecture

Pipeline: `parser.rs` → `ir.rs` → `compressor.rs` + `symbol.rs` → `renderer.rs` → `stream.rs`

### Module Roles

| File | Role |
|------|------|
| `lib.rs` | Public API (`transpile`, `transpile_stream`, `token_count`) |
| `ir.rs` | Language-neutral IR — `DocNode`, `IRDocument`, `FidelityLevel` |
| `parser.rs` | Markdown/HTML/PlainText → `IRDocument` (internal module) |
| `compressor.rs` | 4-stage adaptive compression based on token budget usage rate |
| `symbol.rs` | `SymbolDict` that substitutes domain terms with Unicode PUA (`U+E000–U+F8FF`) |
| `renderer.rs` | `DocNode` → bridge text, table linearization, YAML header assembly |
| `stream.rs` | Tokio-based streaming `TranspileChunk` generation |

### Core Invariants

- Compression is strictly forbidden at `FidelityLevel::Lossless` (`AdaptiveCompressor::compress` returns immediately)
- `SymbolDict` is a per-document independent instance — must not be shared across threads
- Exceeding the PUA symbol allocation limit (`U+F8FF`) returns `SymbolOverflowError`
- `importance` range: `0.0..=1.0`

### Output Format

```
<D>          ← SymbolDict dictionary (omitted if no substitutions)
SymA=TermA
</D>
<H>          ← YAML header (t/s/k keys only)
t: title
s: summary
k: [kw1, kw2]
</H>
<B>          ← body (compression and substitution applied)
...
</B>
```

### Compression Stages (by budget usage rate)

| Usage rate | `CompressionStage` | Behavior |
|--------|-------------------|------|
| 0–60% | `StopwordOnly` | Stopword removal |
| 60–80% | `PruneLowImportance` | + Remove bottom 20% paragraphs by importance |
| 80–95% | `DeduplicateAndLinearize` | + Duplicate sentence removal |
| 95%+ | `MaxCompression` | + Paragraphs → first sentence only (Semantic and above) |

## Version Bump Checklist

When creating a new release tag, update ALL of the following to the same version:

| File | Field | Example |
|------|-------|---------|
| `Cargo.toml` | `version = "x.y.z"` | `0.1.6` |
| `.claude-plugin/plugin.json` | `"version": "x.y.z"` | `0.1.6` |
| Git tag | `vx.y.z` | `v0.1.6` |

All three must match before tagging.

## Release Success Route

Verified working release path (2026-05-12):

1. **Version bump**: `Cargo.toml`, `.claude-plugin/plugin.json` → same version, no `-dev` suffix
2. **plugin.json rules**:
   - `keywords` 배열 마지막 항목 뒤에 trailing comma 금지 (`],` → `]`)
   - `hooks` 필드 금지 — `hooks/hooks.json`은 Claude Code가 자동 로드하므로 명시하면 중복 에러 발생
3. **dist-workspace.toml**: `extra-artifacts` 블록 사용 금지 (cargo-dist 0.31은 `build` 필드를 시퀀스로 요구하며 정적 파일 첨부 불가)
4. **release.yml**: `host` 잡의 Cleanup 스텝 직후 `cp install.sh install.ps1 artifacts/` 스텝 추가 → `gh release create artifacts/*`에 포함됨
5. **Commit & tag**: `git tag vx.y.z && git push origin main --tags`
