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

### random_playのtraceから学習データへ流す補助API

- `RandomGameTrace` から途中局面ごとの教師データを取り出しやすい helper を検討する
- 候補:
  - 各局面と最終勝敗を組にした列
  - 各局面と最終石差を組にした列
  - 各局面とそこまでの着手列を組にした列
- Step 13 では trace 本体までに留め、必要なら後続 step で helper を追加する

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

### `ref`再現AIの midgame 探索と公開導線

- Step 17 では exact/endgame 側の最小核まで実装した
- 次は `ref` の midgame 探索と `search_best_move` を段階的に寄せる
- 深層学習AIとは別物として扱い、推論 API の主体はこちらにする
- 候補:
  - `mid_evaluate_diff`
  - `nega_scout`
  - `search_best_move`
  - 必要なら PV
- `ref` AI については、インストール時 / ビルド時に有効化するかどうかを切り替えられるようにする
- 候補:
  - Cargo feature
  - maturin / Python 側の build option
- Python ライブラリ利用者が `ref` AI を含む構成と含まない構成を選べるようにする
    - Releaseには`ref`AIを含めない。
- `ref` AI 実装後に、AI 入りインストール導線を追加する
  - 通常インストールとは別に、`ref` AI を有効化した build / install モードを用意する
  - 最初は Git / sdist install から始め、必要なら後で Release artifact の分岐も検討する
  - README への記載は AI 実装完了後に行う

### 学習支援の保存導線

- ランダム局面生成結果と feature、最終結果をまとめて保存しやすい形を検討する
- 候補:
  - `PackedBoard` を使った軽量保存
  - `numpy` へそのまま流せる配列形式
  - trace 単位 / 局面単位の両対応
- Step 13 と Step 14 完了後に具体化する

### Git / sdist install 導線の整理

- Release wheel URL に加えて、将来 README に次の導線を追加する
  - `uv add "https://.../veloversi-<version>.tar.gz"`
  - `uv add "veloversi @ git+https://github.com/Kotetsu0000/veloversi.git@<tag>"`
- ただし README 反映は AI 入りインストール方針も含めて整理した後に行う
