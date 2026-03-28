# Step 24: history 付き学習 batch API

## このステップの目的

このステップでは、Step 23 で追加した学習用 batch API を拡張し、
`history_len > 0` の feature 生成に対応させる。

これにより、現局面だけでなく直前局面を含む学習入力を、
保存済みデータからまとめて生成できるようにする。

## version 方針

- 学習支援基盤の拡張中は `0.0.1` を維持する
- history 対応まで揃っても、まだ基盤整備フェーズとして扱う

## このステップで行うこと

- history 復元に必要な最小情報をどこから取るかを固定する
- `prepare_planes_learning_batch` を `history_len > 0` に対応させる
- `prepare_flat_learning_batch` を `history_len > 0` に対応させる
- `B == 1` を含め、history 付き batch を Python から扱えるようにする
- `make check` が通る状態まで整える

## このステップの対象範囲

### 対象

- 学習用 batch API の history 対応
- history 復元 helper
- Python 公開
- tests

### 対象外

- 保存フォーマットの全面変更
- 学習ループ本体
- 学習済みモデル推論
- policy 教師の質改善

## 固定した前提

- history は `moves_until_here` から復元する
- board は current board として保存済み `PackedBoard` を使う
- 復元する history の順序は既存 feature API に合わせて「新しい順」
- 復元できない分は 0 埋め扱いになる既存 feature API の挙動を使う
- value target は引き続き `final_margin_from_black`
- policy target は引き続き `policy_target_index`
- legal move mask は引き続き `(B, 64)`

## 受け入れ条件

- [ ] `history_len > 0` で planes batch を作れる
- [ ] `history_len > 0` で flat batch を作れる
- [ ] 復元した history の順序が既存 feature API と一致する
- [ ] `B == 1` でも history 付き batch を作れる
- [ ] Python API が更新されている
- [ ] `make check` が成功する

## 実装方針

- 既存の `moves_until_here` を replay して board history を復元する
- 変換対象は引き続き packed supervised example を主にする
- history 復元は learning 側へ閉じ込める
- Python wrapper は引き続き `numpy.ndarray` 返却に専念する

## 懸念点

- replay コスト
  - example ごとに初期局面から replay すると重い
  - Step 24 では正しさ優先で replay し、最適化は後続に回す

- pass の扱い
  - `moves_until_here` には `None` が含まれる
  - forced pass を正しく replay しないと history が壊れる

- 保存形式との整合
  - 現在の保存形式は current board と moves のみ
  - Step 24 はこの形式のまま復元可能な範囲で進める

## このステップを先に行う理由

Step 23 で学習 batch API は揃ったが、history を含む入力を使いたい場合に
利用側で board 列を自前復元する必要がある。
この復元をライブラリ側へ寄せることで、学習コードをさらに薄くできる。
