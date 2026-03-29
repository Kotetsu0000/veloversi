# Step 29: 0.2.0 API Surface Alignment

## このステップの目的

`0.2.0` に向けて、Step 28 で追加した method-style API を主要な周辺機能まで広げる。

主目的は次の 4 つ。

- `Board` と `RecordedBoard` の操作感をさらに揃える
- symmetry / serialize / feature / model input を method-style でも使えるようにする
- examples と README を、module-level API ではなく method-style を主導線に統一する
- `0.2.0` に向けた公開 API の形を固める

## version 方針

- このステップ中は `0.1.0` を維持する
- このステップの完了条件を満たした時点で `0.2.0` を切る

## このステップで行うこと

- `Board` に次の method を追加する
  - `pack()`
  - `transform(sym)`
  - `encode_planes(history, config)`
  - `encode_flat_features(history, config)`
  - `prepare_cnn_model_input()`
  - `prepare_flat_model_input()`
- `RecordedBoard` に次の method を追加する
  - `pack()`
  - `transform(sym)` または `transform_current(sym)` のどちらかで current board を対象にする
  - `encode_planes(history=None, config=...)`
  - `encode_flat_features(history=None, config=...)`
  - `prepare_cnn_model_input()`
  - `prepare_flat_model_input()`
- batch API は module-level のまま維持する
- README と examples を method-style 優先で揃える
- `make check` を通す

## このステップの対象範囲

### 対象

- `src/veloversi/__init__.py`
- `src/veloversi/_core.pyi`
- Python examples
- README

### 対象外

- Rust core の board 表現そのものの変更
- 学習ループ本体
- 学習済みモデル runtime
- mutation quality の追加改善

## 固定した前提

- `Board` の純粋性は維持する
- `RecordedBoard` は current board を内部に持つ immutable な公開型とする
- method-style API を主導線にする
- batch API は module-level 関数のまま残す
- `RecordedBoard` の周辺 method は current board に対する操作として定義する

## 受け入れ条件

- [ ] `Board` で symmetry / serialize / feature / model input を method-style で使える
- [ ] `RecordedBoard` で current board ベースの symmetry / serialize / feature / model input を method-style で使える
- [ ] examples が method-style API 前提で揃っている
- [ ] README が method-style API 前提で揃っている
- [ ] `make check` が成功する

## 実装方針

- module-level API は即削除しない
- `Board` / `RecordedBoard` の method は既存 module-level helper を呼ぶ薄い wrapper にする
- `RecordedBoard` の `transform` / `pack` / feature 系は current board を対象にする
- history を受ける method は、`Board` では `list[Board]`、`RecordedBoard` では未指定時に current-only とする

## 懸念点

- `RecordedBoard.transform(...)` が record 全体を変換するのか current board だけを変換するのか曖昧になりやすい
  - Step 29 では current board 対象に限定する
- feature method に `history` をどう渡すかで API が複雑になりやすい
  - まずは既存 module-level API と同じ引数を優先し、`RecordedBoard` では未指定の簡易形も許容する
- README が method と関数の両方を並べると冗長になる
  - method-style を本文、module-level を補足に寄せる

## このステップを先に行う理由

Step 28 で core の盤面操作は method-style に揃った。
しかし、serialize / symmetry / feature / model input はまだ関数中心で、利用体験が途中までしか揃っていない。
`0.2.0` を API 整理の区切りにするなら、この層まで揃える必要がある。
