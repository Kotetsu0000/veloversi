# Step 25: recording / game record API

## このステップの目的

このステップでは、任意局面から試合記録を開始し、手を追加しながら、
最後に複数試合を 1 ファイルへ保存できる recording / game record API を追加する。

主目的は次の3つ。

- 任意局面を開始局面にした記録開始
- immutable な recording 更新
- 複数試合を 1 ファイルで管理できる保存 / 復元

## version 方針

- 学習支援基盤の拡張中は `0.0.1` を維持する
- recording / game record API が揃っても、まだ基盤整備フェーズとして扱う

## このステップで行うこと

- `random_start_board(...)` を追加する
- `start_game_recording(start_board)` を追加する
- `record_move(record, move)` を追加する
- `record_pass(record)` を追加する
- `current_board(record)` を追加する
- `finish_game_recording(record)` を追加する
- `append_game_record(path, record)` を追加する
- `load_game_records(path)` を追加する
- Python から `dict` で扱える公開 API を追加する
- `make check` が通る状態まで整える

## このステップの対象範囲

### 対象

- recording Rust 型
- game record Rust 型
- JSONL 保存 / 復元 helper
- Python 公開
- tests

### 対象外

- 学習ループ本体
- 学習済みモデル推論
- policy 教師の質改善
- parquet / Arrow / HDF5 などの外部依存形式

## 固定した前提

- recording は immutable にする
  - `record = record_move(record, move)` 形式
- Python 公開型は `dict`
- 基本方針として、`Board` でできて recording でできない操作は作らない
- 記録開始は任意局面から行える
- `moves` は `start_board` から先の手列として扱う
- `random_start_board(plies, seed)` は `Board` のみを返す
- recording 開始は `start_game_recording(start_board)` に分離する
- 保存形式はまず JSONL
- 1 レコード = 1 試合
- 1 ファイルに複数試合を保存する
- `append_game_record` は
  - ファイルが無ければ新規作成
  - ファイルがあれば形式確認の上で追記
  - 不正形式なら error
- `final_result` は `black` / `white` / `draw`
- game record には少なくとも次を含める
  - `start_board`
  - `moves`
  - `final_result`
  - `final_black_discs`
  - `final_white_discs`
  - `final_empty_discs`
  - `final_margin_from_black`

## 受け入れ条件

- [ ] 任意局面から recording を開始できる
- [ ] immutable な `record_move` / `record_pass` がある
- [ ] recording から current board を取得できる
- [ ] recording を game record へ確定できる
- [ ] JSONL へ保存できる
- [ ] JSONL から複数試合を復元できる
- [ ] Python API が追加されている
- [ ] `make check` が成功する

## 実装方針

- 内部では `Board` と recording を分離する
- recording は current board を内包する
- Python では `dict` を返し、board 的な操作感を helper で補う
- 保存は stateless な I/O 関数にする
- JSONL の各行は独立した game record とする
- `record_move` / `record_pass` は current board 更新と move 追加を一体で行う
- board 互換 API は Step 25 では最小限に留める
  - `current_board(record)`
  - `legal_moves_list(current_board(record))`
  - `board_status(current_board(record))`
  を基本導線にする
- `finish_game_recording` は終局局面でのみ成功させる
- `append_game_record` は Step 25 では既存ファイル全体を検証する
- `load_game_records` は `list[dict]` を返す

## 未決事項

- board 互換 API を今後どこまで広げるか
  - Step 25 では最小限に留めるが、将来どこまで recording へ寄せるかは未定

## 懸念点

- current board と move 列の整合
  - immutable recording では、current board 更新と move 追加がずれると壊れる
  - `record_move` / `record_pass` の内部で一体更新する必要がある

- `finish_game_recording` の呼び出し時点
  - 終局前でも呼べると `final_result` と石数の意味がぶれる
  - Step 25 では終局局面でのみ成功させる

- JSONL の format check コスト
  - 毎回フル検証すると大きいファイルで重い
  - Step 25 では正しさ優先で進め、必要なら後で最適化する

- board 互換 UX の範囲
  - 何でも recording で受けるようにすると wrapper が広がる
  - Step 25 では学習用途で必要な範囲を優先する

## このステップを先に行う理由

学習用データ生成では、標準初期局面だけでなく、任意局面から試合を開始して
その後の手だけを記録したい場面がある。
そのためには保存向け supervised example とは別に、
「試合記録そのもの」を扱う recording / game record API が必要になる。
