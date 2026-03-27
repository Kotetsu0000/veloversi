# Step 17: `ref` AI 移植 Phase 1

## このステップの目的

このステップでは、`ref` にある AI 実装を段階的に Rust 側へ寄せていく最初の段階として、終盤完全読みの最小核を移植する。

方針は次のとおり。

- `ref` にあるものを段階的に寄せる
- `ref` に関係ない独自探索は作らない
- 依存の少ない中核から移す
- Python API は後段に回し、まず Rust API を安定させる

## version 方針

- `ref` AI が完成するまでは `0.0.1` を維持する
- `ref` AI が完成した段階で `0.1.0` を出す

## このステップで行うこと

- `ref` AI の入口となる探索実装を確認する
- 依存関係を最小単位で分解する
- exact/endgame 探索の最小核を Rust に移植する
- `solve_exact` と `can_solve_exact` の Rust API を追加する
- `best_move` と exact `score` を返す Rust API を整える
- `ref` の endgame 側に対応する最小評価導線をつなぐ
- `make check` が通る状態まで整える

## このステップの対象範囲

### 対象

- `ref` AI の endgame/exact 探索の最小核
- `solve_exact`
- `can_solve_exact`
- `best_move`
- exact `score`
- Rust API

### 対象外

- 独自評価器
- 独自探索アルゴリズム
- Python API の本格公開
- build feature の有効化実装
- PV の充実
- 時間管理
- `ref` AI 入り install 導線
- midgame 探索
- `search_best_move`

## 固定した前提

- 評価器は `ref` の endgame 側にある最小依存だけを使う
- 独自の簡易評価器は作らない
- Python API は同 step 内でも後段。まず Rust API を先に作る
- build feature は設計だけ意識し、この step では実装しない
- `ref` の探索入口と評価入口を起点に依存を辿る
- midgame 評価テーブル一式はこの step では持ち込まない

## 受け入れ条件

- [x] `ref` AI 移植の入口ファイルが特定されている
- [x] exact/endgame 探索の最小核が Rust に入っている
- [x] `solve_exact` がある
- [x] `can_solve_exact` がある
- [x] `best_move` を返す Rust API がある
- [x] exact `score` を返す Rust API がある
- [x] `ref` に無い独自探索を追加していない
- [x] `make check` が成功する

## 実装方針

- まず `ref` の探索入口と評価入口だけを読む
- 枝葉の utility を先に広く移さない
- 1 回で全移植せず、exact/endgame 側の最小で動く単位を優先する
- `ref` の構造と責務を崩さない
- 比較しやすいよう、API も役割単位で最小に留める

## `ref` 参照の起点

- 探索入口:
  - `ref/Egaroucid/src/engine/minimax.hpp`
  - `ref/Egaroucid/src/engine/endsearch_nws.hpp`
- 評価入口:
  - `ref/Egaroucid/src/engine/evaluate_common.hpp`

## このステップを先に行う理由

基盤機能、feature、random_play、release 導線まで揃ったため、次はこのライブラリのもう一つの主目的である `ref` AI 再現に入る段階にある。
ここで endgame/exact の最小核から着手することで、以後の段階的移植をぶらさず進めやすくする。

## 実装結果

- `src/search.rs` を追加した
- Rust 公開 API として次を追加した
  - `SolveConfig`
  - `SolveResult`
  - `SolveError`
  - `can_solve_exact`
  - `solve_exact`
- `solve_exact` は `ref` の `minimax.hpp` / `endsearch_nws.hpp` / `evaluate_common.hpp` を起点に、endgame/exact 側の最小核だけを Rust へ寄せた
- midgame 評価テーブルや Python 公開はこの step では持ち込んでいない

## 検証結果

- `cargo test`: 成功
  - `95 passed; 0 failed; 6 ignored`
- `make check`: 成功
- `make coverage-check`: 成功
  - line coverage `88.87%`
