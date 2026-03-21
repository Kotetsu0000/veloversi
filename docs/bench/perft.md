# Perft Benchmark

## 計測条件

- date: 2026-03-22
- machine: local dev machine
- command: `cargo test --release perft_long_initial_position_mode_one_to_depth_fifteen -- --ignored --nocapture`
- build: release
- note: 深さ 12 / 13 到達時点の途中観測値。初回ビルド時間を含む。

## Generic

| mode | depth | elapsed | note |
| --- | ---: | ---: | --- |
| 1 | 12 | 13.78s | table-based flip + 共通基盤化後、build込み |
| 1 | 13 | 33.70s | table-based flip + 共通基盤化後、build込み |
| 2 | 15 |  |  |

## SIMD

| mode | depth | elapsed | note |
| --- | ---: | ---: | --- |
| 1 | 15 |  |  |
| 2 | 15 |  |  |

## 補足

- generic / SIMD のどちらか一方しかない段階では、未実装側は空欄のままでよい
- 深さ 13 や 14 の途中観測値が必要ならこの節に追記する
