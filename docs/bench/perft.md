# Perft Benchmark

## 計測条件

- date: 2026-03-22
- machine: local dev machine
- command: `VELOVERSI_SIMD=<backend> cargo test --release perft_bench_initial_position_mode_one_to_depth_thirteen -- --ignored --nocapture`
- build: release
- note: 深さ 12 / 13 到達時点の途中観測値。初回ビルド時間を含む。

## Generic

| mode | depth | elapsed | note |
| --- | ---: | ---: | --- |
| 1 | 12 | 13.78s | table-based flip + 共通基盤化後、build込みの参考値 |
| 1 | 13 | 33.70s | table-based flip + 共通基盤化後、build込みの参考値 |
| 1 | 12 | 44.98s | `VELOVERSI_SIMD=generic`、movegen=generic / board=generic |
| 1 | 13 | 44.98s | `VELOVERSI_SIMD=generic`、movegen=generic / board=generic |

## SIMD

| mode | depth | elapsed | note |
| --- | ---: | ---: | --- |
| 1 | 12 | 44.48s | `VELOVERSI_SIMD=sse2`、movegen=generic / board=sse2 |
| 1 | 13 | 44.48s | `VELOVERSI_SIMD=sse2`、movegen=generic / board=sse2 |
| 1 | 12 |  | `VELOVERSI_SIMD=avx2` はこのマシンでは未計測 (`avx2` 非対応) |
| 1 | 13 |  | `VELOVERSI_SIMD=avx2` はこのマシンでは未計測 (`avx2` 非対応) |

## 補足

- `perft_bench_initial_position_mode_one_to_depth_thirteen` は深さ 12 と 13 を連続確認するため、`time` の実測値はコマンド全体の経過時間を表す
- 手元環境は `avx2` 非対応なので、AVX2 経路の実速度確認は対応 CPU が必要
