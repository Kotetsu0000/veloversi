# Step 18: `ref` AI 移植 Phase 2

## このステップの目的

このステップでは、Step 17 で導入した exact/endgame 側の最小核に続いて、`ref` の midgame 探索側を段階的に Rust へ寄せる。

方針は次のとおり。

- `ref` にあるものを段階的に寄せる
- `ref` に関係ない独自探索は作らない
- 1 step で検証しきれる範囲に留める
- Python API は後段に回し、まず Rust API を安定させる

## version 方針

- `ref` AI が完成するまでは `0.0.1` を維持する
- `ref` AI が完成した段階で `0.1.0` を出す

## このステップで行うこと

- `ref` の midgame 探索入口を確認する
- `Search` 相当の最小状態を Rust 側へ切り出す
- `mid_evaluate_diff` を動かす最小評価導線をつなぐ
- `nega_scout` の最小核を Rust に移植する
- `search_best_move` の Rust API を追加する
- `make check` が通る状態まで整える

## このステップの対象範囲

### 対象

- `ref` AI の midgame 探索の最小核
- `mid_evaluate_diff`
- `nega_scout`
- `search_best_move`
- Rust API

### 対象外

- `ref` に無い独自評価器
- `ref` に無い独自探索アルゴリズム
- move ordering の全面移植
- transposition table
- aspiration
- multi-thread
- clog
- time management
- Python API の本格公開
- build feature の有効化実装
- `ref` AI 入り install 導線

## 固定した前提

- `Search` 相当は midgame 探索に必要な最小状態だけ持つ
- 評価器は `mid_evaluate_diff` を動かす最小依存まで
- 探索本体は `nega_scout` の最小核まで
- move ordering は最小限に留める
- Rust API を先に作り、Python API は後続 step に回す
- `0.0.1` を維持する
- 1 step で検証しきれる範囲に留める

## 受け入れ条件

- [x] `ref` の midgame 探索入口ファイルが特定されている
- [x] `Search` 相当の最小状態が Rust に入っている
- [x] `mid_evaluate_diff` を動かす最小評価導線がある
- [x] `nega_scout` の最小核が Rust に入っている
- [x] `search_best_move` の Rust API がある
- [x] `ref` に無い独自探索を追加していない
- [x] `make check` が成功する

## 実装方針

- `ref` の探索入口と評価入口だけを読む
- `Search` 全体は持ち込まず、必要な状態だけ切る
- ordering や TT は最小化し、後段へ回す
- `search_best_move` は Rust API のみ先行する

## 懸念点

- `mid_evaluate_diff` の依存量
  - `ref` の評価は pattern table 群まで繋がっている
  - 最小依存の切り方を誤ると Step 18 の範囲を超えて広がる
  - このため Step 18 では「最小評価導線まで」に厳密に留める

- `Search` 相当の境界
  - 小さく切りすぎると `nega_scout` 実装時に歪む
  - 広く持ちすぎると `ref` の巨大構造を引き込む
  - このため midgame 探索に必要な状態だけを切り出す

- move ordering をどこで止めるか
  - `ref` の midgame 探索は ordering と結びついている
  - ここで欲張ると Step 18 が膨らむ
  - このため ordering は最小限に留め、全面移植は後段へ回す

## `ref` 参照の起点

- 探索入口:
  - `ref/Egaroucid/src/engine/midsearch.hpp`
  - `ref/Egaroucid/src/engine/minimax.hpp`
- 評価入口:
  - `ref/Egaroucid/src/engine/evaluate.hpp`
  - `ref/Egaroucid/src/engine/evaluate_generic.hpp`
  - `ref/Egaroucid/src/engine/evaluate_common.hpp`

## このステップを先に行う理由

Step 17 で exact/endgame 側の最小核は入ったため、次は midgame 探索をつないで `ref` AI の通常推論へ進める段階にある。
ここで `mid_evaluate_diff` と `nega_scout` の最小核を先に固めることで、その後の ordering や TT の段階的移植がやりやすくなる。

## 実装結果

- `src/search.rs` を復旧し、Step 17 の exact solver を維持したまま midgame 探索を追加した
- `src/search_eval_data.rs` を追加し、`ref/Egaroucid/src/engine/evaluate_generic.hpp` の
  `feature_to_coord` を Veloversi の square 番号へ変換した定数として保持した
- `ref/Egaroucid/bin/resources/eval.egev2` は `include_bytes!` で埋め込み、
  `OnceLock` で一度だけ展開して評価テーブルを初期化する構成にした
- Rust 公開 API として次を追加した
  - `SearchConfig`
  - `SearchResult`
  - `ScoreKind`
  - `search_best_move`
- `search_best_move` は次の最小核で構成した
  - `exact_solver_empty_threshold` による exact solver への移行
  - `mid_evaluate_diff`
  - `nega_scout` の最小核
  - root での最小 PVS
- `time_limit_ms`、`use_transposition_table`、`multi_pv` は Step 18 ではまだ `ref` 寄せしていない

## 検証結果

- `cargo test`: 成功
  - `99 passed; 0 failed; 6 ignored`
- `make check`: 成功
- `make coverage-check`: 成功
  - line coverage `87.45%`
