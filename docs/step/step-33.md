# Step 33: Exact Solver Parallel Search Refinement

## このステップの目的

Step 32 で exact solver の高速化と root 並列化は入ったが、実測では並列化の効果が限定的だった。
このステップでは、exact solver の root 並列化を実際に speedup が出る形へ改善する。

主目的は次の 4 つ。

- shared TT を導入して worker 間の探索重複を減らす
- root parallel の窓共有を改善する
- 並列化を有効にする閾値を再調整する
- Step 32 より短い実時間になる局面を実測で確認する

## version 方針

- このステップ中は `0.2.2` を維持する
- このステップ完了後に `0.2.3` として切るかを判断する

## このステップで行うこと

### Phase 1: shared TT

- exact solver の root worker 間で共有できる TT を導入する
- correctness を壊さない保存規約を維持する

### Phase 2: root parallel の窓共有改善

- serial prefix で立てた alpha を worker がより有効に使えるようにする
- 必要なら root の探索順と worker 分配を見直す

### Phase 3: parallel 閾値の再調整

- `empty` と root legal move 数の条件を、実測ベースで見直す
- 小さい終盤では引き続き serial fallback を優先する

### Phase 4: 計測と検証

- Step 32 と同じ固定終盤局面で再計測する
- `make check` を通す
- timeout 失敗仕様と exact 結果一致を維持する

## このステップの対象範囲

### 対象

- `src/search.rs`
- `docs/step/step-33.md`
- `docs/step/todo.md`
- 必要最小限の README / docstring 更新

### 対象外

- midgame 探索の高速化
- Python 探索 API の typed object 化
- thread 数設定の公開 API 化

## 固定した前提

- 対象は exact solver の root 並列化改善のみ
- correctness を優先する
- timeout 超過時は partial result を返さず失敗を返す
- 公開 API の名前と戻り値 schema は変えない
- 小さい終盤では serial fallback を維持してよい
- 改善判断の主対象は `search_best_move_exact` とする
- shared TT は root parallel を使う局面に限定して適用してよい

## 受け入れ条件

- [ ] shared TT が入り、worker 間の探索重複が減っている
- [ ] root parallel の窓共有が Step 32 より改善している
- [ ] `search_best_move_exact` が少なくとも一部の終盤局面で Step 32 より短時間になる
- [ ] timeout 時の失敗結果仕様が維持されている
- [ ] exact 結果の正しさが既存テストで維持されている
- [ ] `make check` が成功する

## 懸念点

- shared TT のロック競合で逆に遅くなる可能性がある
- worker 間の alpha 共有を強めすぎると実装が複雑になり、correctness を壊しやすい
- 並列化の改善幅は局面依存なので、固定局面での再計測が必要
- shared state を入れた結果、timeout 時の停止仕様が壊れる可能性がある

## 懸念と解決策

### shared TT の競合

- 懸念:
  - 共有 TT にロックを入れると、終盤ではロックコストが勝つ可能性がある
- 解決策:
  - coarse-grained な単一ロックではなく、アクセス頻度を抑えた共有方式を選ぶ
  - 実測で悪化するなら適用帯域をさらに絞る

### root parallel の窓共有

- 懸念:
  - alpha 共有が弱いと Step 32 と同様に worker が無駄探索をする
- 解決策:
  - serial prefix と共有 alpha を維持しつつ、worker の探索開始条件を見直す
  - 固定局面で node 数と wall-clock を比較する

### speedup の再現性

- 懸念:
  - 1 局面だけの改善では汎用性が弱い
- 解決策:
  - Step 32 で使った複数の固定終盤局面で再計測する
  - Step 32 baseline と比較して記録する

### timeout 仕様の維持

- 懸念:
  - shared TT や共有 alpha を入れたことで、timeout 後も worker が長く走る可能性がある
- 解決策:
  - shared state を入れても deadline は各 worker が直接参照する
  - timeout 時は partial result を返さず失敗へ統一する
  - 既存の timeout テストを維持し、`make check` を通す

### 改善対象の優先順位

- 懸念:
  - `solve_exact` と `search_best_move_exact` の両方を同時に最適化しようとすると、判断軸がぶれる
- 解決策:
  - このステップでは公開 exact API である `search_best_move_exact` の改善を優先する
  - `solve_exact` は correctness を維持しつつ、必要なら多少保守的でもよい

### shared TT の適用帯域

- 懸念:
  - shared TT を serial fallback 帯域にまで広げると、ロックや共有コストで逆に遅くなる可能性がある
- 解決策:
  - shared TT は root parallel を使う局面だけに限定する
  - serial fallback は従来どおりローカル TT を使う
