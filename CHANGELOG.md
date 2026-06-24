# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] — 2026-06-23

### Summary

Makes token-reduction measurement **non-self-referential** and raises the
PUA substitution ROI gate to the empirically-measured break-even point.
The eval harness now reports both the heuristic and the real cl100k BPE
token counts; the previous heuristic-only reporting inflated reduction on
PUA-heavy output because it baked in the same "PUA = 1 token" assumption
the compressor optimizes for.

### Added

- **Token-honesty measurement API** (`src/lib.rs`): `TokenMethod`,
  `TokenMeasurement`, `DualTokenMeasurement`, `bpe_token_count`,
  `measure_tokens`, `measure_tokens_dual`. The dual measurement reports
  the heuristic count alongside the real cl100k count so reduction claims
  are auditable.
- `pub const PUA_TOKEN_COST: usize = 3` — the measured cl100k cost of a
  single PUA character (byte-fallback), feature-independent.
- `stream::estimate_tokens_heuristic` extracted and always compiled, so
  dual measurement genuinely differs under the `tiktoken` feature (the
  heuristic path is no longer compiled out).
- `eval` harness `--json` mode: emits a single JSON object with a
  `composite` score (0.0–1.0) derived from the **BPE** numbers, plus the
  heuristic numbers for transparency. `make eval` → JSON,
  `make eval-report` → human-readable table (preserved).
- `eval.yaml` wired to `epic eval` via `result_type: composite`; the
  `eval` example now `required-features = ["tiktoken"]`.
- Tests pinning the PUA ground truth (3 tokens/char), the substitution
  break-even (4+ token terms only), and a long-high-frequency
  positive-ROI interning case.

### Changed

- **PUA ROI gate raised to 4+ tokens** (`auto_intern_frequent_terms`).
  The gate previously assumed a PUA char costs 1 token; the real cl100k
  cost is 3, so substituting ordinary words (most are 1–2 tokens)
  *increased* the token count. The gate now requires
  `term_tokens > PUA_TOKEN_COST` (4+) **and** a strictly positive net
  saving after dictionary overhead.
  - Measured effect (cl100k): `hub-docs_model_cards_metadata.md` PUA
    50→0, `diffusers_dreambooth_training.md` PUA 35→0.
  - Real token reduction improved: BPE semantic 80.3%→**81.5%**,
    compressed 91.5%→**91.8%**.

### Fixed

- Dual measurement bug: under `tiktoken`, the heuristic field reported the
  BPE value (the heuristic path was compiled out). `measure_tokens_dual`
  now always uses `estimate_tokens_heuristic` so the two fields differ.
- cfg-split the ROI-gated stopword tests (`compressor.rs`) to assert the
  correct behavior under each tokenizer (the heuristic and BPE disagree
  on common-word token counts).

## [0.3.3] — 2026-06-12

### Added

- Plugin renamed to `llm-transpile` across Claude and Codex manifests; category set to Performance
- `composerIcon` field added to Codex plugin manifest; icon moved to `assets/`

### Fixed

- CI: SHA-256 checksums generated for install scripts on release
- Hooks: `PostToolUse` replaced with `PreToolUse` (broken hook fixed)
- Plugin: marketplace review feedback addressed
- Funding: removed invalid GitHub Sponsors entry, kept Buy Me a Coffee

### Changed

- Install script consolidated to single file with brew + binstall cascade
- Bump `tiktoken-rs` from 0.11.0 to 0.12.0
- Bump `serde_json` from 1.0.149 to 1.0.150
- Bump `pulldown-cmark` from 0.13.3 to 0.13.4

## [0.3.2] — 2026-05-27

### Added

- Antigravity plugin package with install docs and i18n sync
- Plugin folder structure aligned with `registry/` pattern

### Changed

- Bump `serde_json` from 1.0.149 to 1.0.150
- Bump `pulldown-cmark` from 0.13.3 to 0.13.4

## [0.3.1] - 2026-05-25

### Fixed

- Add document extension filter to PostToolUse hook — skip non-document files

## [0.3.0] - 2026-05-25

### Added

- Shell and PowerShell installers for standalone setup
- Cline integration in `transpile install`
- Antigravity (formerly gemini-cli) integration
- Benchmark summary in README

### Changed

- Remove claude and codex from `transpile install` — now managed via plugins
- Add hooks to codex manifest, sync versions across manifests
- Sync all README translations with install restructure and stats fix

## [0.2.5] - 2026-05-21

### Added

- 8 token efficiency improvements across compression pipeline

### Fixed

- Prevent "us" pronoun from being treated as abbreviation

### Changed

- Update README translations for 10 languages

## [0.2.4] - 2026-05-19

### Added

- Codex CLI plugin manifest and marketplace integration
- Codex CLI installation instructions in README

### Fixed

- Skip manual seed when running as Claude Code plugin
- Clippy warning: collapse nested if

### Changed

- Upgrade README badges to for-the-badge style with GitHub stats row
- Bump GitHub Actions: upload-artifact 7, download-artifact 8, checkout 6, attest-build-provenance 4
- Bump tokio 1.52.3, clap 4.6.1

## [0.2.3] - 2026-05-13

### Fixed

- Correct codex skill path from `~/.agents` to `~/.codex`

### Changed

- Rewrite PostToolUse hook from bash to Node.js for Windows support

## [0.2.2] - 2026-05-12

### Added

- Stats dashboard with date range picker, project tracking and security hardening
- Auto-update binary when plugin version is newer

### Fixed

- Upload install scripts via release.yml instead of broken extra-artifacts config

## [0.2.1] - 2026-05-12

### Added

- cargo-binstall metadata for pre-built binary discovery
- Release pipeline migrated to cargo-dist with macOS code signing
- Release profile, Scoop channel, and Windows ARM64 target
- Cross-platform Node.js installer replacing bash bootstrap in plugin hooks

### Fixed

- CI: unify ci.yml with check/test/audit/sbom jobs
- CI: add rust-toolchain.toml and crates.io publish job
- CI: allow dirty Cargo.lock in cargo publish
- Plugin hooks path added to manifest

### Changed

- Remove macOS notarization (cargo-dist 0.31.0 limitation)
- Sync all README translations

## [0.2.0] - 2026-05-09

### Added

- Abbreviation-aware `first_sentence` — skips periods in Dr., U.S., e.g., i.e., Fig., etc.
- PUA character token estimation (cpt=1) in heuristic estimator
- Auto-discovery of frequent terms for SymbolDict substitution (min_freq=3, max_terms=50)
- Header-body duplicate text removal (Semantic/Compressed modes)
- Code block comment stripping and blank line collapsing (Semantic+ fidelity)
- Jaccard similarity-based fuzzy paragraph dedup (threshold ≥ 0.85)
- 40% cap on paragraph pruning to prevent over-removal

### Fixed

- "us" pronoun no longer confused with "U.S." abbreviation
- Duplicate Korean stopword "다만" removed
- `dtolnay/rust-toolchain` missing required `toolchain` input in CI workflows
- `anchore/sbom-action` pinned SHA no longer resolvable (updated to v0.24.0)

### Changed

- Bump tiktoken-rs from 0.5.9 to 0.11.0
- Bump criterion from 0.5.1 to 0.8.2

## [0.1.5] - 2026-04-13

### Added

- `TRANSPILE_AGENT` environment variable injection per tool integration
- `--print-hook-script` CLI flag for debugging hook scripts

### Fixed

- Use `CLAUDE_PLUGIN_ROOT` instead of `CLAUDE_PLUGIN_DIR` for Claude Code plugin
- Fallback to default install path when plugin root environment variable is unset
- Clippy warnings in `print_hook_script` test and transpile bin

## [0.1.4] - 2026-04-13

### Added

- `transpile stats` subcommand with JSONL logging and date/agent filtering
- Per-invocation token metrics logging to `~/.agents/transpile/stats/`

## [0.1.3] - 2026-04-13

### Fixed

- GitHub Actions secrets reference in conditional job expressions

### Changed

- CI skips `publish-crates` and `update-homebrew` when secrets are not configured

## [0.1.2] - 2026-04-13

### Fixed

- Token-per-millisecond precision, double-eval, and HTML coverage calculation in eval

## [0.1.1] - 2026-04-13

### Fixed

- Enforce 10 MiB input limit to prevent denial-of-service on large inputs
- HTML entity PUA bypass — sanitize entities before symbol substitution

### Changed

- Compression optimization with Aho-Corasick pattern matching
- Reduced memory allocations in hot path

## [0.1.0] - 2026-04-12

### Added

- Core transpilation pipeline: Markdown / HTML / PlainText → `<D>?<H><B>` bridge format
- Adaptive 4-stage compression based on token budget usage rate
- `SymbolDict` with Unicode PUA substitution (up to 6,400 terms per document)
- Table linearization: compact `Key:Val` (≤5 rows) or pipe-separated rows
- Tokio-based streaming output with `TranspileChunk`
- Three fidelity levels: `Lossless`, `Semantic`, `Compressed`
- CLI binary with `--input`, `--format`, `--fidelity`, `--budget`, `--quiet`, `--stats`, `--json`, `--count` flags
- `transpile install` interactive wizard for Claude Code, Gemini CLI, Codex CLI, Cursor, OpenCode
- `transpile uninstall` with selective removal
- Claude Code plugin via marketplace (`epicsagas/plugins`)
- CI release workflow with 5-platform build matrix (Linux x86_64/ARM64, macOS Intel/ARM, Windows)
- Automated crates.io publish and Homebrew tap update
- Evaluation suite with 37 documents across 15 languages
- Apache-2.0 license

[0.4.0]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.4.0
[0.3.3]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.3.3
[0.3.2]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.3.2
[0.3.1]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.3.1
[0.3.0]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.3.0
[0.2.5]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.2.5
[0.2.4]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.2.4
[0.2.3]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.2.3
[0.2.2]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.2.2
[0.2.1]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.2.1
[0.2.0]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.2.0
[0.1.5]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.1.5
[0.1.4]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.1.4
[0.1.3]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.1.3
[0.1.2]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.1.2
[0.1.1]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.1.1
[0.1.0]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.1.0
