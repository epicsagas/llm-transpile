.PHONY: eval eval-report test-docs test bench lint fmt check

# `make eval`: structured JSON (composite score, BPE + heuristic token counts)
# consumed by `epic eval` (result_type: composite) and CI. Uses the real
# cl100k BPE tokenizer via the `tiktoken` feature — the heuristic alone is
# self-referential and inflates reduction on PUA-heavy output.
eval:
	cargo run --release --example eval --features tiktoken -- --json

# `make eval-report`: human-readable table + summary (REPL / manual review).
eval-report:
	cargo run --release --example eval --features tiktoken

test-docs:
	cargo run --example test_docs

test:
	cargo test

bench:
	cargo bench

lint:
	cargo clippy -- -D warnings

fmt:
	cargo fmt

check: lint test
