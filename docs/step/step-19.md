# Step 19: `ref` AI 移植 Phase 3

## このステップの目的

このステップでは、Step 18 で導入した `mid_evaluate_diff` と `nega_scout` の最小核に続いて、
`ref` の move ordering と transposition table の最小核を Rust 側へ寄せる。

方針は次のとおり。

- `ref` にあるものを段階的に寄せる
- `ref` に関係ない独自探索は作らない
- 1 step で検証しきれる範囲に留める
- Python API は後段に回し、まず Rust API を安定させる

## version 方針

- `ref` AI が完成するまでは `0.0.1` を維持する
- `ref` AI が完成した段階で `0.1.0` を出す

## このステップで行うこと

- `ref` の move ordering 入口を確認する
- `ref` の transposition table 入口を確認する
- TT move を使う最小 ordering を Rust に移植する
- transposition table の最小核を Rust に移植する
- `SearchConfig.use_transposition_table` を実装に接続する
- `search_best_move` の探索品質と速度を `ref` 側へ寄せる
- `make check` が通る状態まで整える

## このステップの対象範囲

### 対象

- `ref` AI の move ordering の最小核
- `ref` AI の transposition table の最小核
- `SearchConfig.use_transposition_table`
- `search_best_move`
- Rust API

### 対象外

- `ref` に無い独自 ordering
- `ref` に無い独自 TT
- aspiration
- multi-thread
- clog
- time management
- Python API の本格公開
- build feature の有効化実装
- `ref` AI 入り install 導線
- `time_limit_ms`
- `multi_pv`
- killer/history/counter move の全面移植

## 固定した前提

- move ordering は最小限に留める
  - TT move
  - 即勝ち手
  - 既存自然順
- transposition table は最小実装に留める
  - hash
  - depth
  - bound kind
  - score
  - best move
- score 視点は `MarginFromSideToMove` で統一する
- Step 19 で実装接続する `SearchConfig` は次に限定する
  - `max_depth`
  - `max_nodes`
  - `exact_solver_empty_threshold`
  - `use_transposition_table`
- `time_limit_ms` と `multi_pv` は未実装のまま維持する
- `Search` 状態は TT と最小 ordering に必要なぶんだけ広げる
- Python API はこの step では扱わない

## 受け入れ条件

- [x] `ref` の move ordering 入口ファイルが特定されている
- [x] `ref` の transposition table 入口ファイルが特定されている
- [x] TT move を使う最小 ordering が Rust に入っている
- [x] transposition table の最小核が Rust に入っている
- [x] `use_transposition_table` が実装に接続されている
- [x] `search_best_move` の探索品質が Step 18 より改善している
- [x] `ref` に無い独自探索を追加していない
- [x] `make check` が成功する

## 実装方針

- `ref` の ordering と TT の入口だけを読む
- 全面移植ではなく、探索品質に寄与する最小核から入れる
- killer/history/counter move はまだ持ち込まない
- API は Rust のまま維持し、まず探索内部の品質を上げる
- Step 18 と同様に、検証しきれる範囲で止める

## 懸念点

- move ordering の広がり
  - `ref` の ordering は探索周辺の複数要素と結びついている
  - 欲張ると Step 19 が膨らむ
  - このため Step 19 では TT move と最小自然順までに留める

- transposition table の境界
  - 置換戦略や補助情報まで広げると一気に複雑化する
  - このため最小 entry に必要な項目だけ実装する

- `Search` 状態の肥大化
  - ordering と TT を同時に入れると `Search` が重くなりやすい
  - このため killer/history/counter move はまだ入れない

- TT のキーと score 格納規約
  - ここが曖昧だと、保存時と取得時で探索結果が壊れる
  - このため Step 19 では `MarginFromSideToMove` 視点で統一し、
    TT 内部でも同じ視点だけを扱う

- TT 導入後の検証難度
  - TT を入れると探索結果の変化が見えにくくなる
  - このため `use_transposition_table=false` と `true` で
    最善手、score、exact threshold 経路の一致を直接テストする

## `ref` 参照の起点

- 探索入口:
  - `ref/Egaroucid/src/engine/midsearch.hpp`
- ordering 入口:
  - `ref/Egaroucid/src/engine/move_ordering.hpp`
- TT 入口:
  - `ref/Egaroucid/src/engine/transposition_table.hpp`
  - `ref/Egaroucid/src/engine/transposition_table.cpp`
  - 必要なら `ref/Egaroucid/src/engine/search.hpp`

## このステップを先に行う理由

Step 18 で midgame 探索の最小核は入ったため、次に探索品質へ効くのは move ordering と transposition table である。
ここで `ref` の最小核を先に寄せることで、その後の aspiration、時間管理、公開 API 拡張へ進みやすくなる。

## 実装結果

- [src/search.rs](/home/kotetsu0000/program/veloversi/src/search.rs) に最小 transposition table を追加した
  - key
    - `black_bits`
    - `white_bits`
    - `side_to_move`
  - value
    - `depth`
    - `bound kind`
    - `score`
    - `best move`
    - `is_exact`
- `use_transposition_table` を `SearchConfig` から実装へ接続した
- `nega_scout` に次を追加した
  - TT lookup
  - bound cutoff
  - TT store
  - TT move ordering
- move ordering は Step 19 の範囲に合わせて最小に留めた
  - TT move
  - 即勝ち手
  - 既存自然順
- killer/history/counter move はまだ持ち込んでいない

## 検証結果

- `cargo test search::tests -- --nocapture`: 成功
  - `12 passed; 0 failed`
- `make check`: 成功
- `make coverage-check`: 成功
  - line coverage `87.70%`

## 追加した確認

- `use_transposition_table=false` と `true` で
  - `best_move`
  - `best_score`
  - `score_kind`
  - exact threshold 経路
  が一致することをテストした
- depth 4 の初期局面で、TT 有効時の `searched_nodes` が無効時を上回らないことを確認した
