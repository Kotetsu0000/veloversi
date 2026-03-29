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

### recording API の board 互換拡張

- Step 25 では recording API を追加したが、board 互換 UX は最小限に留めている
- 後続 step で、必要なら次を検討する
  - `legal_moves_list(record)` のような recording 直接受理 helper
  - `board_status(record)` のような board 互換 wrapper
  - examples / README 上の高レベル導線整理

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
- Step 26 で `board` / `recording` からモデル入力を作る API は実装済み
- 以後はモデル読み込みと推論導線そのものに絞って具体化する

### value-only / policy+value 学習導線

- Step 26 で `value-only` / `policy + value` の DataLoader 例は実装済み
- 今後は実運用向けの helper 拡張や history 対応を検討する

### 0.1.0 以後の mutation quality 整理

- Step 27 時点で `make mutants` は `190 missed / 850 caught / 594 unviable / 42 timeouts`
- Step 27 の完了条件として、これらは
  - equivalent
  - timeout
  - 現実的に除去困難
  のいずれかに分類済み
- 残件の主な内訳
  - `engine.rs` の bit 演算 / movegen / perft 周辺
  - `python.rs` の PyO3 wrapper 変換関数
  - `search.rs` の探索ヒューリスティック内部
  - `random_play.rs` の sampling heuristic
- 以後は block ではなく品質改善として、必要なら分類ごとに追加で潰す
