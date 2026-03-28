# Step 22: 学習データの保存導線

## このステップの目的

このステップでは、Step 21 で作成した supervised example を、後続の学習処理へ流しやすい形に変換・保存する導線を整える。
特に、value 学習用ラベルと policy 学習用ラベルの両方を持てる保存表現を追加する。

方針は次のとおり。

- 深層学習の学習支援を主目的とする
- supervised example を軽量表現へ変換しやすくする
- value 学習用と policy 学習用の両方へ流しやすい形にする
- trace 単位と example 単位の両方を扱えるようにする
- ファイル形式を固定しすぎず、後続の学習コードへ渡しやすい形を優先する

## version 方針

- 学習支援基盤の拡張中は `0.0.1` を維持する
- 保存導線と examples が一段落した段階で次の version を検討する

## このステップで行うこと

- supervised example を保存向けの軽量表現へ変換する Rust API を追加する
- `PackedBoard` を使った board 保存表現を用意する
- `moves_until_here` を保存向けの `Vec<Option<u8>>` へ変換する
- policy 学習向けに `policy_target_index` を保存する
  - `-1`: policy target なし
  - `0..63`: 次手の着手マス
  - `64`: 次手が pass
- trace 単位 / example 単位で Python から扱いやすい helper を追加する
- examples ディレクトリを追加し、現在の基本 API を確認できる実行例を用意する
- `make check` が通る状態まで整える

## このステップの対象範囲

### 対象

- supervised example の保存向け Rust 型
- 保存向け変換 helper
- Python 公開
- `examples/` ディレクトリ
- 実行確認済みの基本サンプル
- PyTorch へ流しやすい参考例

### 対象外

- 学習ループ本体
- 学習済みモデル推論
- parquet / Arrow / HDF5 などの外部依存形式
- 保存フォーマットの最終固定

## 固定した前提

- 保存単位は example 単位を主とする
- board は `PackedBoard`
- `moves_until_here` は `Vec<Option<u8>>`
- Python では `dict` / `list[dict]` で扱いやすい helper を優先する
- examples はまず Python のみでよい
- value ラベルは `final_result` と `final_margin_from_black`
- policy ラベルは `policy_target_index` で表現する

## 受け入れ条件

- [x] supervised example を保存向け軽量表現へ変換できる
- [x] `PackedBoard` を含む example 単位の保存向け表現がある
- [x] value / policy の両方のラベルが保存表現に含まれる
- [x] Python API が追加されている
- [x] `examples/` が追加されている
- [x] examples の基本サンプルが実行確認できている
- [x] ランダムデータ生成と保存の example がある
- [x] 保存ディレクトリを読む PyTorch DataLoader 参考例がある
- [x] `make check` が成功する

## 実装方針

- `random_play.rs` に supervised example の保存向け変換ロジックを寄せる
- `python.rs` には wrapper のみを追加する
- examples は public API だけを使って記述する
- README には examples の実行方法を追記する
- PyTorch DataLoader の example はライブラリ本体の依存を増やさず、参考コードとして置く

## 懸念点

- 保存表現の固定しすぎ
  - 今の段階で最終形式を固定すると後で変更しづらい
  - Step 22 では軽量変換 helper に留める

- trace 単位と example 単位の両立
  - どちらも同時に重く作ると API が散る
  - Step 22 では example 単位を主にし、trace 単位は積み上げに留める

- policy ラベルの意味
  - ランダム対局由来の次手は強い教師とは限らない
  - Step 22 では保存可能にすることを優先し、教師の質改善は後続 step で扱う

- examples の肥大化
  - サンプルが増えすぎると保守が重い
  - まずは基本 API を一通り触る最小セットにする

## このステップを先に行う理由

Step 21 で supervised example 自体は生成できるようになったため、次に不足しているのは
「そのデータを軽量に保持し、学習コードへ受け渡す導線」と「現状 API の最低限の使用例」である。
ここを先に整えることで、保存処理と利用方法の両方が明確になる。

## 実装結果

- `random_play.rs` に `PackedSupervisedExample` を追加
- `packed_supervised_examples_from_trace`
- `packed_supervised_examples_from_traces`
- value ラベル
  - `final_result`
  - `final_margin_from_black`
- policy ラベル
  - `policy_target_index`
  - `-1`: target なし
  - `0..63`: 次手のマス
  - `64`: pass
- Python 公開
  - `packed_supervised_examples_from_trace`
  - `packed_supervised_examples_from_traces`
- examples
  - `examples/generate_training_data.py`
  - `examples/pytorch_dataloader.py`

## 検証結果

- `make check`: 成功
- `uv run python examples/generate_training_data.py --output-dir examples/generated_data --num-games 2 --seed 123`: 成功
- `uv run python -m py_compile examples/pytorch_dataloader.py`: 成功

## 補足

- PyTorch は標準依存に含めていないため、`examples/pytorch_dataloader.py` の runtime 実行確認はしていない
- DataLoader example は参考実装として置き、ライブラリ本体の依存は増やしていない
