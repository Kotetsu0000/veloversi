# Step 21: 学習データ化 API

## このステップの目的

このステップでは、`RandomGameTrace` から深層学習向けの教師データを直接取り出せる API を追加する。

方針は次のとおり。

- 主目的を深層学習の学習支援へ寄せる
- `ref` AI 再現前提の実装は新規に進めない
- 既存の `random_play` / `feature` / `serialize` をつなぐ導線を整える
- 1 step で検証しきれる範囲に留める

## version 方針

- 学習支援基盤の拡張中は `0.0.1` を維持する
- 学習用 API と保存導線が一段落した段階で次の version を検討する

## このステップで行うこと

- `RandomGameTrace` から途中局面ごとの学習サンプルを生成する Rust API を追加する
- 各サンプルに含める情報を固定する
  - `board`
  - `ply`
  - `moves_until_here`
  - `final_result`
  - `final_margin_from_black`
- 単一 trace と複数 trace の両方を扱いやすい形を用意する
- Python からも取得できる導線を追加する
- `make check` が通る状態まで整える

## このステップの対象範囲

### 対象

- supervised example 用の Rust 型
- `RandomGameTrace` から example 列を生成する helper
- Python 公開
- Rust / Python テスト

### 対象外

- 学習ループ本体
- optimizer
- GPU 学習 orchestration
- 学習済みモデル推論
- 保存フォーマットの最終確定
- policy/value 専用 feature 拡張

## 固定した前提

- まずは value 学習向けの教師データを優先する
- 各 example は途中局面と最終ラベルの対応を明示する
- move sequence は局面までの履歴を保持する
- Python 公開は扱いやすさを優先し、構造は単純にする
- `encode_*_batch` へ流しやすいデータ構造を意識する

## 受け入れ条件

- [x] `RandomGameTrace` から途中局面ごとの supervised example を生成できる
- [x] example に `board` / `ply` / `moves_until_here` / `final_result` / `final_margin_from_black` が含まれる
- [x] Rust API が追加されている
- [x] Python API が追加されている
- [x] 既存 `random_play` の挙動を壊していない
- [x] `make check` が成功する

## 実装方針

- `random_play.rs` に trace 変換ロジックを寄せる
- `python.rs` には wrapper だけを追加する
- まずは trace 単位 helper を主 API にする
- 複数 trace 向け helper は trace 単位 API を積み上げて実装する
- 既存の `Board` と `Move` をそのまま使い、余計な表現変換は増やさない

## 懸念点

- example の粒度
  - 局面単位か、手単位か、trace 単位かで API がぶれやすい
  - Step 21 では途中局面単位に固定する

- move sequence の保持コスト
  - 各 example に履歴全体を持たせるとメモリは増える
  - ただし学習用途では必要性が高いため、まずは明示保持を優先する

- Python 公開形
  - 型付き object にするか、`dict` / `list` にするかで複雑さが変わる
  - Step 21 では単純な公開形を優先する

## このステップを先に行う理由

すでに `random_play`、`feature`、`serialize` は揃っているため、次に不足しているのは
「trace をそのまま学習サンプルへ変換する導線」である。
ここを先に整えることで、その後の保存導線や学習済みモデル推論へ自然につなげられる。

## 実装結果

- `src/random_play.rs` に `SupervisedExample`、`supervised_examples_from_trace`、
  `supervised_examples_from_traces` を追加した
- supervised example には次を保持する
  - `board`
  - `ply`
  - `moves_until_here`
  - `final_result`
  - `final_margin_from_black`
- `src/python.rs` と `src/veloversi/__init__.py` に Python 導線を追加し、
  `dict` / `list[dict]` で受け取れるようにした
- trace 入力の Python 側検証を追加し、`boards` / `moves` / `plies_played` の整合を確認する構成にした

## 検証結果

- `make check`: 成功
  - Rust: `106 passed; 0 failed; 6 ignored`
  - Python: `31 passed`
