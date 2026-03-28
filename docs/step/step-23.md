# Step 23: 学習用バッチ整形 API

## このステップの目的

このステップでは、Step 22 までで作成できる supervised example / packed supervised example を、
PyTorch などの学習コードへそのまま流しやすい batch 形式へ変換する API を追加する。

主目的は次の4つをまとめて取り出せるようにすること。

- feature
- value target
- policy target
- legal move mask

## version 方針

- 学習支援基盤の拡張中は `0.0.1` を維持する
- 外部から見た学習 API が一段落した時点で次の version を検討する

## このステップで行うこと

- supervised example / packed supervised example から学習用 batch を作る Rust API を追加する
- CNN 向け feature を `(B, C, 8, 8)` で返す
- NNUE/MLP 向け feature を `(B, F)` で返す
- value target として `final_margin_from_black` を batch で返す
- policy target として `policy_target_index` を batch で返す
- legal move mask を batch で返す
- Python から `numpy.ndarray` で扱える helper を追加する
- `make check` が通る状態まで整える

## このステップの対象範囲

### 対象

- 学習用 batch Rust 型
- supervised example / packed supervised example からの batch 変換 helper
- Python 公開
- examples の最小更新

### 対象外

- 学習ループ本体
- 学習済みモデル推論
- policy 教師の質改善
- 保存フォーマットの変更

## 固定した前提

- CNN 向け feature は `(B, C, 8, 8)`
- NNUE/MLP 向け feature は `(B, F)`
- `B == 1` の場合も同じ batch API を使う
- value target は `final_margin_from_black`
- policy target は `policy_target_index`
- legal move mask は 64 マスに対応する batch 配列で返す
- Python 返り値は `numpy.ndarray` を優先する
- legal move mask は `(B, 64)` に固定する
- `policy_target_index` だけが `64` (pass) と `-1` (target なし) を持てる
- Step 23 では history なしの現局面 batch に限定する

## 受け入れ条件

- [ ] `(B, C, 8, 8)` の feature batch を作れる
- [ ] `(B, F)` の feature batch を作れる
- [ ] value target batch を作れる
- [ ] policy target batch を作れる
- [ ] legal move mask batch を作れる
- [ ] Python API が追加されている
- [ ] `make check` が成功する

## 実装方針

- feature 計算は既存の `encode_planes_batch` / `encode_flat_features_batch` を再利用する
- batch 対象はまず packed supervised example を主にする
- legal move mask は board から都度計算する
- Python wrapper は `numpy.ndarray` 返却に専念する
- Python 公開は
  - planes 学習 batch
  - flat 学習 batch
  の2系統にまとめる

## 懸念点

- policy target と legal move mask の関係
  - terminal と pass を含むため、mask と target の扱いを混同しやすい
  - Step 23 では `policy_target_index` はそのまま返し、mask は 64 マス分だけ返す

- history の扱い
  - feature 生成には history を渡せるが、保存済み例では履歴が moves 列のみ
  - Step 23 ではまず現局面だけを対象にし、history 付き batch は後続で検討する

- value target の標準化
  - `final_result` と `final_margin_from_black` のどちらを主 target にするかがぶれやすい
  - Step 23 では `final_margin_from_black` を標準 target に固定する

- API の増えすぎ
  - value/policy/CNN/flat を全部別々にすると散る
  - Step 23 では「学習用 batch 生成」に絞り、保存 API やモデル推論 API と混ぜない

## このステップを先に行う理由

Step 22 で学習データは保存できるようになったが、学習コード側では依然として
feature / label / legal mask を自前で組み立てる必要がある。
この変換をライブラリ側で持つことで、PyTorch などへ流すための接着コードを薄くできる。
