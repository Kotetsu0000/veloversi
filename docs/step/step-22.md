# Step 22: 学習データの保存導線

## このステップの目的

このステップでは、Step 21 で作成した supervised example を、後続の学習処理へ流しやすい形に変換・保存する導線を整える。

方針は次のとおり。

- 深層学習の学習支援を主目的とする
- supervised example を軽量表現へ変換しやすくする
- trace 単位と example 単位の両方を扱えるようにする
- ファイル形式を固定しすぎず、後続の学習コードへ渡しやすい形を優先する

## version 方針

- 学習支援基盤の拡張中は `0.0.1` を維持する
- 保存導線と examples が一段落した段階で次の version を検討する

## このステップで行うこと

- supervised example を保存向けの軽量表現へ変換する Rust API を追加する
- `PackedBoard` を使った board 保存表現を用意する
- `moves_until_here` を保存向けの `Vec<Option<u8>>` へ変換する
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

## 受け入れ条件

- [ ] supervised example を保存向け軽量表現へ変換できる
- [ ] `PackedBoard` を含む example 単位の保存向け表現がある
- [ ] Python API が追加されている
- [ ] `examples/` が追加されている
- [ ] examples の基本サンプルが実行確認できている
- [ ] `make check` が成功する

## 実装方針

- `random_play.rs` に supervised example の保存向け変換ロジックを寄せる
- `python.rs` には wrapper のみを追加する
- examples は public API だけを使って記述する
- README には examples の実行方法を追記する

## 懸念点

- 保存表現の固定しすぎ
  - 今の段階で最終形式を固定すると後で変更しづらい
  - Step 22 では軽量変換 helper に留める

- trace 単位と example 単位の両立
  - どちらも同時に重く作ると API が散る
  - Step 22 では example 単位を主にし、trace 単位は積み上げに留める

- examples の肥大化
  - サンプルが増えすぎると保守が重い
  - まずは基本 API を一通り触る最小セットにする

## このステップを先に行う理由

Step 21 で supervised example 自体は生成できるようになったため、次に不足しているのは
「そのデータを軽量に保持し、学習コードへ受け渡す導線」と「現状 API の最低限の使用例」である。
ここを先に整えることで、保存処理と利用方法の両方が明確になる。
