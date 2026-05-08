# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[0.2.0]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.2.0
[0.1.5]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.1.5
[0.1.4]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.1.4
[0.1.3]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.1.3
[0.1.2]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.1.2
[0.1.1]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.1.1
[0.1.0]: https://github.com/epicsagas/llm-transpile/releases/tag/v0.1.0
