# Step 34: Exact Solver Final Optimization Closure

## このステップの目的

Step 32 と Step 33 で exact solver の single-thread 最適化と root 並列化は入った。
ただし、まだ次の「追加で詰められる余地」が残っている。

- thread 数や並列化ポリシーを公開設定できない
- benchmark が test helper に散っていて、性能判断の基準が固定されていない
- shared TT / root parallel の閾値が実測ベースではあるが、公開 API から制御できない
- exact solver の速度改善を「これ以上は費用対効果が低い」と言えるだけの記録がない

このステップでは、exact solver の高速化で残っている実用上の余地をまとめて潰し、
Step 完了時点で「exact solver の高速化は一旦閉じる」と判断できる状態にする。

## version 方針

- このステップ中は `0.2.2` を維持する
- このステップ完了時に `0.2.3` を切る前提で進める

## このステップで行うこと

### Phase 1: 並列設定の公開

- `search_best_move_exact` の並列度と並列化ポリシーを制御できる設定を追加する
- 少なくとも次を設定可能にする
  - worker 数
  - serial fallback を使う `empty` 閾値
  - shared TT を使う `empty` 閾値

### Phase 2: benchmark の整備

- exact solver 専用 benchmark を `tests` の ignored helper から整理する
- 固定終盤局面の benchmark セットを作る
- 比較対象を明確にする
  - serial exact
  - public exact API
  - 並列設定差分

### Phase 3: 最終チューニング

- 並列閾値
- worker 数
- shared TT 適用帯域
- deadline 判定間隔
を benchmark ベースで再調整する

### Phase 4: 終了条件の確定

- benchmark 結果を記録する
- 追加最適化候補を洗い出す
- それでも残るものを
  - 費用対効果が低い
  - exact solver ではなく midgame 側の課題
  - API / benchmark 整備の課題
に切り分ける

## このステップの対象範囲

### 対象

- `src/search.rs`
- `src/python.rs`
- `src/veloversi/__init__.py`
- `src/veloversi/__init__.pyi`
- `src/veloversi/_core.pyi`
- `README.md`
- `docs/step/step-34.md`
- `docs/step/todo.md`

### 対象外

- midgame 探索の高速化
- 独自評価関数の改善
- Python 通常探索 API の公開

## 固定した前提

- 改善対象は引き続き `search_best_move_exact`
- correctness と timeout 仕様は維持する
- timeout 超過時は partial result を返さず失敗を返す
- 設定追加は exact API のみに限定する
- benchmark は固定終盤局面で比較可能にする
- Step 34 完了時には、exact solver の高速化は block ではなく保守対象へ移す

## 受け入れ条件

- [x] exact solver の worker 数と並列化閾値を公開設定できる
- [x] exact solver 専用 benchmark の実行導線が整理されている
- [x] benchmark 結果に基づいて既定値が調整されている
- [x] Step 33 時点と比べて実用上の悪化がない
- [x] `make check` が成功する
- [x] 高速化の残件が「費用対効果が低いもの」だけに整理されている

## 懸念と解決策

### 公開設定を増やしすぎる

- 懸念:
  - thread 数や閾値を細かく出しすぎると API が重くなる
- 解決策:
  - Step 34 では exact solver に必要な最小設定だけを公開する
  - shared TT / serial fallback の 2 閾値と worker 数に絞る

### benchmark が再現しにくい

- 懸念:
  - 盤面が固定されていないと最適化判断がぶれる
- 解決策:
  - 固定終盤局面を benchmark セットとして保持する
  - serial / public exact / 設定差分を同じ盤面で比較する

### チューニングが環境依存になる

- 懸念:
  - CPU コア数やスケジューラで最適 worker 数が変わる
- 解決策:
  - 既定値は `available_parallelism()` ベースにする
  - benchmark 記録では実行環境を明記する
  - API で override できるようにする

### timeout と worker 数の相互作用

- 懸念:
  - worker 数を増やすと、timeout 直前の停止コストや join 待ちが増えて、短い制限時間で不安定になりやすい
- 解決策:
  - benchmark は timeout なしだけでなく、短い timeout 条件でも確認する
  - worker 数の既定値は最速だけでなく timeout 安全性も含めて決める

### Rust API と Python API の設定差

- 懸念:
  - Rust 側だけ exact 設定を増やすと、Python からは既定値しか触れず公開 API として不整合になる
- 解決策:
  - Step 34 で追加する exact 設定は Rust / Python の両方で使えるようにする
  - 名前と意味を揃え、README と docstring に同じ規約で記載する

### benchmark の保守性

- 懸念:
  - 固定局面 benchmark が雑に増えると、将来どの局面を比較しているのか追えなくなる
- 解決策:
  - benchmark 用局面の由来と empty 数をコメントで固定する
  - 少なくとも 16 空き、18 空き、20 空きの 3 系統を比較対象にする

### 「余地がなくなる」の定義が曖昧

- 懸念:
  - どこまでやれば終了かがぶれる
- 解決策:
  - Step 34 では
    - 公開設定
    - benchmark
    - 既定値調整
    - 残件分類
  の 4 点を揃えた時点で閉じる
  - 以後の残件は exact solver の block ではなく保守対象へ送る

## 実装結果

- Rust 公開 API に `ExactSearchConfig` と `search_best_move_exact_with_config` を追加した
- Python 公開 API は既存の `search_best_move_exact` / `board.search_best_move_exact` / `record.search_best_move_exact` に optional 設定を追加した
  - `worker_count`
  - `serial_fallback_empty_threshold`
  - `shared_tt_empty_threshold`
- exact solver benchmark は固定ケースを `src/search.rs` の ignored benchmark helper に整理した
  - `empty16`
  - `empty18`
  - `empty20`
- 既定値は Step 33 のまま維持した
  - `serial_fallback_empty_threshold = 18`
  - `shared_tt_empty_threshold = 20`
  - `worker_count = None` のとき `available_parallelism()` ベース

## benchmark 結果

`cargo test --release exact_search_bench_step34_configurations -- --ignored --nocapture`

- empty16
  - serial: `581.095244ms`
  - default: `576.338200ms`
  - worker1: `589.940343ms`
- empty18
  - serial: `4.387435127s`
  - default: `4.235315275s`
  - worker1: `4.409580277s`
- empty20
  - serial: `7.332772162s`
  - default: `6.067557879s`
  - worker1: `7.136760407s`

判断:

- 既定値は 16 / 18 / 20 空きの benchmark で single-worker より速い
- 18 空きと 20 空きでは serial exact よりも速い
- この時点で、exact solver の高速化で残る論点は block ではなく保守対象へ移せる

Step 33 比較の再計測:

`cargo test --release exact_search_bench_step32_vs_step33_ -- --ignored --nocapture`

- 18 空き:
  - Step 32: `4.146492493s`
  - current: `4.224412764s`
  - 差分: 約 `+1.9%`
- 20 空き:
  - Step 32: `6.165042131s`
  - current: `6.084131330s`
  - 差分: 約 `-1.3%`

判断:

- 18 空きでは依然として Step 32 より少し遅いが、18 空きは shared TT を使わない帯域なので Step 33 で選んだ保守的分岐を維持する
- 20 空きでは Step 32 より速い状態を維持している
- Step 34 の追加内容は設定公開と benchmark 整備が主であり、実用上の性能は維持できている

## 検証結果

- `cargo test search::tests -- --nocapture`: 成功
- `uv run pytest -q src/test_python_api.py -k "search_best_move_exact"`: 成功
- `cargo test --release exact_search_bench_step34_configurations -- --ignored --nocapture`: 成功
- `make check`: 成功
