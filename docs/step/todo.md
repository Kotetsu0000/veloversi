# Todoリスト

このファイルはTodoリストです。今後のstepで行う内容を記載します。

## 使い方

- まだ`step-xx.md`に記載されていないが、今後行う内容を記載します。
- `step-xx.md`に記載された内容は、このTodoリストから削除します。
- 削除を忘れている物を発見したら、`step-xx.md`に記載された内容をこのTodoリストから削除してください。
- 追加する内容は以下にどんどん追記していってください。

## 書き方

Todoリストの内容は、以下のフォーマットで記載してください。

```
### 内容の見出し(見出しはかならずh3で)

ここにMarkdown形式で内容を記載
```

## Todoリスト

<!-- 以下から追記 -->

### feature mutation quality の改善

- Step 14 実装後の `cargo-mutants` で、feature 周辺に未捕捉が残っている
- 特に次を追加で直接検証したい
  - `phase_value`
  - `turn_value`
  - `write_bit_plane`
  - `write_bit_vector`
  - dense feature の要素位置固定テスト
- 機能追加ではなく品質改善として後続 step で扱う

### Python wrapper / random_play mutation quality の改善

- Step 15 実装後の `cargo-mutants` で、`python.rs` と `random_play.rs` に未捕捉が残っている
- 特に次を追加で直接検証したい
  - Python 文字列変換 helper
  - `PyBoard` の getter / `to_bits`
  - `pack_board_py` / `legal_moves_list` / `disc_count_py` などの wrapper 返り値
  - `XorShift64Star`
  - `sample_reachable_positions`
- 機能追加ではなく品質改善として後続 step で扱う

### 独自 midgame 評価の改善

- `eval.egev2` 同梱はライセンス上のリスクがあるため、現在は配布可能な独自静的評価を使っている
- 強さ改善が必要なら、後続 step で次を検討する
  - mobility / frontier / corner 周辺重みの再調整
  - 学習支援基盤を使った重み最適化
  - `ref` を直接同梱しない評価表現の再設計

### policy 教師の質改善

- Step 22 ではランダム対局由来の `policy_target_index` を保存する
- ただし random policy は強い教師ではないため、後続 step で次を検討する
  - 価値ラベル中心の学習データ生成
  - より強い policy source の導入
  - terminal / pass を含む policy target の扱い整理

### recording / game record API

- 任意局面から記録開始できる recording API を追加する
- 内部設計では `Board` と記録状態を分離し、`Board` 自体には履歴責務を持たせない
- ただし Python 公開面では board と近い操作感を維持する
  - `Board` で使える主要操作は recording に対しても使える形を目指す
- recording は immutable にする
  - `record = record_move(record, move)` 形式
- Python 公開型はまず `dict` にする
- 基本方針として、`Board` でできて recording でできない操作は作らない
- 検討対象:
  - `random_start_board(...)`
  - `start_game_recording(start_board)`
  - `record_move(...)`
  - `record_pass(...)`
  - `current_board(...)`
  - `finish_game_recording(...)`
  - `append_game_record(path, record)`
  - `load_game_records(path)`
- 保存形式の方針:
  - 1ファイルに複数試合
  - 1レコード = 1試合
  - まずは JSONL
  - `append_game_record` は
    - ファイルが無ければ新規作成
    - ファイルがあれば形式確認の上で追記
    - 不正形式なら error
- game record には少なくとも次を含める
  - `start_board`
  - `moves`
  - `final_result` (`black` / `white` / `draw`)
  - `final_black_discs`
  - `final_white_discs`
  - `final_empty_discs`
  - `final_margin_from_black`

### Git / sdist install 導線の整理

- Release wheel URL に加えて、将来 README に次の導線を追加する
  - `uv add "https://.../veloversi-<version>.tar.gz"`
  - `uv add "veloversi @ git+https://github.com/Kotetsu0000/veloversi.git@<tag>"`
- ただし README 反映は学習済みモデル導線も含めて整理した後に行う

### 学習済みモデル推論導線

- 学習済みモデルを Python から読み込み、`Board` / batch feature に対して推論できる導線を検討する
- 候補:
  - NumPy ベースの簡易推論 API
  - PyTorch モデルに渡しやすい helper
- Step 23 の学習用 batch API は実装済み
- 以後はモデル読み込みと推論導線そのものに絞って具体化する
