# examples

このディレクトリには、現在の公開 API を確認するための最小実行例を置きます。

## `basic_usage.py`

以下を一通り確認します。

- 初期局面の生成
- 合法手取得
- 着手
- `Board` method-style API
- `pack_board` / `unpack_board`
- `play_random_game`
- `supervised_examples_from_trace`
- `board.transform(...)`
- `board.encode_planes(...)` / `board.encode_flat_features(...)`
- `board.prepare_cnn_model_input()` / `board.prepare_flat_model_input()`
- `sample_reachable_positions`

実行:

```bash
uv run python examples/basic_usage.py
```

## `generate_training_data.py`

`board.legal_moves_list()` の中からランダムに手を選び、policy/value 学習向けの JSONL データを出力します。

- board は packed 形式
- value ラベル
  - `final_result`
  - `final_margin_from_black`
- policy ラベル
  - `policy_target_index`
  - `-1`: target なし
  - `0..63`: 次手のマス
  - `64`: pass

実行:

```bash
uv run python examples/generate_training_data.py --output-dir examples/generated_data --num-games 2 --seed 123
```

## `pytorch_dataloader.py`

保存済み game record JSONL を `RecordDataset` 経由で読み、PyTorch の map-style `Dataset` / `DataLoader` に流す参考例です。

- 1 index = 1 サンプル
- batch 化は `collate_fn`
- `value-only`
- `policy + value`
- CNN 用 `(B, 3, 8, 8)`
- flat 用 `(B, 192)`
- `RecordDataset` は policy 有効局面だけを index 対象にします
- 単一ファイル path と複数ファイル path の両方を受けられます
- `len(dataset)` は全手数ではなく、policy target を持つ局面数です

実行には PyTorch が必要です。リポジトリの標準依存には含めていません。

構文確認:

```bash
uv run python -m py_compile examples/pytorch_dataloader.py
```

PyTorch 導入済み環境での実行例:

```bash
uv run python examples/pytorch_dataloader.py
```

## `game_recording.py`

任意局面から recording を開始し、終局まで手を追加して JSONL に保存する最小例です。

- `random_start_board`
- `start_game_recording`
- `RecordedBoard.apply_move`
- `RecordedBoard.apply_forced_pass`
- `RecordedBoard.finish`
- `RecordedBoard.to_dict`
- `RecordedBoard.save_record`
- `load_game_records`

補足:

- `RecordedBoard.to_dict()` は進行中 recording の辞書化です
- `RecordedBoard.finish()` は完成 game record を返します
- `RecordedBoard.save_record(path)` は完成 game record を JSONL に追記します

実行:

```bash
uv run python examples/game_recording.py
```
