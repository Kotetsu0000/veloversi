# Perft Benchmark

## 計測条件

- date: 2026-03-22
- machine: local dev machine
- command: `VELOVERSI_SIMD=<backend> cargo test --release perft_bench_initial_position_mode_one_to_depth_thirteen -- --ignored --nocapture`
- build: release
- note: 深さ 13 までのコマンド全体時間。単独実行で再計測した値。

## Generic

| mode | depth | elapsed | note |
| --- | ---: | ---: | --- |
| 1 | 13 | 20.38s | `VELOVERSI_SIMD=generic`、movegen=generic / flip=generic / board=generic |

## SIMD

| mode | depth | elapsed | note |
| --- | ---: | ---: | --- |
| 1 | 13 | 20.84s | `VELOVERSI_SIMD=sse2`、movegen=generic / flip=generic / board=sse2 |
| 1 | 13 | 15.28s | `VELOVERSI_SIMD=avx2`、movegen=avx2 / flip=avx2 / board=sse2 |

## 補足

- `perft_bench_initial_position_mode_one_to_depth_thirteen` は深さ 13 到達までをまとめて確認するため、`time` の実測値はコマンド全体の経過時間を表す
- `generic -> sse2` の差は小さく、現状の改善の主因は AVX2 経路
- 今回の単独再計測では `avx2` は `generic` 比で約 25% 短縮
