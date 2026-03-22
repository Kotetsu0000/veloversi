.PHONY: build-ext test lint format check mutants coverage coverage-check perft-long perft-bench-auto perft-bench-generic perft-bench-sse2 perft-bench-avx2 api-bench-rust-legal api-bench-rust-apply-unchecked api-bench-rust-apply api-bench-python

PYTHON_PATHS := src $(wildcard tests)

build-ext:
	uv run maturin develop

test: build-ext
	cargo test
	uv run pytest || [ $$? -eq 5 ]

lint:
	cargo clippy --all-targets --all-features -- -D warnings
	uv run ruff check $(PYTHON_PATHS)
	uv run basedpyright

format:
	cargo fmt --check
	uv run ruff format --check $(PYTHON_PATHS)

check: format lint test

mutants:
	cargo mutants --file src/lib.rs

coverage:
	cargo llvm-cov --html

coverage-check:
	cargo llvm-cov --fail-under-lines 85

perft-long:
	cargo test --release perft_long_initial_position_mode_one_to_depth_fifteen -- --ignored --nocapture
	cargo test --release perft_long_initial_position_mode_two_to_depth_fifteen -- --ignored --nocapture

perft-bench-auto:
	VELOVERSI_SIMD=auto cargo test --release perft_bench_initial_position_mode_one_to_depth_thirteen -- --ignored --nocapture

perft-bench-generic:
	VELOVERSI_SIMD=generic cargo test --release perft_bench_initial_position_mode_one_to_depth_thirteen -- --ignored --nocapture

perft-bench-sse2:
	VELOVERSI_SIMD=sse2 cargo test --release perft_bench_initial_position_mode_one_to_depth_thirteen -- --ignored --nocapture

perft-bench-avx2:
	VELOVERSI_SIMD=avx2 cargo test --release perft_bench_initial_position_mode_one_to_depth_thirteen -- --ignored --nocapture

api-bench-rust-legal:
	cargo test --release api_bench_generate_legal_moves_initial_position -- --ignored --nocapture

api-bench-rust-apply-unchecked:
	cargo test --release api_bench_apply_move_unchecked_initial_position -- --ignored --nocapture

api-bench-rust-apply:
	cargo test --release api_bench_apply_move_initial_position -- --ignored --nocapture

api-bench-python: build-ext
	uv run python -m veloversi.bench_api
