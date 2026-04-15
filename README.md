# veloversi

Veloversi は、Python から使えるオセロ / リバーシライブラリです。

主な用途:

- 盤面操作と合法手生成
- 対局トレースとゲーム記録の生成
- 学習用データの作成
- CNN / flat 入力への変換
- PyTorch 用 DataLoader への接続

## 対応環境

- Python `3.12+`

## インストール

GitHub Release の Assets から、自分の OS / arch に合う wheel を `uv add` で追加します。

`cp312-abi3` は「Python 3.12 以上で共通に使える abi3 wheel」を意味します。  
そのため、Python 3.13 や 3.14 でも別 wheel は不要です。

### Linux x86_64

```bash
uv add "https://github.com/Kotetsu0000/veloversi/releases/download/v0.4.0/veloversi-0.4.0-cp312-abi3-manylinux_2_34_x86_64.whl"
```

### Linux aarch64

```bash
uv add "https://github.com/Kotetsu0000/veloversi/releases/download/v0.4.0/veloversi-0.4.0-cp312-abi3-manylinux_2_34_aarch64.whl"
```

### macOS Intel

```bash
uv add "https://github.com/Kotetsu0000/veloversi/releases/download/v0.4.0/veloversi-0.4.0-cp312-abi3-macosx_10_12_x86_64.whl"
```

### macOS Apple Silicon

```bash
uv add "https://github.com/Kotetsu0000/veloversi/releases/download/v0.4.0/veloversi-0.4.0-cp312-abi3-macosx_11_0_arm64.whl"
```

### Windows x86_64

```powershell
uv add "https://github.com/Kotetsu0000/veloversi/releases/download/v0.4.0/veloversi-0.4.0-cp312-abi3-win_amd64.whl"
```

### sdist

wheel が合わない環境では、Release に含まれる sdist からインストールできます。  
この場合は Rust toolchain が必要です。

```bash
uv add "https://github.com/Kotetsu0000/veloversi/releases/download/v0.4.0/veloversi-0.4.0.tar.gz"
```

## 最小例

```python
import veloversi as vv

board = vv.initial_board()
moves = board.legal_moves_list()
next_board = board.apply_move(moves[0])

print(moves)
print(next_board.board_status())
```

## 盤面 API

基本の盤面 API:

- `initial_board`
- `board_from_bits`
- `validate_board`
- `generate_legal_moves`
- `legal_moves_list`
- `is_legal_move`
- `apply_move`
- `apply_forced_pass`
- `board_status`
- `disc_count`
- `game_result`
- `final_margin_from_black`
- `search_best_move_exact`

補助 API:

- `all_symmetries`
- `transform_board`
- `transform_square`
- `pack_board`
- `unpack_board`

## ランダム対局と学習データ

ランダム対局トレース:

- `play_random_game`
- `sample_reachable_positions`

supervised example:

- `supervised_examples_from_trace`
- `supervised_examples_from_traces`
- `packed_supervised_examples_from_trace`
- `packed_supervised_examples_from_traces`

packed supervised example には、少なくとも次が含まれます。

- packed board
- `final_result`
- `final_margin_from_black`
- `policy_target_index`

`policy_target_index` は次を意味します。

- `-1`: target なし
- `0..63`: 次手のマス
- `64`: pass

## 学習用入力

`Board` method-style:

- `board.transform(sym)`
- `board.encode_planes(history, config)`
- `board.encode_flat_features(history, config)`
- `board.prepare_cnn_model_input()`
- `board.prepare_flat_model_input()`
- `board.prepare_nnue_model_input()`
- `board.search_best_move_exact(timeout_seconds=1.0, worker_count=None, serial_fallback_empty_threshold=18, shared_tt_empty_threshold=20)`
- `board.select_move_with_model(model, depth=1, timeout_seconds=1.0, policy_mode="best", device="cpu", exact_from_empty_threshold=16, always_try_exact=False)`

汎用 feature API:

- `encode_planes`
- `encode_planes_batch`
- `encode_flat_features`
- `encode_flat_features_batch`

学習用 batch API:

- `prepare_planes_learning_batch`
- `prepare_flat_learning_batch`

返り値は `dict` で、少なくとも次を含みます。

- `features`
- `value_targets`
- `policy_targets`
- `legal_move_masks`

shape:

- planes: `(B, C, 8, 8)`
- flat: `(B, F)`
- legal move mask: `(B, 64)`

モデル入力用 API:

- `prepare_cnn_model_input`
- `prepare_cnn_model_input_batch`
- `prepare_flat_model_input`
- `prepare_flat_model_input_batch`
- `prepare_nnue_model_input`

モデル入力 API は `Board` と `RecordedBoard` の両方を受けます。  
`RecordedBoard` を渡した場合は現在局面を使います。

CNN 用入力:

- shape: `(B, 3, 8, 8)`
- channels:
  - 自分の石
  - 相手の石
  - 合法手

flat 用入力:

- shape: `(B, 192)`
- 内訳:
  - 自分の石 64
  - 相手の石 64
  - 合法手 64

NNUE 用入力:

- shape: `(B, 67)` または `(67,)`
- dtype: `int32`
- 内訳:
  - ternary pattern index 64
  - scalar bucket 3

## Recording / Game Record

recording API:

- `random_start_board`
- `start_game_recording`
- `RecordedBoard.apply_move`
- `RecordedBoard.apply_forced_pass`
- `RecordedBoard.transform`
- `RecordedBoard.encode_planes`
- `RecordedBoard.encode_flat_features`
- `RecordedBoard.prepare_cnn_model_input`
- `RecordedBoard.prepare_flat_model_input`
- `RecordedBoard.prepare_nnue_model_input`
- `RecordedBoard.search_best_move_exact`
- `RecordedBoard.select_move_with_model`
- `RecordedBoard.to_dict`
- `RecordedBoard.finish`
- `RecordedBoard.save_record`
- `finish_game_recording`
- `append_game_record`
- `load_game_records`
- `open_game_record_dataset`

recording は immutable で、Python では `RecordedBoard` として扱います。
`RecordedBoard` は現在局面を内部に持ち、`Board` と近い操作感で使えます。
`RecordedBoard.apply_move()` / `RecordedBoard.apply_forced_pass()` は、現在局面の更新に加えて履歴更新も行います。
`Board` にある主要な method は `RecordedBoard` でも同名で使えます。

終盤の exact 探索を行いたい場合:

```python
import veloversi as vv

board = vv.random_start_board(plies=50, seed=123)
result = board.search_best_move_exact(
    timeout_seconds=1.0,
    worker_count=None,
    serial_fallback_empty_threshold=18,
    shared_tt_empty_threshold=20,
)

if result["success"]:
    print(result["best_move"], result["exact_margin"])
else:
    print(result["failure_reason"])
```

`RecordedBoard.search_best_move_exact(...)` も同様に使えます。探索対象は常に `current_board` です。

モデルを使って着手を選びたい場合:

```python
import torch
import veloversi as vv

model = ...  # torch.nn.Module または vv.load_model(...) の返り値
board = vv.initial_board()

result = board.select_move_with_model(
    model,
    depth=1,
    timeout_seconds=1.0,
    policy_mode="best",
    search_mode="fixed",
    device="cpu",
    exact_from_empty_threshold=16,
    always_try_exact=False,
)

if result["success"]:
    print(result["best_move"], result["source"])
else:
    print(result["failure_reason"])
```

PyTorch `nn.Module` を使う経路では `torch` が必要です。Rust value model を使う経路では不要です。

`select_move_with_model(...)` の挙動:

- model は `torch.nn.Module` または `RustValueModel` を受けます
- 入力形式は自動判別します
  - CNN: `(1, 3, 8, 8)`
  - flat: `(1, 192)`
- Rust value model では NNUE 入力 `(1, 67)` を使います
- 出力形式も自動判別します
  - policy: `(64,)` または `(1, 64)`
  - value: scalar / `(1,)` / `(1, 1)`
- Rust value model は value のみです
- `policy_mode="best"`
  - 合法手の最大値を返します
- `policy_mode="sample"`
  - 合法手上の確率分布からサンプリングします
- `search_mode="fixed"`
  - value 出力時に固定深さで探索します
- `search_mode="iterative"`
  - value 出力時に 1..depth の反復深化で探索します
  - timeout 時は最後に完了した深さの結果を返します
- 強制パス局面では model を呼ばず、着手なし結果を返します
- `exact_from_empty_threshold` 以下の終盤では exact-only で動作します
- 制限時間内に exact が成功すれば exact を返します
- exact が timeout / failure した場合は、その失敗結果を返します
- `always_try_exact=True` の場合:
  - `empty_count > exact_from_empty_threshold`
    - exact / model を並列に開始し、先に成功結果を返した側を採用します
  - `empty_count <= exact_from_empty_threshold`
    - `always_try_exact` の値に関係なく exact-only で動作し、model fallback は行いません
- 返り値の `completed_depth`
  - fixed / policy / exact では `None`
  - iterative では最後に完了した深さです
- value 経路の返り値:
  - `raw_value`: model の生出力を使った探索 score
  - `value`: `math.tanh(raw_value)` 済みの `-1..1` の値

## Balanced Opening File

value model で序盤局面を評価し、均衡している局面だけを JSONL に保存できます。

```python
import veloversi as vv

model = vv.load_model("model_weights.vvm")

stats = vv.generate_balanced_opening_file(
    model,
    "balanced-openings.jsonl",
    plies=8,
    threshold=2 / 64,
    value_scale="normalized",
    batch_size=1024,
    dedupe_symmetry=True,
)
print(stats)

openings = vv.load_balanced_opening_file("balanced-openings.jsonl", validate=True)
board = vv.random_balanced_opening_board(openings, seed=123)
```

`value_scale="normalized"` では `tanh(raw_value)` を閾値判定に使います。
`value_scale="raw"` では model raw output をそのまま閾値判定に使います。

## Rust Value Model

PyTorch で学習し、Rust 側で高速推論する value model 導線があります。

学習側:

```python
import torch
import veloversi as vv

model = vv.model.NNUE()
state_dict = torch.load("model_weights.pth", map_location="cpu")
model.load_state_dict(state_dict)
```

変換:

```python
import veloversi as vv

vv.export_model("model_weights.pth", "model_weights.vvm")
```

推論側:

```python
import veloversi as vv

vv_model = vv.load_model("model_weights.vvm")
board = vv.initial_board()

result = board.select_move_with_model(
    vv_model,
    depth=2,
    timeout_seconds=1.0,
    exact_from_empty_threshold=16,
)
```

補足:

- `vv.model.NNUE()` と `vv.export_model(...)` は `torch` が必要です
- `vv.load_model(...)` と Rust value model の推論は `torch` 不要です
- `device` 引数は Rust value model では無視されます

学習側の入口:

```python
import veloversi as vv

model = vv.model.NNUE()

# NNUE 入力は int index の `(B, 67)` を受けます。
# forward() は export/Rust 推論整合のため生値 `(B, 1)` を返します。
# 学習時は通常 `forward_value()` を使います。
value = model.forward_value(batch["board_nnue"])
```

学習用 DataLoader helper は `veloversi.model` ではなく `veloversi.get_dataloader(...)` です。

- `vv.get_dataloader(..., mode="value_only")`
- `vv.get_dataloader(..., mode="policy_only")`
- `vv.get_dataloader(..., mode="policy_value")`

exact 探索の設定:

- `timeout_seconds`
  - 探索制限時間。timeout 超過時は partial result ではなく失敗を返します。
- `worker_count`
  - 並列 worker 数。`None` なら自動設定です。
- `serial_fallback_empty_threshold`
  - この空き数未満では serial fallback を使います。
- `shared_tt_empty_threshold`
  - この空き数以上で shared TT を使います。

最小例:

```python
import veloversi as vv

record = vv.start_game_recording(vv.random_start_board(plies=6, seed=123))

while True:
    status = record.board_status()
    if status == "terminal":
        break
    if status == "forced_pass":
        record = record.apply_forced_pass()
        continue
    record = record.apply_move(record.legal_moves_list()[0])

record.save_record("games.jsonl")
```

進行中 recording を辞書化したい場合:

```python
record_dict = record.to_dict()
```

`to_dict()` は進行中 recording の辞書化です。完成済み game record ではありません。
含まれるのは少なくとも次です。

- `start_board`
- `current_board`
- `moves`

完成 game record を辞書として取得したい場合:

```python
game_record = record.finish()
```

`finish()` は終局局面でのみ成功し、保存用の完成 game record を返します。

game record は JSONL の 1 行 1 試合です。  
各 record は、少なくとも次を持ちます。

- `start_board`
- `moves`
- `final_result`
- `final_black_discs`
- `final_white_discs`
- `final_empty_discs`
- `final_margin_from_black`

`final_result` は次の固定文字列です。

- `black`
- `white`
- `draw`

## RecordDataset

保存済み game record JSONL を、局面 index で引ける dataset API として扱えます。

- `dataset = vv.open_game_record_dataset(path)`
- `dataset = vv.open_game_record_dataset([path1, path2, ...])`
- `len(dataset)`
- `dataset.get(global_index)`
- `dataset.get_cnn_input(global_index)`
- `dataset.get_flat_input(global_index)`
- `dataset.get_targets(global_index)`

注意:

- `len(dataset)` は全手数ではなく、policy target を持つ局面数です
- pass や policy 無効局面は index 対象から除外されます
- 複数ファイルを渡した場合も、`global_index` はその連結集合に対する通し番号です
- JSONL は append-only 前提です。既存 record の並び替えや書き換えをすると `global_index` の対応は変わります

`dataset.get(global_index)` は少なくとも次を返します。

- `board`
- `record_index`
- `ply`
- `global_index`
- `policy_target_index`
- `final_result`
- `final_margin_from_black`

`dataset.get_targets(global_index)` は少なくとも次を返します。

- `value_target`
  - 現在手番視点の最終石差
- `policy_target`
  - shape `(64,)` の one-hot `numpy.ndarray(float32)`

## Examples

実行可能な例は `examples/` にあります。

基本的な盤面操作:

```bash
uv run python examples/basic_usage.py
```

ランダム対局から学習データを生成:

```bash
uv run python examples/generate_training_data.py --output-dir examples/generated_data --num-games 2 --seed 123
```

recording / game record の例:

```bash
uv run python examples/game_recording.py
```

PyTorch DataLoader の例:

```bash
uv run python -m py_compile examples/pytorch_dataloader.py
```

詳細は [examples/README.md](/home/kotetsu0000/program/veloversi/examples/README.md) を参照してください。
