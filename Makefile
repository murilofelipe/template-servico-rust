.PHONY: format format-check lint test coverage check

format:
	cargo fmt --all

format-check:
	cargo fmt --all -- --check

lint:
	cargo clippy --all-targets --all-features -- -D warnings

test:
	cargo test --all-targets --all-features

coverage:
	cargo test --all-targets --all-features

check: format-check lint test
