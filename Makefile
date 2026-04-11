.PHONY: eval test bench lint fmt check

eval:
	cargo run --example eval

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
