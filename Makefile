.PHONY: format format-check lint test audit deny sast coverage check verify-logs verify-sast db-create db-drop db-migrate db-prepare db-reset

DATABASE_URL ?= postgres://postgres:postgres@localhost:5432/template_db

format:
	cargo fmt --all

format-check:
	cargo fmt --all -- --check

lint:
	cargo clippy --all-targets --all-features -- -D warnings

test:
	cargo test --all-targets --all-features

audit:
	cargo audit --ignore RUSTSEC-2023-0071

deny:
	cargo deny check

sast: audit deny

coverage:
	cargo tarpaulin --ignore-tests --out Xml --out Html

check: format-check lint sast test

verify-logs:
	bash scripts/verify-logging.sh

verify-sast:
	bash scripts/verify-sast.sh

db-create:
	sqlx database create --database-url $(DATABASE_URL)

db-drop:
	sqlx database drop --database-url $(DATABASE_URL)

db-migrate:
	sqlx migrate run --database-url $(DATABASE_URL)

db-prepare:
	cargo sqlx prepare -- --all-targets

db-reset: db-drop db-create db-migrate
