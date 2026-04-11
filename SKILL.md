# SKILL.md — llm-transpiler Developer Skill Reference

This document defines the Claude Code skills and development workflows used in this project.
Use these as slash commands in Claude Code (`/skill-name`) or as a reference for common tasks.

---

## Table of Contents

- [Project Skills](#project-skills)
- [Development Workflow](#development-workflow)
- [Module-Specific Skills](#module-specific-skills)
- [Release Skills](#release-skills)
- [Evaluation Skills](#evaluation-skills)

---

## Project Skills

### `/epic:go` — Implement a feature

Use when adding a new feature or fixing a bug. Follows TDD — writes tests first, then implements.

**Trigger conditions**
- New `DocNode` variant
- New compression stage
- New input format support
- Bug fix in any module

**Expected outputs**
- Test in the relevant module (`ir::tests`, `compressor::tests`, etc.)
- Implementation with zero clippy warnings
- `cargo test` passing

---

### `/epic:check` — Pre-commit verification

Runs code review + security audit + performance analysis in parallel.

**Run before every commit** to `main`.

```bash
# Equivalent manual steps
cargo test
cargo clippy -- -D warnings
cargo fmt --check
cargo build --release
```

---

### `/epic:ship` — Commit, verify, and push

Verifies the full pipeline (tests → clippy → fmt) then commits and pushes to `main`.

**Red flags that block ship**
- Any failing test
- Clippy warning (`-D warnings` is enforced)
- Unformatted files (`cargo fmt --check` fails)
- Compression logic touching `FidelityLevel::Lossless`

---

### `/epic:spec` — Write or update a spec

Use when proposing changes to the bridge format, compression stages, or public API.
Output lands in `SPEC.md`.

---

## Development Workflow

### Adding a new `DocNode` variant

1. Define the variant in `src/ir.rs` — match arms in `importance()` and `char_len()` must be exhaustive.
2. Handle in `src/parser.rs` — add parsing logic for each `InputFormat`.
3. Handle in `src/renderer.rs` — add `render_node()` branch.
4. Handle in `src/compressor.rs` — decide whether the node participates in pruning.
5. Add unit tests in the relevant `#[cfg(test)]` block.
6. Run `/epic:check` before committing.

### Adding a new compression stage

- Stages are ordered by budget usage rate (`0–60`, `60–80`, `80–95`, `95+`).
- The new stage **must not activate** when `fidelity == FidelityLevel::Lossless`.
- Add a test in `compressor::tests` that verifies `Lossless` bypass.
- Update the compression table in `CLAUDE.md` and `README.md`.

### Modifying the bridge output format (`<D>/<H>/<B>`)

- The format is a **breaking change** — bump `version` in `Cargo.toml` accordingly.
- Update `SPEC.md` §3.4, `CLAUDE.md` "Output Format", and `README.md` "Output Format".
- Existing downstream consumers parse `<B>` and `<H>` — do not rename tags.

---

## Module-Specific Skills

### `ir.rs`

| Task | Command |
|------|---------|
| Test IR invariants | `cargo test --lib ir::tests` |
| Check importance range | `cargo test doc_node_importance_defaults` |
| Check metadata lookup | `cargo test ir_document_metadata_lookup` |

**Key invariants to preserve**
- `importance` is always in `0.0..=1.0`
- `FidelityLevel::Lossless` never sets `allows_compression() = true`

---

### `symbol.rs`

| Task | Command |
|------|---------|
| Test all symbol behavior | `cargo test --lib symbol::tests` |
| Test idempotency | `cargo test intern_idempotent` |
| Test overflow | `cargo test overflow_returns_error` |
| Test roundtrip | `cargo test encode_decode_roundtrip` |

**Key invariants to preserve**
- PUA range: `U+E000–U+F8FF` (6,400 slots)
- `intern()` is idempotent — same term always returns the same char
- No collision with visible ASCII patterns (`$`, `%`, digits)

---

### `compressor.rs`

| Task | Command |
|------|---------|
| Test all compression | `cargo test --lib compressor::tests` |
| Test Lossless bypass | `cargo test lossless_skips_all_compression` |
| Test stopword removal | `cargo test stopword_removal_works` |
| Test pruning | `cargo test prune_low_importance_removes_bottom_20_pct` |

---

### `renderer.rs`

| Task | Command |
|------|---------|
| Test all rendering | `cargo test --lib renderer::tests` |
| Test table formats | `cargo test table_small_key_val_format` |
| Test full output | `cargo test render_full_structure` |

---

### `stream.rs`

| Task | Command |
|------|---------|
| Test streaming | `cargo test --lib stream::tests` |
| Test TTFT (first chunk) | `cargo test first_chunk_contains_header` |
| Test budget cutoff | `cargo test budget_triggers_force_final` |

---

### `transpile` CLI binary

| Task | Command |
|------|---------|
| Build CLI | `cargo build --bin transpile` |
| Run on a file | `./target/debug/transpile --input doc.md` |
| Count tokens only | `./target/debug/transpile --input doc.md --count` |
| JSON output | `./target/debug/transpile --input doc.md --json` |
| stdin pipe | `cat doc.md \| ./target/debug/transpile` |

---

## Release Skills

### `/add-license apache-2`

Applies the Apache-2.0 license template and updates `Cargo.toml` and `README.md`.

- License owner: `epicsagas`
- Template: `~/.claude/commands/LICENSE-Apache-2.0.txt`
- Updates: `LICENSE`, `Cargo.toml` (`license = "Apache-2.0"`), README badge

### `/add-funding`

Creates `.github/FUNDING.yml` and adds the Buy Me a Coffee badge to `README.md`.

- GitHub: `epicsagas`
- Buy Me a Coffee: `epicsaga`

### Pre-publish checklist

Before publishing to crates.io:

```bash
# 1. Verify all tests pass
cargo test

# 2. Check lint and format
cargo clippy -- -D warnings
cargo fmt --check

# 3. Dry-run publish
cargo publish --dry-run

# 4. Confirm Cargo.toml fields
#    name, version, description, license, repository, rust-version
```

---

## Evaluation Skills

Run the eval suite to check token reduction metrics across the dataset:

```bash
# Full quantitative evaluation (reduction %, throughput, lossless integrity)
cargo run --example eval

# Integration test with real documents
cargo run --example test_docs

# Benchmark (HTML report → target/criterion/)
cargo bench
```

**Dataset locations**

| Directory | Contents |
|-----------|----------|
| `eval/dataset/policy/` | Internal policy documents (Markdown) |
| `eval/dataset/hf/` | Hugging Face open-source documentation (Markdown) |

**Target metrics**

| Metric | Target |
|--------|--------|
| Semantic token reduction | ≥ 15% |
| Compressed token reduction | ≥ 25% |
| Lossless integrity | 100% (all sampled words preserved) |
| Throughput | ≥ 100 tok/ms |

---

## Quick Reference

```
/epic:go      → Implement (TDD: test → code → verify)
/epic:check   → Review + audit + perf analysis
/epic:ship    → Test + lint + fmt + commit + push
/epic:spec    → Write or update SPEC.md
/add-license  → Apply license template
/add-funding  → Add FUNDING.yml + badge
```
