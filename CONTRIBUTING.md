# Contributing to llm-transpile

Thank you for your interest in contributing. This guide covers everything you need to get started.

## Prerequisites

- **Rust 1.92+** (matches `rust-version` in `Cargo.toml`)
- Git

## Development Setup

```bash
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo build
```

## Development Commands

```bash
# Run all tests
cargo test

# Test a specific module
cargo test --lib ir::tests
cargo test --lib symbol::tests
cargo test --lib compressor::tests
cargo test --lib renderer::tests

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt

# Benchmarks (HTML report → target/criterion/)
cargo bench

# Run evaluation suite
cargo run --release --example eval
```

## Pull Request Process

1. Fork the repository and create a branch from `main`
2. Make your changes with clear, conventional commit messages
3. Ensure `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check` all pass
4. Include tests for any new logic
5. Open a pull request with a description of the change and motivation

## Coding Guidelines

- Keep the MSRV at Rust 1.92 — avoid features introduced after that version
- New compression behavior must not affect `Lossless` mode
- Each PR should include tests for any new logic in the relevant module (`ir`, `compressor`, `symbol`, `renderer`, `stream`)
- Run `cargo clippy -- -D warnings` and `cargo fmt` before submitting

## Reporting Issues

- **Bug reports**: Use the Bug Report issue template
- **Feature requests**: Use the Feature Request issue template
- **Security vulnerabilities**: See [SECURITY.md](SECURITY.md)

## Version Bumping

When preparing a release, update **all three** locations to the same version:

| File | Field | Example |
|------|-------|---------|
| `Cargo.toml` | `version = "x.y.z"` | `0.1.6` |
| `.claude-plugin/plugin.json` | `"version": "x.y.z"` | `0.1.6` |
| Git tag | `vx.y.z` | `v0.1.6` |

## License

By contributing, you agree that your contributions will be licensed under the [Apache-2.0 License](LICENSE).
