.PHONY: build-ext test lint format check mutants

build-ext:
	uv run maturin develop

test: build-ext
	cargo test
	uv run pytest || [ $$? -eq 5 ]

lint:
	cargo clippy --all-targets --all-features -- -D warnings
	uv run ruff check .
	uv run basedpyright

format:
	cargo fmt --check
	uv run ruff format --check .

check: format lint test

mutants:
	cargo mutants --file src/lib.rs
