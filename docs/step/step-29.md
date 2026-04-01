# Step 29: 0.2.0 API Surface Alignment

## このステップの目的

`0.2.0` に向けて、Step 28 で追加した method-style API を主要な周辺機能まで広げる。

主目的は次の 5 つ。

- `Board` の method-style API を symmetry / feature / model input まで広げる
- `RecordedBoard` を `Board` のラッパとして扱い、`Board` の主要操作をそのまま使えるようにする
- `RecordedBoard` は board 操作に加えて記録操作と保存操作を持つ
- examples と README を method-style API 優先に統一する
- 重複した公開導線を整理し、主導線と互換導線を明確にする

## version 方針

- このステップ中は `0.1.0` を維持する
- このステップの完了条件を満たした時点で `0.2.0` を切る

## version 更新

- `Cargo.toml` と `pyproject.toml` は `0.2.0` に更新した
- README の Release install URL も `v0.2.0` / `veloversi-0.2.0-*` に更新した

## このステップで行うこと

- `Board` に次の method を追加する
  - `transform(sym)`
  - `encode_planes(history, config)`
  - `encode_flat_features(history, config)`
  - `prepare_cnn_model_input()`
  - `prepare_flat_model_input()`
- `RecordedBoard` には `Board` の主要 method を forward する
  - `apply_move(move)`
  - `apply_forced_pass()`
  - `generate_legal_moves()`
  - `legal_moves_list()`
  - `is_legal_move(move)`
  - `board_status()`
  - `disc_count()`
  - `game_result()`
  - `final_margin_from_black()`
  - `transform(sym)`
  - `encode_planes(history, config)`
  - `encode_flat_features(history, config)`
  - `prepare_cnn_model_input()`
  - `prepare_flat_model_input()`
- `RecordedBoard` には記録操作も持たせる
  - `finish()`
  - `save_record(path)`
- `RecordedBoard.to_dict()` を追加し、現在の record 相当の辞書表現を返せるようにする
- batch API は module-level のまま維持する
- README と examples を method-style 優先で揃える
- 重複した導線を整理する
  - `pack_board(board)` と `board.to_bits()`
  - `apply_move(board, move)` と `board.apply_move(move)`
  - `record_move(record, move)` と `record.apply_move(move)`
- 次は追加しない
  - `Board.pack()`
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
- `RecordedBoard` は継承ではなくラッパだが、利用体験としては `Board` に近づける
- method-style API を主導線にする
- batch API は module-level 関数のまま残す
- `RecordedBoard` では `Board` の主要操作は可能な限り同名 method で forward し、記録操作では追加で履歴更新も行う
- `RecordedBoard.to_dict()` は進行中の recording を辞書化する
- `RecordedBoard.finish()` は終局済み current board から完成 game record の辞書を返す

## 受け入れ条件

- [x] `Board` で symmetry / feature / model input を method-style で使える
- [x] `RecordedBoard` は `Board` の主要 method を持ち、board と同じ感覚で扱える
- [x] `RecordedBoard` は記録操作 method と `to_dict()` / `save_record()` を持つ
- [x] examples が method-style API 前提で揃っている
- [x] README が method-style API 前提で揃っている
- [x] 主導線 API と互換 API の区別が README 上で明確である
- [x] `make check` が成功する

## 実装方針

- module-level API は即削除しない
- `Board` / `RecordedBoard` の method は既存 module-level helper を呼ぶ薄い wrapper にする
- history を受ける method は `Board` と `RecordedBoard` の両方に追加する
- `RecordedBoard` は `Board` の主要 method を可能な限り forward する
- 記録操作では current board 更新と move 記録を同時に行う
- `to_dict()` は現在の record 相当の辞書表現を返す
- `finish()` は完成 game record の辞書表現を返す
- `pack_board(board)` は互換として残すが、README では `board.to_bits()` を主導線にする

## 懸念点

- `RecordedBoard` が `Board` と似すぎると、単なる盤面と記録付き盤面の違いが見えにくくなる
  - 記録追加が起きる操作は README と docstring で明示する
- `RecordedBoard` の同名 method は current board に対する操作を意味する
  - `apply_move` / `apply_forced_pass` だけは current board 更新に加えて履歴更新も行う
- `to_dict()` と `finish()` の役割が混ざりやすい
  - `to_dict()` は進行中 recording、`finish()` は完成 game record として分ける
- README が method と関数の両方を並べると冗長になる
  - method-style を本文、module-level を補足に寄せる

## このステップを先に行う理由

Step 28 で core の盤面操作は method-style に揃った。
しかし、`RecordedBoard` はまだ `Board` のラッパとして十分に振る舞っておらず、symmetry / feature / model input も関数中心で、公開面には重複した導線が残っている。
`0.2.0` を API 整理の区切りにするなら、`RecordedBoard` を board 的に使える公開型へ寄せつつ、互換として残す関数の位置付けをここで固める必要がある。

## 実装結果

- `Board` に次の method-style API を追加した
  - `transform(sym)`
  - `encode_planes(history, config)`
  - `encode_flat_features(history, config)`
  - `prepare_cnn_model_input()`
  - `prepare_flat_model_input()`
- `RecordedBoard` に current board を対象とする同名 method を追加した
  - `transform(sym)`
  - `encode_planes(history, config)`
  - `encode_flat_features(history, config)`
  - `prepare_cnn_model_input()`
  - `prepare_flat_model_input()`
- `RecordedBoard` の記録操作と辞書化 API を維持した
  - `apply_move(move)`
  - `apply_forced_pass()`
  - `to_dict()`
  - `finish()`
  - `save_record(path)`
- README と examples は method-style を主導線として更新した

## 検証結果

- `uv run pytest -q src/test_python_api.py`: 成功
  - `51 passed`
- `uv run python examples/basic_usage.py`: 成功
- `uv run python examples/game_recording.py`: 成功
- `make check`: 成功
