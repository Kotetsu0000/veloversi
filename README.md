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
uv add "https://github.com/Kotetsu0000/veloversi/releases/download/v0.2.0/veloversi-0.2.0-cp312-abi3-manylinux_2_34_x86_64.whl"
```

### Linux aarch64

```bash
uv add "https://github.com/Kotetsu0000/veloversi/releases/download/v0.2.0/veloversi-0.2.0-cp312-abi3-manylinux_2_34_aarch64.whl"
```

### macOS Intel

```bash
uv add "https://github.com/Kotetsu0000/veloversi/releases/download/v0.2.0/veloversi-0.2.0-cp312-abi3-macosx_10_12_x86_64.whl"
```

### macOS Apple Silicon

```bash
uv add "https://github.com/Kotetsu0000/veloversi/releases/download/v0.2.0/veloversi-0.2.0-cp312-abi3-macosx_11_0_arm64.whl"
```

### Windows x86_64

```powershell
uv add "https://github.com/Kotetsu0000/veloversi/releases/download/v0.2.0/veloversi-0.2.0-cp312-abi3-win_amd64.whl"
```

### sdist

wheel が合わない環境では、Release に含まれる sdist からインストールできます。  
この場合は Rust toolchain が必要です。

```bash
uv add "https://github.com/Kotetsu0000/veloversi/releases/download/v0.2.0/veloversi-0.2.0.tar.gz"
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

完成 game record を辞書として取得したい場合:

```python
game_record = record.finish()
```

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
- JSONL は append-only 前提です

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
