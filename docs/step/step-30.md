# Step 30: Game Record Dataset / Index API

## このステップの目的

保存済み game record JSONL を、PyTorch の map-style `Dataset` に載せやすい「局面 index」単位の API として扱えるようにする。

主目的は次の 5 つ。

- game record JSONL を開いて総局面数を取得できるようにする
- 通し番号 `global_index` で 1 局面を取得できるようにする
- `global_index` から CNN / flat 入力を直接取得できるようにする
- `global_index` から value / policy の教師データを直接取得できるようにする
- `random_start_board(...)` 由来の `start_board` でも正しく replay できるようにする

## version 方針

- このステップ中は `0.2.0` を維持する
- このステップは `0.2.x` 系の学習支援拡張として扱う

## version 更新

- `Cargo.toml` と `pyproject.toml` は `0.2.2` に更新した
- README の Release install URL も `v0.2.2` / `veloversi-0.2.2-*` に更新した

## このステップで行うこと

- `RecordDataset` 相当の公開 API を追加する
  - `open_game_record_dataset(path)`
  - `len(dataset)` または `dataset.len()`
  - `dataset.get(global_index)`
  - `dataset.get_cnn_input(global_index)`
  - `dataset.get_flat_input(global_index)`
  - `dataset.get_targets(global_index)`
- `dataset.get(global_index)` の戻り値には少なくとも次を含める
  - `board`
  - `record_index`
  - `ply`
  - `global_index`
  - `policy_target_index`
  - `final_result`
  - `final_margin_from_black`
- `dataset.get_targets(global_index)` の返り値には少なくとも次を含める
  - `value_target`
  - `policy_target`
- `value_target` は現在手番視点の最終石差を返す
- `policy_target` は盤上 64 マスの one-hot `numpy.ndarray` を返す
- `random_start_board(...)` を開始局面に持つ record でも replay が正しく動くようにする
- README / examples に dataset/index API の最小利用例を追加する
- `make check` を通す

## このステップの対象範囲

### 対象

- Python 公開 API
- dataset/index 用の helper
- README
- examples

### 対象外

- 新しい保存形式の導入
- PyTorch runtime そのものの内蔵
- history 付き dataset index 最適化
- mutation quality の追加改善

## 固定した前提

- JSONL は 1 行 1 試合 record のまま使う
- `RecordDataset` は map-style `Dataset` に載せやすいランダムアクセス API を目指す
- replay は常に `record.start_board` を起点に行う
- 標準初期局面を暗黙前提にしない
- `dataset.get(global_index)` が返す `board` は常に `Board` とする
- `value_target` は「現在手番視点の最終石差」
- `policy_target` は 64 次元 one-hot `numpy.ndarray(float32)`
- pass と policy 無効 sample は dataset の index 対象から除外する
- `global_index` は常に policy target を持つ局面だけを指す
- `len(dataset)` は「全手数」ではなく「policy 有効局面数」を返す
- `dataset.get_targets(global_index)` の `policy_target` shape は `(64,)` に固定する
  - PyTorch `DataLoader` の default collate で batch 化すると `(B, 64)` になる前提で扱う
- `policy_target` の dtype は `float32` に固定する
- JSONL は append-only を前提とする
  - 途中行の書き換えや順序変更があると `global_index` の対応は変わる

## 受け入れ条件

- [x] game record JSONL を開いて総局面数を取得できる
- [x] `global_index` で 1 局面を取得できる
- [x] `global_index` で CNN / flat 入力を取得できる
- [x] `global_index` で value / policy 教師データを取得できる
- [x] `random_start_board(...)` 開始の record でも正しく replay できる
- [x] README / examples に最小利用例がある
- [x] `make check` が成功する

## 実装方針

- まずは全 record を読み込み、累積手数 index をメモリ上に持つ
- `global_index -> (record_index, ply)` は累積 index から求める
- 局面復元は `start_board + moves[:ply]` の replay で行う
- `get_cnn_input` / `get_flat_input` は既存 model input API を再利用する
- `get_targets` は
  - value: 現在手番視点へ符号を合わせる
  - policy: 64 次元 one-hot
  を返す
- pass / policy 無効 sample は index 構築時に除外する
- `dataset.get(global_index)` の戻り値は最小限に保つ
  - `board`
  - `record_index`
  - `ply`
  - `global_index`
  - `policy_target_index`
  - `final_result`
  - `final_margin_from_black`
- `random_start_board(...)` 起点の replay テストを厚く入れる
  - 固定ケースに加えて、少なくとも 32 seed 分の `random_start_board` 記録で復元一致を確認する

## 懸念点

- 毎回 replay するのでランダムアクセスは重い
  - Step 30 は正しさ優先とし、checkpoint / cache は後続に回す
- 総局面数は「全手数」ではなく「policy 有効局面数」になる
  - README と examples で明記する
- file 全体の index 構築時に JSONL が壊れていると開けない
  - 早めに検証して error を返す
- `random_start_board(...)` 開始 record の replay はバグが入りやすい
  - 固定ケース + 複数 seed の定量テストで確認する
- append-only 前提を破ると `global_index` の安定性が失われる
  - README と docstring で明記する

## このステップを先に行う理由

Step 25 で game record 保存が入り、Step 26 で学習入力 helper が揃った。
しかし、現在は JSONL を試合単位でしか扱えず、PyTorch の map-style `Dataset` にそのまま乗せるには毎回利用側で index と replay を実装する必要がある。
局面 index API を追加すれば、保存済みデータを直接 `__len__` / `__getitem__` 相当で扱えるようになり、学習コードの実装コストを大きく下げられる。

## 実装結果

- Python 公開 API に `RecordDataset` と `open_game_record_dataset(path)` を追加した
- `RecordDataset` は次を提供する
  - `len(dataset)` / `dataset.len()`
  - `dataset.get(global_index)`
  - `dataset.get_cnn_input(global_index)`
  - `dataset.get_flat_input(global_index)`
  - `dataset.get_targets(global_index)`
- `global_index` は policy 有効局面だけを index 対象とする
  - pass と policy 無効 sample は除外する
- replay は常に `start_board` を起点に行う
  - `random_start_board(...)` 開始 record でも復元できる
- README と `examples/pytorch_dataloader.py` を `RecordDataset` 前提に更新した

## 検証結果

- `uv run pytest -q src/test_python_api.py`: 成功
  - `54 passed`
- `uv run python -m py_compile examples/pytorch_dataloader.py`: 成功
- `make check`: 成功
- `make coverage-check`: 成功
  - line coverage `90.58%`
