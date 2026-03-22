# API Benchmark

## 計測条件

- date: 2026-03-22
- machine: local dev machine
- note: 初期局面を固定し、同じ API を大量反復する単純ベンチ
- workload:
  - Rust は `cargo test --release ... -- --ignored --nocapture`
  - Python は `uv run python -m veloversi.bench_api`
  - Python object API と bits helper API を同条件で比較

## Rust

| api | command | iterations | elapsed | note |
| --- | --- | ---: | ---: | --- |
| `generate_legal_moves` | `make api-bench-rust-legal` | 5,000,000 | 0.037032s | `VELOVERSI_SIMD=auto`, movegen=`avx2`, flip=`avx2`, board=`sse2` |
| `apply_move_unchecked` | `make api-bench-rust-apply-unchecked` | 5,000,000 | 0.026693s | `VELOVERSI_SIMD=auto`, movegen=`avx2`, flip=`avx2`, board=`sse2` |
| `apply_move` | `make api-bench-rust-apply` | 5,000,000 | 0.034425s | `VELOVERSI_SIMD=auto`, movegen=`avx2`, flip=`avx2`, board=`sse2` |

## Python

| api | command | iterations | elapsed | note |
| --- | --- | ---: | ---: | --- |
| `generate_legal_moves` | `make api-bench-python` | 200,000 | 0.238938s | object API |
| `generate_legal_moves_bits` | `make api-bench-python` | 200,000 | 0.278029s | bits helper API |
| `apply_move_unchecked` | `make api-bench-python` | 200,000 | 0.322433s | object API |
| `apply_move_unchecked_bits` | `make api-bench-python` | 200,000 | 0.333708s | bits helper API |
| `apply_move` | `make api-bench-python` | 200,000 | 0.418224s | object API |
| `apply_move_bits` | `make api-bench-python` | 200,000 | 0.298363s | bits helper API |

## 補足

- Rust と Python で iteration 数が異なるため、絶対値の直接比較ではなく桁感と傾向の確認に使う
- bits helper API と object API の優劣は API と実行条件でぶれうるため、継続的に同じコマンドで比較する
- Python での既定利用形態は `Board` を使う object API とし、bits helper API は局面の分解済みデータを既に持っている呼び出し元向けの補助 API とする
