# Step 28: 0.2.0 API UX Rework

## このステップの目的

`0.2.0` に向けて、Python 公開 API の利用体験を整理する。

主目的は次の 4 つ。

- `Board` と recording の主要操作名を揃える
- module-level 関数中心の利用から method-style API へ寄せる
- recording を `dict` から board 的に扱える公開型へ移行する
- 利用者目線で「盤面操作」と「記録付き盤面操作」の差を最小化する

## version 方針

- このステップ中は `0.1.0` を維持する
- このステップの完了条件を満たした時点で `0.2.0` を切る

## このステップで行うこと

- `Board` に method-style API を追加する
  - `board.apply_move(move)`
  - `board.legal_moves_list()`
  - `board.is_legal_move(move)`
  - `board.board_status()`
  - `board.disc_count()`
  - `board.game_result()`
  - `board.final_margin_from_black()`
- recording を `dict` から公開型へ移行する
  - 仮称 `RecordedBoard`
  - immutable を維持する
- recording 側にも `Board` と同名の主要 method を追加する
  - `record.apply_move(move)`
  - `record.legal_moves_list()`
  - `record.is_legal_move(move)`
  - `record.board_status()`
  - `record.disc_count()`
  - `record.game_result()`
  - `record.final_margin_from_black()`
- `start_game_recording(start_board)` は `RecordedBoard` を返すようにする
- `random_start_board(...)` は `Board` を返すまま維持する
- 既存 module-level API は互換のため残すか、最小限の互換 wrapper に整理する
- README / examples を method-style 前提へ更新する
- `make check` を通す

## このステップの対象範囲

### 対象

- `src/python.rs`
- `src/lib.rs`
- Python 公開面
- examples
- README

### 対象外

- Rust core の board 表現そのものの変更
- 学習ループ本体
- 学習済みモデル runtime
- 新しい保存形式
- mutation quality の追加改善

## 固定した前提

- 内部では `Board` の純粋性を維持する
- Python での UX は board 的な操作感を優先する
- recording は immutable を維持する
- recording は Python 公開型 `RecordedBoard` として扱う
- 目標 UX は method-style を基本にする
  - `new_board = board.apply_move(19)`
  - `new_record = record.apply_move(19)`
- `Board` で使える主要操作は、recording 側でも同じ名前で使える状態を目指す
- `RecordedBoard` は `record.save_record(path)` のような保存 method を持つ
- module-level API の整理は段階的に進める
  - Step 28 中は既存の module-level API を残してよい
  - README / examples は method-style を主導線にする

## 受け入れ条件

- [ ] `Board` が method-style API を持つ
- [ ] recording 公開型が method-style API を持つ
- [ ] `Board` と recording の主要操作名が揃っている
- [ ] 既存 examples が method-style API 前提に更新されている
- [ ] `README.md` が新しい API に追従している
- [ ] `make check` が成功する

## 実装方針

- Rust core の `Board` はそのままにし、PyO3 公開面で method を追加する
- recording は `dict` ではなく PyO3 公開型へ移し、現在局面 `Board` を内部に持たせる
- 既存 module-level 関数は即削除せず、移行用の導線として残す
- ただし examples / README は新 API を優先する
- 保存 API は `RecordedBoard.save_record(path)` を主導線にし、内部では既存 helper を再利用する

## 懸念点

- recording を `dict` から公開型へ変えるため、Python 側では破壊的変更になる
  - `0.2.0` で扱う前提にする
- method 名を広げすぎると `Board` と recording の境界が曖昧になる
  - まずは主要操作に限定する
- module-level API と method-style API の二重管理で README が冗長になりやすい
  - README は method-style を主、module-level は互換として扱う
- `record.apply_move(move)` は `Board.apply_move(move)` と同名でも意味が広い
  - current board の更新と記録追加を同時に行うことを明示する

## このステップを先に行う理由

現状の `veloversi.apply_move(board, move)` / `veloversi.record_move(record, move)` は、機能としては足りているが利用体験が分裂している。
学習支援ライブラリとして使う際も、盤面と記録付き盤面の操作感が揃っていた方が扱いやすい。
`0.2.0` では、この利用体験の整理を最優先にする。
