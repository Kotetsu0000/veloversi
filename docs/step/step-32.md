# Step 32: Exact Solver Performance Upgrade

## このステップの目的

`search_best_move_exact` / `solve_exact` の実用速度を上げる。

このステップでは、段階的に実装しつつも、最終的に並列化まで含めて exact solver の主要な高速化要素を一通り入れることを目的にする。

主目的は次の 6 つ。

- exact solver 専用 transposition table を導入する
- move ordering を exact solver 向けに強化する
- 既存 alpha-beta を活かして無駄探索をさらに減らす
- timeout と correctness を壊さずに探索速度を上げる
- root 並列化を導入する
- Python 公開 API はそのままに、内部実装だけを高速化する

## version 方針

- このステップ中は `0.2.2` を維持する
- このステップ完了後に `0.2.3` として切るかを判断する

## このステップで行うこと

### Phase 1: exact solver の基盤高速化

- exact solver 専用の transposition table を追加する
- exact solver の node key と score 規約を固定する
- `solve_exact` と `search_best_move_exact` の両方で再利用できるようにする

### Phase 2: move ordering 強化

- exact solver の root / internal node に ordering を入れる
- 優先度は少なくとも次を持つ
  - TT move
  - 即終局 / 即最大利得手
  - corner 優先
  - 自然順

### Phase 3: 探索削減の追加

- exact solver 向けに入れられる pruning / early cutoff を追加する
- timeout 判定位置を見直し、速度と停止性のバランスを取る

### Phase 4: 並列化

- root 並列化を追加する
- 少なくとも root move 単位での並列探索を行えるようにする
- timeout と結果整合性を壊さないようにする

### Phase 5: 検証

- 小さな終盤局面で `solve_exact` と `search_best_move_exact` の結果が変わらないことを確認する
- timeout 挙動が壊れていないことを確認する
- `make check` を通す

## このステップの対象範囲

### 対象

- `src/search.rs`
- Python 公開 docstring / README の必要最小限更新
- exact solver の Rust / Python テスト

### 対象外

- midgame `search_best_move` の高速化
- `ref` 由来の評価器再導入
- Python 側の typed object 化
- release workflow の変更

## 固定した前提

- 対象は exact solver であり、midgame heuristic search は対象外
- correctness を優先する
- timeout 超過時は引き続き partial result を返さず失敗を返す
- Python の公開 API 名と戻り値 schema は変えない
- 並列化は root 並列化を基本にする
- 内部ノードの細粒度並列化は、このステップではやらない

## 受け入れ条件

- [x] exact solver 専用 transposition table が入っている
- [x] exact solver の move ordering が Step 31 より強化されている
- [x] `search_best_move_exact` が Step 31 より少ない探索ノード、または短い実時間で終わる局面が確認できる
- [x] root 並列化が入っている
- [x] timeout 時の失敗結果仕様が維持されている
- [x] `solve_exact` / `search_best_move_exact` の正しさが既存テストで維持されている
- [x] `make check` が成功する

## 実装方針

- まず single-thread の高速化から入る
  - TT
  - ordering
  - cutoff
- その後に root 並列化を足す
- exact solver の score 規約は現在と同じく「現在手番視点の石差」で統一する
- timeout 管理は共有 deadline を基準にする
- 並列化後も公開 API は変えない

## 懸念点

- exact solver 用 TT の score / bound 規約を誤ると correctness が壊れる
  - score 視点と保存・取得条件を先に固定する必要がある
- 並列化で timeout をまたぐと、停止と join が複雑になる
  - root 並列化に限定し、共有 deadline と結果統合を明確にする
- 並列化で探索順が変わるとノード数は比較しにくくなる
  - correctness と wall-clock 改善を主に見る
- thread 数を固定しすぎると環境依存が強くなる
  - 既定値と明示指定の扱いを決める必要がある

## 懸念と解決策

### TT の score / bound 規約

- 懸念:
  - exact solver では score が完全値なので、bound の扱いを雑にすると不正確になる
- 解決策:
  - 現在手番視点 score に統一する
  - `BoardKey` と remaining empties / depth 相当の条件を合わせて保存規約を固定する
  - 小さな終盤局面の differential test を増やす

### ordering のコスト

- 懸念:
  - ordering の計算コストが高いと exact solver では逆効果になる
- 解決策:
  - cheap な ordering から入れる
    - TT move
    - corner
    - 即終局 / 即最大利得
    - 自然順
  - 重い heuristic はこのステップでは入れない

### timeout と並列化

- 懸念:
  - worker が deadline 超過後もしばらく走る可能性がある
- 解決策:
  - 共有 deadline を全 worker が参照する
  - root move 単位で join しやすい構造にする
  - timeout 時は成功結果を返さず失敗へ統一する

### speedup の検証

- 懸念:
  - 盤面によって速度差がぶれる
- 解決策:
  - 固定終盤局面を複数用意する
  - `searched_nodes` と wall-clock の両方を確認する

### thread 数の扱い

- 懸念:
  - 並列度を固定すると環境依存が強くなる
- 解決策:
  - Step 32 では内部既定値で実装する
  - 既定は `available_parallelism()` ベースにする
  - 公開設定は後続に回す

## このステップを先に行う理由

Step 31 で Python から exact 探索を直接呼べるようになった。
ただし実用性の観点では、速度が足りなければ終盤限定でも使いにくい。
exact solver の高速化は、探索 API を利用可能な形にするための次の自然な段階である。

## 実装結果

- `solve_exact` / `search_best_move_exact` の内部 exact 探索に、exact solver 専用 transposition table を追加
- exact solver の ordering を強化
  - TT move
  - corner
  - 即勝ち手
  - 自然順
- `search_best_move_exact` に root 並列化を追加
  - ただし小さい終盤では並列化オーバーヘッドが勝つため、`empty < 12` または root legal move が少ない局面では serial fallback
  - root の先頭 3 手は serial に探索して alpha を立ててから残りを並列化
- timeout 超過時は引き続き partial result を返さず失敗結果を返す

## 計測結果

baseline は Step 31 相当 commit `5f59449`、比較対象は現行 Step 32 実装。
同一の終盤局面・同一の release build 条件で、公開 exact API `search_best_move_exact` の wall-clock を比較した。

- 10 空き・複数合法手
  - Step 31: `4.834592ms / 5回`
  - Step 32: `3.295283ms / 5回`
  - 改善率: 約 `31.8%` 短縮
- 12 空き・複数合法手
  - Step 31: `9.273769ms / 3回`
  - Step 32: `6.214051ms / 3回`
  - 改善率: 約 `33.0%` 短縮
- 14 空き・複数合法手
  - Step 31: `55.674663ms / 2回`
  - Step 32: `35.082427ms / 2回`
  - 改善率: 約 `37.0%` 短縮
- 16 空き・複数合法手
  - Step 31: `922.406635ms / 1回`
  - Step 32: `552.275225ms / 1回`
  - 改善率: 約 `40.1%` 短縮

現行 Step 32 の内部比較も行った。
`solve_exact`（serial exact）と `search_best_move_exact` を比べると、16 空き以下では並列化オーバーヘッドが勝つ局面が多かったため、既定挙動としては `empty < 18` を serial fallback にしている。

- 10 空き
  - serial: `3.207827ms / 5回`
  - `search_best_move_exact`: `3.295283ms / 5回`
- 12 空き
  - serial: `6.067300ms / 3回`
  - `search_best_move_exact`: `6.214051ms / 3回`
- 14 空き
  - serial: `37.058462ms / 2回`
  - `search_best_move_exact`: `35.082427ms / 2回`
- 16 空き
  - serial: `585.688291ms / 1回`
  - `search_best_move_exact`: `552.275225ms / 1回`

結論:
- exact solver 専用 TT
- ordering 強化
- `FxHashMap` 化
- deadline 判定の間引き
により、公開 exact API は Step 31 baseline より実測で高速化できた
- root 並列化は実装済みだが、現状は小さい終盤では使わない方が速いため、保守的な閾値で有効化している
