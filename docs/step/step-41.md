# Step 41: Search-evaluated Balanced Openings

## このステップの目的

Step 40 では、`plies` 手までの合法序盤局面を列挙し、model の静的評価だけで balanced opening file を生成できるようにした。

この step では、balanced 判定の精度を上げるために、候補局面を先読み探索で評価できるようにする。
あわせて、生成済み opening file / opening set を `for` 文で逐次扱える API を追加する。

## 背景

序盤局面の静的 model 評価だけでは、真に均衡した局面を選べない可能性がある。

理由:

- 序盤は長期的な手順依存が大きい
- model の単発評価は「その局面の見た目」への評価であり、双方が良く指した後の評価ではない
- `abs(model(board)) <= threshold` は「model が自信なさげな局面」を拾っているだけになる場合がある

より信頼できる balanced 判定には、

- 候補局面から数手先読みする
- leaf で model 評価する
- negamax の root value を balanced 判定に使う

という形が必要になる。

## version 方針

- Step 41 は `0.5.0` として release する
- Step 40 の API は維持し、探索評価は opt-in で追加する

## このステップで行うこと

### Phase 1: 保存済み opening を逐次出力する API

Python public API に次を追加する。

```python
iter_opening_boards(
    path_or_set,
    *,
    validate=True,
) -> Iterator[dict[str, object]]
```

使い方:

```python
for item in vv.iter_opening_boards("balanced-openings.jsonl"):
    sequence = item["sequence"]
    board = item["board"]
```

返す item は少なくとも次を持つ。

- `sequence`: `f5d6c4d3c2b3b4b5` のような着手列
- `board`: `Board`
- `black_bits`
- `white_bits`
- `side_to_move`

方針:

- `path_or_set` は `str` / `Path` / `BalancedOpeningSet` を受け付ける
- `str` / `Path` の場合は内部で `load_balanced_opening_file(path, validate=validate)` する
- `BalancedOpeningSet` の場合はその内容を順に yield する
- `BalancedOpeningSet` 入力時は既に load 済みとして `validate` は無視する
- 生成済み JSONL / `BalancedOpeningSet` の順序を維持する
- 新しく候補局面を生成・列挙する関数ではない
- `load_balanced_opening_file(...)` と同じ検証規約を使う
- 繰り返し使う場合は、先に `load_balanced_opening_file(...)` で `BalancedOpeningSet` を作って渡すことを推奨する
- path 入力時は一度全件 load する。初版では真の streaming parser は作らない
- 空の opening set は error にせず、何も yield しない
- public property の copy 方針は維持し、iterator 実装では private helper で内部 list を読む
- iterator item は entry の field を pass-through し、`board` を追加した新しい dict として返す

### Phase 2: 探索付き評価モードの追加

`generate_balanced_opening_file(...)` に探索評価オプションを追加する。

```python
generate_balanced_opening_file(
    model,
    path,
    *,
    plies=8,
    threshold=2 / 64,
    value_scale="normalized",
    batch_size=1024,
    dedupe_symmetry=True,
    device="cpu",
    eval_depth=0,
    eval_mode="static",
    prefilter_threshold=None,
)
```

意味:

- `eval_mode="static"`
  - Step 40 と同じ
  - 候補局面そのものを model で評価する
  - `eval_depth` は 0 のみ許容
- `eval_mode="value_search"`
  - 各候補局面から `eval_depth` 手先読みする
  - leaf で model raw value を評価する
  - negamax root value を `raw_value` として扱う
  - `eval_depth >= 1` を要求する

validation:

- `eval_mode` は `"static"` / `"value_search"` のみ許容する
- `eval_depth` は int で、`0 <= eval_depth <= 60` のみ許容する
- `eval_mode="static"` では `eval_depth == 0` のみ許容する
- `eval_mode="value_search"` では `eval_depth >= 1` を要求する
- 不正な組み合わせは `ValueError` にする

value model の前提:

- model output は現在手番視点の value として扱う
- 黒視点 value model 対応は初版では対象外にする
- `value_perspective` option は追加しない。必要になったら後続で検討する
- PyTorch model の input format は生成開始時に 1 回だけ検出し、prefilter と search leaf で共有する
- PyTorch model は生成開始時に `eval()` し、終了時または例外時に元の training 状態へ戻す
- batch value output は `(B,)` / `(B, 1)` のみ許容し、policy output は `ValueError` にする

探索内部は raw value で計算し、JSONL には次を保存する。

- `raw_value`
- `normalized_value = tanh(raw_value)`
- `filter_value`
- `eval_mode`
- `eval_depth`

field の意味:

- `eval_mode="static"` の `raw_value` は候補局面そのものの model raw value
- `eval_mode="value_search"` の `raw_value` は探索 root raw value
- `value_scale="raw"` の threshold は、`eval_mode` ごとの `raw_value` 単位で判定する
- Step 40 schema の行に `eval_mode` / `eval_depth` がない場合、loader は `"static"` / `0` として補完する

### Phase 3: prefilter

探索評価は重いので、任意で静的評価による prefilter を入れる。

```python
prefilter_threshold=8 / 64
```

挙動:

- `prefilter_threshold is None`
  - 全候補に探索評価をかける
- `prefilter_threshold` が指定されている
  - まず静的評価を batch で行う
  - `abs(static_filter_value) <= prefilter_threshold` の候補だけ探索評価へ進める
- `prefilter_threshold` は final threshold と同じ `value_scale` で判定する
- prefilter / search / final のどの段階でも non-finite value は保存せず skip する
- `prefilter_threshold` は高速化用であり、balanced な候補を落とす可能性がある
- 精度優先なら `prefilter_threshold=None` を使う
- 使う場合は final threshold より広めの値を README で推奨する
- `prefilter_threshold is not None` のときだけ、JSONL に `prefilter_raw_value`, `prefilter_normalized_value`, `prefilter_filter_value` を保存する
- prefilter なしの場合、prefilter field は省略する

stats には次を含める。

- `generated`: 実際に評価候補になった dedupe 後の opening candidate 数
- `prefiltered`: prefilter 通過数。prefilter なしなら `generated`
- `searched`: value search を実行した候補数。static なら `0`
- `accepted`: JSONL に保存した数
- `skipped_non_finite`: prefilter / search / final で non-finite により除外した合計数

### Phase 4: 探索実装

Step 39 の value search helper を利用する。

方針:

- `select_move_with_model(...)` を候補ごとに呼ぶのではなく、内部 helper を直接使う
- leaf evaluator は Step 40 の model batch/static 評価 helper と同じ value 規約にする
- 初版では探索中 leaf の batch 化はしない
- PyTorch model でも探索 leaf は 1 局面ずつ評価してよい
- 後続で leaf batch / transposition table を検討する
- exact fallback は使わない
- terminal / forced pass は Step 39 の value search helper と同じ規約にする
- terminal value と raw model value の scale 差は Step 39 の既存規約を優先し、後続改善候補にする
- timeout / node limit は初版では追加しない
- search 並列化は初版では追加しない
- model 評価中の例外は握りつぶさず伝播させる
- temporary file 経由の atomic write を維持し、失敗時は既存 target path を壊さない
- progress callback / resume / incremental generation は初版では追加しない

determinism:

- opening 候補の列挙順は合法手昇順 DFS に固定する
- symmetry dedupe の代表は最初に出た sequence に固定する
- candidate に元 index を持たせ、prefilter / search 後も元 index 昇順で JSONL に書く
- 将来並列化しても output order は元順に戻す

### Phase 5: 文書とテスト

- README に `iter_opening_boards(...)` の例を追加する
- README に `eval_mode="value_search"` の例を追加する
- README に、value model は現在手番視点であること、`value_scale="raw"` は advanced option であることを書く
- README に、GPU での value search は leaf 1 件評価のため遅い可能性があることを書く
- README に、`plies=8` は慣例的な既定値であり、他の値では `side_to_move` が変わることを書く
- README に、`iter_opening_boards(...)` と `BalancedOpeningSet.boards` の使い分けを書く
- README では「model-filtered balanced openings」と表現し、絶対的な互角性を保証しないことを書く
- Python stub を更新する
- pytest を追加する
- pytest は `plies=1` or `plies=2`, `eval_depth=1` の軽量ケースに固定する
- `make check` を通す

## 対象範囲

### 対象

- `src/veloversi/__init__.py`
- `src/veloversi/__init__.pyi`
- `src/test_python_api.py`
- `README.md`
- `docs/step/step-41.md`

### 対象外

- Rust core への opening iterator 実装
- 初期局面から合法 opening 候補を直接 public iterator として列挙する API
- RustValueModel の batch 探索 leaf 評価
- transposition table の導入
- exact solver を使った opening 判定
- 公式 XOT リストの同梱

## 固定した前提

- Step 40 の静的評価 API は維持する
- `iter_opening_boards(...)` は保存済み opening file / opening set を読む public API として追加する
- 初期局面から合法 opening 候補を直接 public iterator として列挙する API は追加しない
- `generate_balanced_opening_file(...)` の既定挙動は Step 40 と同じ static 評価
- 探索評価は `eval_mode="value_search"` で opt-in
- `eval_mode="static"` は `eval_depth == 0` のみ許容し、`eval_mode="value_search"` は `eval_depth >= 1` を要求する
- `eval_depth` は `0 <= eval_depth <= 60` の int に固定する
- 探索内部は raw value を使う
- JSONL の `value` 系 field は Step 40 と同じ規約を維持し、`normalized_value = tanh(raw_value)` とする
- value model output は現在手番視点に固定する
- 黒視点 value model 対応、`value_perspective` option、exact mode は対象外にする
- 初版では探索 leaf の batch 化はしない
- 初版では timeout / node limit / search 並列化 / progress callback / resume は追加しない
- prefilter は final threshold と同じ `value_scale` で判定し、指定時のみ prefilter value field を保存する
- non-finite value は prefilter / search / final のどの段階でも skip し、`skipped_non_finite` に合算する
- output は deterministic にする。合法手昇順 DFS、dedupe は最初の sequence、書き出しは元 index 昇順に固定する
- JSONL は per-row schema を維持し、metadata header 行は初版では導入しない
- loader は Step 40 schema と Step 41 schema の両方を読む。未知 field は pass-through する
- `a1` は square `0`、`h8` は square `63` の既存座標規約に固定する
- pass は sequence に含めない。初版では `plies` / `played_moves` field は追加しない
- public API は `iter_opening_boards`, `generate_balanced_opening_file`, `load_balanced_opening_file`, `random_balanced_opening_board`, `BalancedOpeningSet` に限定する
- Step 41 の version は `0.5.0` に固定する

## 受け入れ条件

- [x] `iter_opening_boards(path_or_set, *, validate=True)` が追加されている
- [x] `for item in iter_opening_boards(path)` で保存済み opening file の全局面を逐次取得できる
- [x] `for item in iter_opening_boards(opening_set)` で読み込み済み set の全局面を逐次取得できる
- [x] iterator item に `sequence`, `board`, `black_bits`, `white_bits`, `side_to_move`, `raw_value`, `normalized_value`, `filter_value` が含まれる
- [x] iterator item は未知 field を保持し、entry に `board` を追加した dict として返る
- [x] JSONL / `BalancedOpeningSet` の順序を維持して yield する
- [x] path 入力時は `validate` 引数が `load_balanced_opening_file(...)` に反映される
- [x] `BalancedOpeningSet` 入力時は `validate` を無視し、空 set は何も yield しない
- [x] `generate_balanced_opening_file(...)` が `eval_mode` / `eval_depth` を受け付ける
- [x] 既定の static 挙動が Step 40 と互換である
- [x] `eval_mode` / `eval_depth` の不正な組み合わせが `ValueError` になる
- [x] `eval_mode="value_search"` で探索評価を使って balanced 判定できる
- [x] `prefilter_threshold` が指定された場合、静的評価で候補を絞ってから探索する
- [x] prefilter value field は prefilter 指定時だけ JSONL に保存される
- [x] stats に `generated` / `prefiltered` / `searched` / `accepted` / `skipped_non_finite` が含まれ、定義どおりに増減する
- [x] Step 40 schema の JSONL を読み込むと `eval_mode="static"` / `eval_depth=0` として補完される
- [x] Step 41 schema の JSONL は未知 field を落とさず読み込める
- [x] output order が deterministic で、prefilter / search の有無で元候補順が崩れない
- [x] model 例外時に既存 target file が壊れない
- [x] PyTorch model の training 状態が例外時も復元される
- [x] version が `0.5.0` に更新されている
- [x] README / stub / tests が更新されている
- [x] `make check` が成功する

## 設計時に潰した懸念

以下は上の確定仕様の根拠として残す。実装時は「推奨案」を採用済み方針として扱う。

### `iter_opening_boards(...)` の責務

- 懸念:
  - 名前だけ見ると、初期局面から opening 候補を生成する API に見える可能性がある
  - 実際には保存済み opening file / `BalancedOpeningSet` を読み出す API である
- 推奨案:
  - `iter_opening_boards(path_or_set, *, validate=True)` は load 済みデータを逐次読む API に固定する
  - 初期局面から合法候補を列挙する helper は private のままにする
  - docstring と README に「balanced opening file / set を読む」と明記する

### `iter_opening_boards(...)` の返却 item schema

- 懸念:
  - `BalancedOpeningSet.entries` の dict と iterator item の dict が微妙に違うと利用側が混乱する
- 推奨案:
  - iterator item は entry の内容を保ちつつ `board` を追加した dict にする
  - 少なくとも次を常に含める
    - `sequence`
    - `board`
    - `black_bits`
    - `white_bits`
    - `side_to_move`
    - `raw_value`
    - `normalized_value`
    - `filter_value`
  - 将来追加 field があっても pass-through する

### path 入力時の load cost

- 懸念:
  - `iter_opening_boards(path)` を何度も呼ぶと毎回 file を読み直す
- 推奨案:
  - path 入力は利便性用とする
  - 繰り返し利用では `load_balanced_opening_file(...)` で `BalancedOpeningSet` を作って渡すことを README で推奨する

### validate の既定値

- 懸念:
  - `validate=True` は安全だが、sequence replay が入るため遅い
  - `validate=False` は速いが壊れた JSONL を見逃す
- 推奨案:
  - 既定値は `validate=True` にする
  - 速度が必要な用途では明示的に `validate=False` を指定する
  - `BalancedOpeningSet` 入力時は既に load 済みなので `validate` は無視する

### iterator の型

- 懸念:
  - list を返すのか generator を返すのかが曖昧だとメモリ使用の期待がずれる
- 推奨案:
  - `iter_opening_boards(...)` は generator / iterator を返す
  - path 入力時は `load_balanced_opening_file(...)` が一度全件 load するため、完全 streaming ではないことを明記する
  - 初版では全件 load で十分とし、真の streaming parser は後続検討にする

### `eval_mode` と `eval_depth` の組み合わせ

- 懸念:
  - `eval_mode="static"` なのに `eval_depth > 0` など、意味のない組み合わせが発生する
- 推奨案:
  - `eval_mode="static"` では `eval_depth == 0` のみ許容する
  - `eval_mode="value_search"` では `eval_depth >= 1` を要求する
  - 不正な組み合わせは `ValueError` にする

### `eval_depth` の上限

- 懸念:
  - 大きすぎる `eval_depth` は現実的に終わらない
- 推奨案:
  - 初版では `eval_depth` は `0 <= eval_depth <= 60` の int とする
  - 実行時間は利用者責任とし、README では小さい depth から使う例にする
  - 必要なら後続で timeout / node limit を追加する

### 探索評価の timeout / 中断

- 懸念:
  - opening file 生成中に候補ごとの探索が長時間止まる可能性がある
- 推奨案:
  - Step 41 初版では timeout は追加しない
  - 代わりに `prefilter_threshold` と小さい `eval_depth` を推奨する
  - 後続 step で `search_timeout_seconds` / `node_limit` を検討する

### prefilter の value scale

- 懸念:
  - final threshold の `value_scale` と prefilter の scale がずれると分かりにくい
- 推奨案:
  - `prefilter_threshold` は final threshold と同じ `value_scale` で判定する
  - `value_scale="normalized"` なら static `normalized_value`
  - `value_scale="raw"` なら static `raw_value`
  - stats に `prefiltered` を含める

### prefilter の non-finite value

- 懸念:
  - prefilter 段階で `nan` / `inf` が出た候補をどう扱うかが曖昧
- 推奨案:
  - prefilter / search / final のどの段階でも non-finite は保存せず skip
  - stats の `skipped_non_finite` に合算する

### 探索評価の符号規約

- 懸念:
  - negamax では side-to-move 視点の raw value を扱うため、符号反転の実装を間違えると balanced 判定が壊れる
- 推奨案:
  - Step 39 の value search helper と同じ raw value 符号規約を使う
  - leaf evaluator は「現在手番視点」の raw value を返す
  - root で得た raw score を `raw_value` として保存する
  - `normalized_value = tanh(raw_value)` は探索完了後にだけ計算する

### exact / terminal / forced pass の扱い

- 懸念:
  - 探索中に terminal / forced pass が出た場合の評価規約が static 評価と異なる
- 推奨案:
  - Step 39 の value search helper と同じ規約にする
  - terminal は final margin を side-to-move 視点 normalized value として返す既存挙動に従う
  - forced pass は side-to-move を反転して探索を継続する
  - search raw と terminal normalized が混ざる問題は既存 helper の規約を優先する

### terminal value と raw model value の scale 差

- 懸念:
  - 探索中、leaf model は raw value、terminal は margin / 64 の normalized value になり、scale が混在する
- 推奨案:
  - Step 41 初版では Step 39 の既存 helper 規約を維持する
  - この scale 差は既存 value search の課題として扱う
  - 必要なら後続で terminal も raw 相当に変換する設計を検討する

### 探索評価の実行時間

- 懸念:
  - `plies=8` の全候補に depth search をかけるとかなり重い
- 推奨案:
  - `eval_mode="static"` を既定にする
  - `prefilter_threshold` を用意する
  - `eval_depth` は opt-in にする

### 探索 leaf の batch 化

- 懸念:
  - leaf を 1 局面ずつ model 評価すると遅い
- 推奨案:
  - 初版では正しさ優先で 1 局面評価にする
  - 後続で batch leaf 評価を検討する

### PyTorch model の eval/train 復元

- 懸念:
  - 探索評価では model 呼び出し回数が多く、training mode のままだと dropout / batchnorm で結果が不安定になる
- 推奨案:
  - Step 40 と同じく生成開始時に `eval()` し、終了時に元の training 状態へ戻す
  - 例外時も `finally` で復元する

### `select_move_with_model(...)` を直接呼ばない理由

- 懸念:
  - 候補ごとに `select_move_with_model(...)` を呼ぶ方が簡単だが、exact fallback や返却 dict 処理が混ざって重い
- 推奨案:
  - Step 39 の内部 value search helper を直接使う
  - exact fallback は balanced opening 生成では使わない
  - 探索評価の責務を「model leaf による value search」に限定する

### stats の意味

- 懸念:
  - `generated`, `prefiltered`, `searched`, `accepted` の意味が曖昧だと生成結果を評価しにくい
- 推奨案:
  - `generated`: opening candidate 総数
  - `prefiltered`: prefilter 通過数。prefilter なしなら `generated`
  - `searched`: value_search を実行した候補数。static なら `0`
  - `accepted`: JSONL に保存した数
  - `skipped_non_finite`: non-finite で除外した数

### JSONL schema の互換性

- 懸念:
  - Step 41 で `eval_mode` / `eval_depth` を追加すると、Step 40 の loader が古い schema をどう扱うかが問題になる
- 推奨案:
  - Step 41 の loader は Step 40 schema と Step 41 schema の両方を読めるようにする
  - `eval_mode` / `eval_depth` がない行は `"static"` / `0` 相当として扱う
  - writer は新 field を保存する

### README の表現

- 懸念:
  - 「balanced」と書くと絶対的に互角な opening だと誤解される
- 推奨案:
  - README では「model-filtered balanced openings」と表現する
  - `eval_mode="value_search"` は静的評価より強いが、model と depth に依存すると明記する

### model value の視点

- 懸念:
  - balanced 判定では value が「現在手番視点」なのか「黒視点」なのかが重要
  - `select_move_with_model(...)` の value search は現在手番視点の評価を前提にしている
  - 黒視点 value model を渡すと、符号が合わず balanced 判定が壊れる
- 推奨案:
  - `generate_balanced_opening_file(...)` は現在手番視点 value model を前提にする
  - README / docstring に「value model output must be from side-to-move perspective」と明記する
  - 黒視点 model 対応は初版では対象外にする
  - 後続で必要なら `value_perspective="side_to_move" | "black"` を追加する

### static 評価と search 評価の field 名

- 懸念:
  - `raw_value` が static raw なのか search root raw なのか、JSONL だけ見ても分かりにくい
- 推奨案:
  - `eval_mode` / `eval_depth` を必ず保存する
  - `eval_mode="static"` では `raw_value` は局面そのものの model raw
  - `eval_mode="value_search"` では `raw_value` は探索 root raw
  - prefilter の値を保存する場合は `prefilter_raw_value` / `prefilter_normalized_value` として別 field にする

### prefilter 値を JSONL に保存するか

- 懸念:
  - prefilter でなぜ通ったのかを後から確認できない
  - ただし field を増やすと schema が複雑になる
- 推奨案:
  - `prefilter_threshold is not None` のときだけ `prefilter_raw_value`, `prefilter_normalized_value`, `prefilter_filter_value` を保存する
  - prefilter なしの場合は field を省略する
  - loader は未知 field を pass-through する

### loader の未知 field

- 懸念:
  - Step 41 以後に JSONL field が増えたとき、loader が落ちると互換性が弱い
- 推奨案:
  - 必須 field だけ validation する
  - 未知 field は entry dict に残して pass-through する
  - `iter_opening_boards(...)` も未知 field を保持した item を yield する

### Step 40 schema との互換性

- 懸念:
  - Step 40 の JSONL には `eval_mode` / `eval_depth` がない
- 推奨案:
  - loader は field がない場合に `eval_mode="static"`, `eval_depth=0` として扱う
  - `iter_opening_boards(...)` の item には補完後の `eval_mode` / `eval_depth` を含める

### deterministic output

- 懸念:
  - 生成した JSONL の順序が実行ごとに変わると、diff や sampling 結果が不安定になる
- 推奨案:
  - opening 候補の列挙順は合法手昇順 DFS に固定する
  - symmetry dedupe の代表は最初に出た sequence に固定する
  - prefilter / search 後も元の候補順を維持して JSONL に書く
  - 並列化を入れる場合も output order は元順に戻す

### search 評価の並列化

- 懸念:
  - 探索評価は重いが、並列化すると model や PyTorch device 周りが複雑になる
- 推奨案:
  - Step 41 初版では並列化しない
  - output determinism と実装単純性を優先する
  - 後続で `worker_count` を追加する場合は CPU/RustValueModel 中心に検討する

### GPU 利用時の leaf 1件評価

- 懸念:
  - PyTorch model を GPU で使う場合、search leaf を 1 件ずつ評価すると極端に遅い可能性がある
- 推奨案:
  - Step 41 初版では leaf batch 化しないことを明記する
  - GPU での `eval_mode="value_search"` は遅い可能性があると README に書く
  - 後続で leaf batch queue を検討する

### prefilter と search で model 入力形式検出が重複する

- 懸念:
  - static prefilter と search leaf で毎回 input format を検出すると遅く、挙動も不安定になる
- 推奨案:
  - 生成開始時に PyTorch model の input format を 1 回だけ検出する
  - prefilter と search leaf は同じ input format を使う
  - RustValueModel は NNUE path 固定にする

### model が policy/value 両対応の場合

- 懸念:
  - model の出力が入力や mode により policy / value で変わる設計だと、自動判定が曖昧になる
- 推奨案:
  - Step 41 では Step 40 と同じ自動判定に従う
  - batch value output は `(B,)` / `(B, 1)` のみ許容する
  - policy output が検出されたら `ValueError`
  - 必要なら後続で `model_output="value"` のような明示 option を追加する

### `eval_mode="value_search"` と `value_scale="raw"`

- 懸念:
  - search root raw は model raw と完全に同じ分布とは限らない
  - `value_scale="raw"` の threshold を static と同じ感覚で使うと誤解しやすい
- 推奨案:
  - 既定は `value_scale="normalized"` に維持する
  - `value_scale="raw"` は advanced option として docs に明記する
  - `eval_mode` に関係なく raw threshold は「その mode の raw_value 単位」と定義する

### `plies` が奇数のときの side-to-move

- 懸念:
  - `plies` が奇数か偶数かで side-to-move が変わり、model value の視点も変わる
- 推奨案:
  - 現在手番視点 value model 前提なら問題ない
  - JSONL には必ず `side_to_move` を保存する
  - README で `plies=8` は慣例的な既定値であり、他の値では手番が変わることを明記する

### forced pass を含む opening sequence

- 懸念:
  - `plies` は board を進めた回数だが、sequence には pass を含めない方針なので、sequence 長と plies が一致しない場合がある
- 推奨案:
  - Step 40 と同じく pass は sequence に含めない
  - JSONL に `plies` / `played_moves` を追加するか検討する
  - 初版では序盤で forced pass は通常起きないため、既存方針を維持する

### `iter_opening_boards(...)` のコピーコスト

- 懸念:
  - `BalancedOpeningSet.entries` / `boards` が copy を返す場合、iterator 実装で毎回大きな copy が発生する可能性がある
- 推奨案:
  - `iter_opening_boards(...)` は `BalancedOpeningSet` の内部 list を直接読む private helper を使う
  - public property は copy を返す方針を維持する
  - iterator item は 1 件ずつ新しい dict として返す

### `BalancedOpeningSet` が mutable に見える

- 懸念:
  - `entries` が list[dict] を返すため、利用者が変更できるように見える
- 推奨案:
  - property は copy を返し、内部状態は変更されないようにする
  - docstring に copy であることを明記する
  - 厳密な immutable 型化は後続で検討する

### file path と set の validate 挙動差

- 懸念:
  - `iter_opening_boards(path, validate=True)` と `iter_opening_boards(opening_set, validate=True)` で挙動が違う
- 推奨案:
  - path 入力では load 時に validate を使う
  - set 入力では既に load 済みとして validate は無視する
  - docstring に明記する

### empty set の iterator

- 懸念:
  - `random_balanced_opening_board(...)` は空 set で `ValueError` だが、iterator も error にするか迷う
- 推奨案:
  - `iter_opening_boards(...)` は空 set なら何も yield しない
  - `random_balanced_opening_board(...)` だけ `ValueError` にする

### tests の実行時間

- 懸念:
  - Step 41 の探索評価テストを `plies=8`, `eval_depth>=2` で書くと重い
- 推奨案:
  - pytest では `plies=1` or `plies=2`, `eval_depth=1` に固定する
  - 実用例は README に `plies=8`, `eval_depth=2` として示す
  - 長時間テストは追加しない

### version 方針

- 懸念:
  - API 追加だが Step 40 直後なので patch か minor か迷う
- 推奨案:
  - `iter_opening_boards(...)` と探索評価 option の追加は feature なので `0.5.0` とする

### JSONL format version

- 懸念:
  - Step 41 で field が増え、将来さらに field が増えると、どの生成仕様の file か判定しづらくなる
  - per-row schema だけでは file 全体の生成条件を確認しにくい
- 推奨案:
  - 初版では metadata header 行は導入せず、Step 40 互換の per-row JSONL を維持する
  - 代わりに各 row に `eval_mode` / `eval_depth` を保存する
  - file 全体 metadata が必要になったら後続で `{"type":"metadata", ...}` 行を導入する
  - loader は unknown `type` を拒否せず、metadata 行を skip できる設計を後続候補にする

### 出力 file の上書き挙動

- 懸念:
  - `generate_balanced_opening_file(...)` が既存 file を無条件に置き換えるため、誤って前回結果を消す可能性がある
- 推奨案:
  - Step 41 では Step 40 と同じく成功時に target path を置き換える
  - 失敗時は既存 target path を維持する
  - 後続で必要なら `overwrite=True` option を追加する

### progress reporting

- 懸念:
  - `eval_mode="value_search"` は時間がかかるため、無反応に見える
- 推奨案:
  - Step 41 初版では progress callback は追加しない
  - stats で結果を返すことに留める
  - 後続で `progress_callback(stats)` または verbose option を検討する

### resume / incremental generation

- 懸念:
  - 長時間生成が途中で失敗した場合、最初からやり直しになる
- 推奨案:
  - Step 41 初版では atomic write を優先し、partial output / resume は実装しない
  - 後続で必要なら chunked output と resume key を設計する

### prefilter false negative

- 懸念:
  - 静的評価 prefilter が強すぎると、探索すれば balanced だった候補を落とす可能性がある
- 推奨案:
  - `prefilter_threshold=None` を許容し、精度優先なら prefilter なしで探索できるようにする
  - README では prefilter は高速化用であり、候補を落とす可能性があると明記する
  - 使う場合は final threshold より広めの値を推奨する

### device OOM / batch_size

- 懸念:
  - PyTorch batch 評価で `batch_size` が大きすぎると GPU/CPU memory が足りなくなる
- 推奨案:
  - Step 41 でも `batch_size` は利用者指定にする
  - OOM は model 実行例外として伝播させる
  - README では問題が出る場合は `batch_size` を下げると記載する

### model exception の扱い

- 懸念:
  - model forward 中の例外を握りつぶすと、壊れた model や入力 shape 問題に気づきにくい
- 推奨案:
  - model 評価中の例外は原則そのまま伝播させる
  - JSONL は temporary file 経由なので、失敗時に既存 target path は壊さない
  - stats に error を入れて成功扱いにする設計は採用しない

### static prefilter と search result の順序

- 懸念:
  - prefilter 後の探索対象だけを集め直すと、出力順が prefilter 実装依存になる可能性がある
- 推奨案:
  - candidate に元 index を持たせる
  - search 後の accepted entries は元 index 昇順で書き出す
  - 初版で並列化しない場合もこの規約を固定する

### coordinate convention

- 懸念:
  - `a1..h8` の座標系が他ツールと違う場合、sequence の解釈がずれる
- 推奨案:
  - 既存 `Board` の square index と同じ規約を使う
  - `a1` は square `0`、`h8` は square `63` と明記する
  - sequence replay test で固定する

### API name の長さ

- 懸念:
  - `generate_balanced_opening_file(...)` / `random_balanced_opening_board(...)` は長い
- 推奨案:
  - 明確さを優先して現行名を維持する
  - alias は増やさない
  - README の import alias `import veloversi as vv` で実用上の長さを抑える

### `iter_opening_boards(...)` と `BalancedOpeningSet.boards` の使い分け

- 懸念:
  - 全 board だけ欲しい場合、`openings.boards` と `iter_opening_boards(openings)` のどちらを使うべきか迷う
- 推奨案:
  - board だけ必要なら `openings.boards`
  - sequence / value metadata も一緒に処理したいなら `iter_opening_boards(...)`
  - README にこの使い分けを短く書く

### search result の PV 保存

- 懸念:
  - 探索評価を使うなら、その root value に至る PV も保存したくなる
  - ただし JSONL が大きくなり、初版の schema が複雑になる
- 推奨案:
  - Step 41 初版では PV は保存しない
  - 後続で `include_pv=True` option を検討する

### exact search との比較

- 懸念:
  - 終盤に近い `plies` や高 depth では exact search で判定したくなる
- 推奨案:
  - Step 41 は model leaf value search に限定する
  - exact を使う opening 判定は対象外
  - 後続で `eval_mode="exact"` を検討する

### generated count と dedupe の関係

- 懸念:
  - `generated` が dedupe 前なのか dedupe 後なのか曖昧になる
- 推奨案:
  - `generated` は実際に評価候補になった dedupe 後件数とする
  - dedupe 前件数が必要なら後続で `raw_generated` を追加する
  - README / docstring に明記する

### public/private helper の境界

- 懸念:
  - Step 41 実装で private helper が増え、後から public API として依存される可能性がある
- 推奨案:
  - public API は `iter_opening_boards`, `generate_balanced_opening_file`, `load_balanced_opening_file`, `random_balanced_opening_board`, `BalancedOpeningSet` に限定する
  - `_enumerate_opening_candidates` などは private のままにする
  - private helper は README に書かない
