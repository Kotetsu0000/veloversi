# examples

このディレクトリには、現在の公開 API を確認するための最小実行例を置きます。

## `basic_usage.py`

以下を一通り確認します。

- 初期局面の生成
- 合法手取得
- 着手
- `pack_board` / `unpack_board`
- `play_random_game`
- `supervised_examples_from_trace`
- `encode_planes` / `encode_flat_features`
- `sample_reachable_positions`

実行:

```bash
uv run python examples/basic_usage.py
```

## `generate_training_data.py`

`legal_moves_list(board)` の中からランダムに手を選び、policy/value 学習向けの JSONL データを出力します。

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

保存済み JSONL ディレクトリを読み、`prepare_planes_learning_batch` を使って PyTorch `Dataset` / `DataLoader` に流す参考例です。

実行には PyTorch が必要です。リポジトリの標準依存には含めていません。

構文確認:

```bash
uv run python -m py_compile examples/pytorch_dataloader.py
```
