.PHONY: build-ext test lint format check mutants coverage coverage-check

build-ext:
	uv run maturin develop

test: build-ext
	cargo test
	uv run pytest || [ $$? -eq 5 ]

lint:
	cargo clippy --all-targets --all-features -- -D warnings
	uv run ruff check src tests
	uv run basedpyright

format:
	cargo fmt --check
	uv run ruff format --check src tests

check: format lint test

mutants:
	cargo mutants --file src/lib.rs

coverage:
	cargo llvm-cov --html

coverage-check:
	cargo llvm-cov --fail-under-lines 85
