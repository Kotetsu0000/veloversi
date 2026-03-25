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

### feature APIの仕様実装

- Flat 系:
  - `encode_flat_features(board) -> np.ndarray`
  - `encode_flat_features_batch(boards) -> np.ndarray`
  - shape は `(F,)`, `(B, F)` を基本にする
- Plane 系:
  - `encode_planes(board) -> np.ndarray`
  - `encode_planes_batch(boards) -> np.ndarray`
  - shape は `(C, 8, 8)`, `(B, C, 8, 8)` を基本にする
- Python 返り値は `numpy.ndarray`
- `channels_first` を前提にする
- 単一局面版と batch 版は内部実装を共通化する

### `ref`再現AIの最小推論版

- `ref` の探索思想を参考にした推論 API を実装する
- 最初の目標:
  - 1 局面に対する `best_move`
  - 評価値
  - 必要なら PV
- 深層学習AIとは別物として扱い、推論 API の主体はこちらにする
- `ref` AI については、インストール時 / ビルド時に有効化するかどうかを切り替えられるようにする
- 候補:
  - Cargo feature
  - maturin / Python 側の build option
- Python ライブラリ利用者が `ref` AI を含む構成と含まない構成を選べるようにする
    - Releaseには`ref`AIを含めない。

### 学習支援の保存導線

- ランダム局面生成結果と feature、最終結果をまとめて保存しやすい形を検討する
- 候補:
  - `PackedBoard` を使った軽量保存
  - `numpy` へそのまま流せる配列形式
  - trace 単位 / 局面単位の両対応
- Step 13 と Step 14 完了後に具体化する
