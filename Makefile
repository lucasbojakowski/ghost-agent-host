.PHONY: check test lint fmt validate

check:
	cargo check --workspace --all-features

test:
	cargo test --workspace --all-features

lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

fmt:
	cargo fmt --all -- --check

validate: fmt check test lint
