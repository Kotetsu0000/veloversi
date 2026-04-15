# Step 40: Model-based Balanced Opening File Generation

## このステップの目的

model で評価した序盤局面から、均衡している開始局面集合をファイルとして生成し、読み込んで利用できるようにする。

具体的には、

- `select_move_with_model(...)` の返却 `value` を normalized value として整理する
- 8 手を既定にした合法手順を列挙する
- model で局面を評価し、閾値以内の局面だけを balanced opening file として保存する
- 保存した opening file を読み込み、seed 付きでランダムに開始局面を選べる API を追加する

を実装する。

## 背景

当初は XOT のような開始局面集合を扱う想定だった。
XOT の公式リストは次のページから参照できる。

- 解説: https://berg.earthlingz.de/xot/aboutxot.php
- small list: https://berg.earthlingz.de/xot/downloads/openingssmall.txt
- large list: https://berg.earthlingz.de/xot/downloads/openingslarge.txt

解説ページによると、XOT は Edax / Ntest で評価された 8 手オープニング列のリストである。

- small list: 3,623 sequences, 評価 -1 から +1
- large list: 10,784 sequences, 評価 -2 から +2

ただし、2026-04-15 時点で確認した解説ページには明示ライセンスが見当たらない。
また、この step の目的は公式 XOT リストそのものではなく、model で評価した均衡序盤局面を生成・利用することである。

そのため、API 名には XOT を使わず、`balanced_opening` として扱う。

## version 方針

- Step 40 は `0.3.x` の後続 feature として扱う
- 既存ユーザー互換は強く意識しすぎない
- ただし raw value は `raw_value` として確認できるように残す

## このステップで行うこと

### Phase 1: `select_move_with_model(...)` の value 返却規約

現状の PyTorch `NNUE.forward()` は Rust export / 推論整合のため raw value を返す。

```python
def forward_value_raw(self, x):
    return self.fc2(hidden)

def forward_value(self, x):
    return torch.tanh(self.forward_value_raw(x))

def forward(self, x):
    return self.forward_value_raw(x)
```

また、Rust 側 `NnueValueModel::predict_board(...)` も raw output を返す。

この step では、`select_move_with_model(...)` の返却 dict における value 規約を次のように固定する。

- `raw_value`: model の生出力、または value 探索内部で得た raw score
- `value`: `tanh(raw_value)` 済みの normalized value

探索内部では raw value をそのまま使う。
返却 dict を作る最後の段階でだけ `tanh(...)` を適用する。

これにより、

- 探索時の大小比較は既存挙動を維持する
- public return の `value` は常に `-1..1` に収まる
- 64 倍したときに石差相当として解釈しやすい
- raw が必要な利用者は `raw_value` を見られる

ようにする。

policy 出力経路や exact 経路の規約は維持する。
exact 経路では既存どおり exact margin 由来の normalized value を返す。
返却 schema を安定させるため、policy / exact / forced_pass / failure の各経路でも `raw_value` key は含め、model raw が存在しない場合は `None` とする。

### Phase 2: 8 手合法手順の列挙

標準初期局面から `plies` 手の合法手順を列挙する helper を追加する。

既定値は `plies=8` とする。

- 各手順の着手座標列を `f5d6c4d3c2b3b4b5` のような文字列で保持する
- 各手順の最終局面を `Board` として保持する
- 強制パスが発生した場合は board は進めるが、sequence には pass を含めない
- 必要なら symmetry 正規化で重複局面を落とす

`dedupe_symmetry=True` の場合は、board bits と side-to-move を含む正規化 key で重複を除く。
代表 sequence は列挙順で最初に出たものを採用し、結果を deterministic にする。

### Phase 3: balanced opening file 生成 API

Python public API に次を追加する。

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
) -> dict
```

挙動:

- `plies` 手の合法手順を列挙する
- 生成開始時に PyTorch model の input format を 1 回だけ検出する
- 検出した input format を固定して batch 推論する
- `RustValueModel` は初版では `batch_size` を受け取りつつ、内部では 1 局面ずつ評価してよい
- policy 出力 model は均衡判定に使えないため `ValueError` とする
- PyTorch batch value 出力として許容する shape は `(B,)` / `(B, 1)` に固定する
- `(B, 64)` は policy 出力として扱い、balanced opening 生成では拒否する
- 各局面の `raw_value` と `normalized_value = tanh(raw_value)` を計算する
- `value_scale="normalized"` の場合は `normalized_value` を閾値判定に使う
- `value_scale="raw"` の場合は `raw_value` を閾値判定に使う
- `abs(filter_value) <= threshold` の局面だけ保存する
- `batch_size` は positive int 必須とし、それ以外は `ValueError` とする
- `plies` は `0 <= plies <= 60` の int とし、それ以外は `ValueError` とする
- `threshold` は finite かつ `>= 0` の数値とし、それ以外は `ValueError` とする
- `raw_value` / `normalized_value` / `filter_value` が finite でない局面は保存せず、stats の `skipped_non_finite` に数える
- 生成時は target path と同じ directory に `tempfile.NamedTemporaryFile(delete=False)` で temporary file を作り、成功後に `Path.replace(...)` で target path へ置き換える
- 失敗時は temporary file を best-effort で削除し、既存 target path は更新しない
- target path の親 directory は自動作成せず、存在しない場合は通常の file IO error に任せる
- 閾値が厳しく `accepted=0` でも生成関数は成功とし、空の JSONL と stats を返す

`value_scale` の許容値は `"normalized"` / `"raw"` に固定する。
既定値は `"normalized"` とする。
`value_scale="raw"` の場合、`threshold` は model raw output 単位として扱う。

戻り値には少なくとも次を含める。

- `generated`
- `accepted`
- `path`
- `plies`
- `threshold`
- `value_scale`
- `batch_size`
- `dedupe_symmetry`
- `skipped_non_finite`

### Phase 4: balanced opening file 形式

ファイル形式は JSONL とする。

各行は次の情報を持つ。

```json
{"sequence":"f5d6c4d3c2b3b4b5","black_bits":123,"white_bits":456,"side_to_move":"black","raw_value":0.42,"normalized_value":0.39693043,"filter_value":0.39693043}
```

方針:

- `sequence` は人間が確認しやすい元手順として保存する
- `black_bits` / `white_bits` / `side_to_move` は高速復元用に保存する
- `raw_value` は model 生出力
- `normalized_value` は `tanh(raw_value)`
- `filter_value` は threshold 判定に実際に使った値
- 初版では per-row schema を最小限に保つ

### Phase 5: 読み込みとランダム選択 API

Python public API に次を追加する。

```python
load_balanced_opening_file(path, *, validate=True) -> BalancedOpeningSet
random_balanced_opening_board(path_or_set, seed: int) -> Board
```

`BalancedOpeningSet` は public Python 型として追加する。

- `len(openings)` で件数を返す
- `openings.entries` で entry 一覧を参照できる
- `openings.boards` で `Board` 一覧を参照できる
- 初版では全件をメモリに載せる

挙動:

- JSONL を読み込む
- 各行から `Board` を復元する
- `validate=True` の場合は sequence replay 結果と bits 復元結果が一致することを確認する
- `validate=False` の場合は bits から復元するだけにして高速に読む
- 不正行は `ValueError` にする
- 空ファイルは空の `BalancedOpeningSet` として読み込む
- `random_balanced_opening_board(...)` は seed で再現可能に 1 局面を返す
- 同じ seed と同じ opening set なら同じ盤面を返す
- 空の opening set に対する `random_balanced_opening_board(...)` は `ValueError` とする
- `path_or_set` に `str` / `Path` が渡された場合は内部で `load_balanced_opening_file(..., validate=True)` する
- 実用上は、繰り返し利用では `load_balanced_opening_file(...)` で読み込んだ `BalancedOpeningSet` を渡すことを推奨する
- `seed` は `0 <= seed <= 0xFFFF_FFFF_FFFF_FFFF` の int とし、それ以外は `ValueError` とする
- random selection は初版では Python `random.Random(seed)` を使い、同じ library version での再現性を固定する

### Phase 6: 文書とテスト

- README に balanced opening file 生成例を追加する
- Python stub を更新する
- docstring を追加する
- `src/test_python_api.py` にテストを追加する
- `make check` を通す

## 対象範囲

### 対象

- `src/veloversi/__init__.py`
- `src/veloversi/__init__.pyi`
- `src/veloversi/_core.pyi`
- `src/test_python_api.py`
- `README.md`
- `docs/step/step-40.md`

### 対象外

- 公式 XOT リストの repo 同梱
- XOT 公式リストのライセンス判断
- Rust core への balanced opening set 実装
- WASM API への公開
- model 学習ロジックの変更
- exact solver / midgame search のアルゴリズム変更

## 固定した前提

- 公式 XOT リストは同梱しない
- API 名に XOT は使わない
- 均衡序盤局面集合は、ユーザーが渡した model で生成する
- 生成対象は標準初期局面から `plies` 手の合法手順とする
- `plies=8` を既定値にする
- board file は JSONL とする
- `select_move_with_model(...)` の返却 `value` は normalized value とする
- raw 出力は `raw_value` として別に扱う
- value 経路以外でも返却 schema 安定のため `raw_value` key を含め、該当しない場合は `None` とする
- balanced opening の閾値判定は既定で normalized value を使う
- `threshold=2 / 64` を既定値にする
- `threshold=1 / 64` で XOT small 相当の厳しさを表現できる
- `value_scale="raw"` も初版からサポートする
- `value_scale="raw"` の threshold は raw output 単位として扱う
- `batch_size` は初版から引数に含める
- `batch_size` は positive int に固定する
- `plies` は `0 <= plies <= 60` の int に固定する
- `threshold` は finite かつ `>= 0` の数値に固定する
- PyTorch model の input format 検出は生成開始時に 1 回だけ行う
- balanced opening 生成は value model 専用とし、policy model は拒否する
- PyTorch batch value 出力 shape は `(B,)` / `(B, 1)` のみ許容する
- JSONL 書き出しは temporary file から rename する
- temporary file は `tempfile.NamedTemporaryFile(delete=False)` で target path と同じ directory に作る
- target path の親 directory は自動作成しない
- `accepted=0` は生成成功として扱う
- non-finite value は保存せず `skipped_non_finite` に数える
- `BalancedOpeningSet` は public 型として追加する
- `seed` は `u64` 範囲の int に固定する
- random selection は初版では Python `random.Random(seed)` を使う
- 読み込みとランダム選択は Python API から始める

## 受け入れ条件

- [ ] `select_move_with_model(...)` の value 返却規約が整理されている
- [ ] value 経路の返却 dict に normalized `value` が入る
- [ ] value 経路の返却 dict に `raw_value` が入る
- [ ] value 経路以外の返却 dict に `raw_value=None` が入る
- [ ] PyTorch model と `RustValueModel` の返却 value scale が一貫している
- [ ] `plies` 手合法手順を列挙できる
- [ ] `generate_balanced_opening_file(...)` が JSONL を出力できる
- [ ] `batch_size` を指定できる
- [ ] `batch_size` が positive int でない場合に `ValueError` になる
- [ ] `plies` が `0 <= plies <= 60` の int でない場合に `ValueError` になる
- [ ] `threshold` が finite かつ `>= 0` でない場合に `ValueError` になる
- [ ] PyTorch model の input format 検出が生成開始時 1 回に固定されている
- [ ] policy 出力 model は `generate_balanced_opening_file(...)` で `ValueError` になる
- [ ] PyTorch batch value 出力は `(B,)` / `(B, 1)` を受け付け、それ以外の value 不明 shape を拒否する
- [ ] `value_scale="normalized"` と `"raw"` の両方を扱える
- [ ] non-finite value の局面は保存されず、`skipped_non_finite` に数えられる
- [ ] JSONL 書き出しが temporary file 経由で atomic に行われる
- [ ] target path の親 directory が存在しない場合は自動作成せず file IO error になる
- [ ] `accepted=0` でも生成関数が成功し、空 JSONL と stats を返す
- [ ] JSONL には `sequence`, `black_bits`, `white_bits`, `side_to_move`, `raw_value`, `normalized_value`, `filter_value` が保存される
- [ ] `BalancedOpeningSet` が public 型として提供される
- [ ] `len(openings)`, `openings.entries`, `openings.boards` が使える
- [ ] `load_balanced_opening_file(..., validate=True)` で sequence replay 検証ができる
- [ ] `load_balanced_opening_file(..., validate=False)` で replay なしに読み込める
- [ ] 空 JSONL は空の `BalancedOpeningSet` として読み込める
- [ ] `random_balanced_opening_board(...)` が seed 付きで再現可能に盤面を返す
- [ ] `seed` が `u64` 範囲の int でない場合に `ValueError` になる
- [ ] 空の `BalancedOpeningSet` に対する `random_balanced_opening_board(...)` は `ValueError` になる
- [ ] 返る盤面が `validate` を満たす
- [ ] README / docstring / stub が更新されている
- [ ] `make check` が成功する

## 懸念点

### raw value と normalized value の混同

- 懸念:
  - model 生出力と `tanh(raw)` 済み値を混同すると、threshold 判定や表示値が分かりにくくなる
- 解決策:
  - `raw_value`, `normalized_value`, `filter_value` を明示的に分ける
  - `select_move_with_model(...)` の返却 dict でも `value` と `raw_value` を分ける

### 探索内部の値と返却値が異なる

- 懸念:
  - 探索内部は raw、返却は normalized なので、実装箇所を間違えると探索結果が変わる
- 解決策:
  - `evaluate_position` は raw を返す
  - negamax / ordering / best value 更新は raw のまま行う
  - `_success_result(...)` へ渡す最後の段階でだけ `tanh(raw_value)` を計算する

### RustValueModel の batch 評価

- 懸念:
  - `RustValueModel.evaluate_board(...)` は初版では 1 局面ずつの評価になる
- 解決策:
  - API には `batch_size` を含める
  - PyTorch は初版から batch 推論する
  - RustValueModel は初版では内部ループ評価でよい
  - 必要なら後続で Rust batch API を追加する

### 8 手全列挙の件数と実行時間

- 懸念:
  - 全合法手順の列挙と model 評価に時間がかかる
- 解決策:
  - batch 推論を初版から入れる
  - 戻り値に `generated` / `accepted` を含め、結果を確認しやすくする
  - `plies` を引数化し、検証時は小さい値で実行できるようにする

### symmetry dedupe の定義

- 懸念:
  - 手順は違うが対称な同一局面をどこまで同じとみなすかで件数が変わる
- 解決策:
  - `dedupe_symmetry=True` を既定にする
  - 正規化 key は board bits と side-to-move を含める
  - 代表 sequence は列挙順で最初のものを採用する
  - `False` も選べるようにして検証しやすくする

### 公式 XOT との差異

- 懸念:
  - model で評価するため、公式 XOT small / large と同じ集合にはならない
- 解決策:
  - API 名から XOT を外す
  - 「model-filtered balanced openings」として文書化する
  - 閾値の既定値だけ XOT の small / large に寄せる

### JSONL format の将来互換

- 懸念:
  - 後続で metadata や生成条件を保存したくなる
- 解決策:
  - 最小限の per-row schema で始める
  - 将来、先頭 metadata 行を導入する場合は `type` field などで区別できるようにする

### policy model を渡した場合の扱い

- 懸念:
  - balanced opening 生成には局面 value が必要だが、policy 出力 model では均衡判定ができない
- 解決策:
  - `generate_balanced_opening_file(...)` は value model 専用にする
  - policy 出力を検出したら `ValueError` とする

### batch output shape の曖昧さ

- 懸念:
  - 既存の単一局面 helper とは異なり、batch 推論では output shape の解釈が曖昧になりやすい
- 解決策:
  - batch value 出力は `(B,)` / `(B, 1)` のみ許容する
  - `(B, 64)` は policy とみなして balanced opening 生成では拒否する
  - その他の shape は `ValueError` とする

### 生成途中で壊れた JSONL が残る

- 懸念:
  - 生成中の例外や中断で target path に不完全な JSONL が残る可能性がある
- 解決策:
  - temporary file に書き出し、成功後に target path へ rename する
  - 失敗時は target path を更新しない

### accepted が 0 件の場合

- 懸念:
  - threshold が厳しすぎると採用局面が 0 件になる
- 解決策:
  - `generate_balanced_opening_file(...)` は `accepted=0` でも成功とする
  - `load_balanced_opening_file(...)` は空 set を返す
  - `random_balanced_opening_board(...)` は空 set に対して `ValueError` とする

### non-finite value の扱い

- 懸念:
  - model 出力が `nan` / `inf` の場合、JSONL や threshold 判定が壊れる
- 解決策:
  - `raw_value`, `normalized_value`, `filter_value` が finite でない局面は保存しない
  - stats に `skipped_non_finite` を含める

### threshold / plies / seed validation

- 懸念:
  - 不正な数値を許すと生成範囲やランダム選択が曖昧になる
- 解決策:
  - `threshold` は finite かつ `>= 0`
  - `plies` は `0 <= plies <= 60` の int
  - `seed` は `0 <= seed <= 0xFFFF_FFFF_FFFF_FFFF` の int
  - いずれも不正なら `ValueError`

### raw threshold の単位

- 懸念:
  - `value_scale="raw"` の threshold は normalized value と単位が異なる
- 解決策:
  - `value_scale="raw"` は model raw output 単位として明記する
  - 既定値は `value_scale="normalized"` のままにする

### temporary file の衝突

- 懸念:
  - 固定名の temporary file では並列実行時に衝突する
- 解決策:
  - target path と同じ directory に `tempfile.NamedTemporaryFile(delete=False)` で一意な temporary file を作る
  - 成功時は `Path.replace(...)` で置き換える
  - 失敗時は best-effort で temporary file を削除する

### BalancedOpeningSet の公開形

- 懸念:
  - dict/list ベースで返すと後続で型を変えにくい
- 解決策:
  - 初版から public `BalancedOpeningSet` 型を追加する
  - `len(openings)`, `openings.entries`, `openings.boards` を提供する
  - 8 手 opening 規模では全件メモリ保持で十分とする

### random selection の再現性

- 懸念:
  - Python RNG と Rust RNG が一致しない
- 解決策:
  - 初版では Python `random.Random(seed)` で同じ library version 内の再現性を固定する
  - Rust 側 API と一致させる必要が出たら後続で専用 RNG を公開する
