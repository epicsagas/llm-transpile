# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
