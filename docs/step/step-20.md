# Step 20: `ref` AI 移植 Phase 4

## このステップの目的

このステップでは、Step 19 までで導入した midgame 探索の最小核、move ordering、transposition table に続いて、
`SearchConfig` に残っている `time_limit_ms` と `multi_pv` を最小構成で実装へ接続する。

方針は次のとおり。

- `ref` にあるものを段階的に寄せる
- `ref` に関係ない独自探索は作らない
- 1 step で検証しきれる範囲に留める
- Python API は後段に回し、まず Rust API を安定させる

## version 方針

- `ref` AI が完成するまでは `0.0.1` を維持する
- `ref` AI が完成した段階で `0.1.0` を出す

## このステップで行うこと

- `ref` の時間管理入口を確認する
- `time_limit_ms` を最小構成で探索停止条件へ接続する
- root 限定の `multi_pv` を最小構成で導入する
- PV 返却を安定化する
- `search_best_move` の Rust API を維持したまま内部実装を更新する
- `make check` が通る状態まで整える

## このステップの対象範囲

### 対象

- `time_limit_ms` の最小実装
- root 限定の `multi_pv` 最小実装
- `pv` の返却安定化
- `search_best_move`
- Rust API

### 対象外

- `ref` に無い独自時間管理
- iterative deepening の全面導入
- aspiration
- multi-thread
- clog
- Python API の本格公開
- build feature の有効化実装
- `ref` AI 入り install 導線
- `multi_pv` の詳細結果型追加

## 固定した前提

- `time_limit_ms` は Step 20 では最小実装に留める
  - 探索開始時刻を持つ
  - 節目ごとに打ち切る
- `multi_pv` は root 限定の最小実装とする
- `SearchResult` の型は Step 20 では増やさない
- 打ち切り時は error を返さず、その時点の最良結果を返す
- `reached_depth` と `searched_nodes` で探索状態を表す
- Python API はこの step では扱わない

## 受け入れ条件

- [x] `ref` の時間管理入口ファイルが特定されている
- [x] `time_limit_ms` が探索停止条件として接続されている
- [x] root 限定の `multi_pv` 最小実装が入っている
- [x] `pv` が安定して返る
- [x] `SearchResult` の互換を壊していない
- [x] `ref` に無い独自探索を追加していない
- [x] `make check` が成功する

## 実装方針

- `ref` の時間管理入口だけをまず読む
- Step 20 では full time management に広げない
- `multi_pv` は root でのみ扱う
- `SearchResult` の型を増やさずに内部実装だけを更新する
- Rust API を先に安定させる

## 懸念点

- 時間打ち切りの粒度
  - 細かく見すぎると探索が重くなる
  - 粗すぎると `time_limit_ms` の意味が薄くなる
  - このため Step 20 では root / 再帰の節目で確認する最小実装に留める

- `multi_pv` の広がり
  - 内部ノードまで多経路管理を入れると一気に複雑化する
  - このため root 限定で止める

- 結果型の拡張誘惑
  - `multi_pv` を入れると返り値を増やしたくなる
  - このため Step 20 では `SearchResult` を変えず、まず best line の安定化を優先する

- 打ち切り時の返り値規約
  - 途中停止を error にするか、その時点の最良結果にするかを先に固定しないと API がぶれる
  - このため Step 20 では error を増やさず、その時点の最良結果を返す
  - 探索状態は `reached_depth` と `searched_nodes` で表す

- exact solver と時間制限の関係
  - exact solver に入ったあとまで時間制限を強くかけると仕様がぶれる
  - このため Step 20 では exact solver に入った後は最後まで走らせる
  - `time_limit_ms` は通常探索に対して効かせる

- 検証観点
  - `time_limit_ms` と `multi_pv` は結果が壊れていないことの確認が重要
  - このため次を直接確認する
    - `time_limit_ms=None` と短い制限ありで結果型が壊れない
    - `searched_nodes` が減る
    - `reached_depth` が不正にならない
    - `multi_pv=1` と `multi_pv>1` で `best_move` / `best_score` / `pv` が壊れない

## `ref` 参照の起点

- 探索入口:
  - `ref/Egaroucid/src/engine/ai.hpp`
- midgame 探索:
  - `ref/Egaroucid/src/engine/midsearch.hpp`
- 必要なら:
  - `ref/Egaroucid/src/engine/search.hpp`

## このステップを先に行う理由

Step 19 で `SearchConfig` の `use_transposition_table` までは実装へ接続できたため、
次は残っている `time_limit_ms` と `multi_pv` を最小構成で埋める段階にある。
ここを片付けることで、Rust 側の `search_best_move` は `ref` AI 公開前の土台としてかなり揃う。

## 実装結果

- `time_limit_ms` を通常探索の停止条件へ接続した
  - exact solver へ入った後は打ち切らない
  - 通常探索では root と再帰の節目で停止判定を行う
  - 打ち切り時は error を返さず、その時点の最良結果を返す
- root 限定の `multi_pv` を最小構成で導入した
  - root 候補を複数保持する
  - 外部 API の `SearchResult` は変更していない
  - best line の安定化にだけ使う
- `pv` と `best_move` の更新規約を整理し、途中停止時でも結果形が崩れないようにした

## 検証結果

- `make check`: 成功
  - Rust: `104 passed; 0 failed; 6 ignored`
  - Python: `28 passed`
- `cargo test search::tests -- --nocapture`: 成功
  - `time_limit_ms` あり/なしで結果形が壊れないことを確認
  - exact solver 経路は `time_limit_ms` で打ち切られないことを確認
  - `multi_pv=1` と `multi_pv>1` で best line が壊れないことを確認
