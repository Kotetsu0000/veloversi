# Step 27: 0.1.0 Preparation

## このステップの目的

`0.1.0` を出せる状態まで、深層学習支援ライブラリとしての残件をまとめて閉じる。

主目的は次の4つ。

- `history_len > 0` を含む学習 batch API を完成させる
- DataLoader / examples / README を現在の API と整合させる
- `make check` / `make coverage-check` / `make mutants` を通す
- mutation 残件を、equivalent / timeout / 現実的に除去困難なもの以外は潰す

## version 方針

- このステップ中は `0.0.1` を維持する
- このステップの完了条件を満たした時点で `0.1.0` を切る

## このステップで行うこと

- `prepare_planes_learning_batch` の `history_len > 0` 対応
- `prepare_flat_learning_batch` の `history_len > 0` 対応
- `moves_until_here` から replay して history を復元する
- `value-only` / `policy + value` の example を history 対応後の API に合わせて整える
- `README.md` と `examples/README.md` を最終整備する
- `make check` を通す
- `make coverage-check` を通す
- `make mutants` を実行し、残件を整理する

## このステップの対象範囲

### 対象

- `src/learning.rs`
- 必要なら `src/random_play.rs`
- Python 公開面
- examples
- README
- mutation quality の改善

### 対象外

- 学習ループ本体
- 学習済みモデル推論 runtime
- parquet / Arrow / HDF5 など別保存形式
- 新しい AI / 探索器

## 固定した前提

- `history` は `moves_until_here` を初期局面から replay して復元する
- history の順序は既存 feature API に合わせて「新しい順」
- `value-only` は全 sample を対象にする
- `policy + value` は policy 有効 sample のみを対象にする
- value target は `final_margin_from_side_to_move / 64.0`
- pass は policy 学習の対象にしない
- `make mutants` の残件は
  - equivalent
  - timeout
  - 現実的に除去困難
  だけを許容する
- Step 27 完了後の commit はユーザーが行う
- こちらは commit を作らず、必要な commit コマンドだけ提示する

## 受け入れ条件

- [ ] `prepare_planes_learning_batch` が `history_len > 0` を扱える
- [ ] `prepare_flat_learning_batch` が `history_len > 0` を扱える
- [ ] replay 由来の history が既存 feature 契約どおり「新しい順」で入る
- [ ] `value-only` / `policy + value` の example が現状 API と一致する
- [ ] `README.md` / `examples/README.md` が最新状態に揃っている
- [ ] `make check` が成功する
- [ ] `make coverage-check` が成功する
- [ ] `make mutants` を実行済みである
- [ ] `make mutants` の残件が equivalent / timeout / 現実的に除去困難なものだけである

## 実装方針

- Step 24 の history 対応をこの step に取り込む
- replay は正しさ優先で初期局面から行う
- `None` は `apply_forced_pass` で厳密に進める
- `collate_fn` / model input API の契約は Step 26 を維持する
- mutation 残件は機械的に全潰しせず、理由を区別して整理する
- history replay では、replay 完了後の board が example の current board と一致することを直接検証する
- `make mutants` は
  - まず history 対応を通す
  - その後に実行する
  - 残件を分類して潰す
  の順で進める
- 残す `missed` は
  - equivalent
  - timeout
  - 現実的に除去困難
  のいずれかに明示分類する

## 懸念点

- history replay は batch ごとにコストがかかる
  - Step 27 では正しさ優先で進める
- `make mutants` は探索・feature・python wrapper に広く影響する
  - 途中で scope を広げすぎないようにする
- equivalent 判定を雑にすると品質確認の意味が薄くなる
  - 残す場合は理由を明文化する
- pass を含む replay は forced pass と一致しないと壊れる
  - `None` は forced pass としてのみ受理する

## このステップを先に行う理由

ここまでで深層学習支援に必要な API 群は概ね揃っている。
残っている本命は、history を含む batch 化と、`0.1.0` に出せる品質の確認である。
これをまとめて閉じることで、`0.1.0` の区切りを明確にできる。
