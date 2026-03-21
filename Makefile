.PHONY: build-ext test lint format check mutants coverage coverage-check perft-long perft-bench-auto perft-bench-generic perft-bench-sse2 perft-bench-avx2

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
