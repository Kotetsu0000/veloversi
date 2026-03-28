# veloversi

Veloversi は、Rust で書かれたオセロ / リバーシライブラリです。
Python から利用するための拡張モジュールも提供します。

## 必要ツール

- Rust
- Python 3.12
- uv
- cargo-mutants
- cargo-llvm-cov

## 初回セットアップ

```bash
uv sync --group dev
uv run maturin develop
```

## 検証コマンド

```bash
make test
make lint
make format
make check
make mutants
make coverage
make coverage-check
make perft-long
make api-bench-rust-legal
make api-bench-rust-apply-unchecked
make api-bench-rust-apply
make api-bench-python
```

`make check` は常設 CI 用です。`make mutants`、`make coverage`、`make coverage-check`、`make perft-long` は手動実行用で、`push` / `pull_request` の CI には含めません。

個別に実行する場合:

```bash
cargo test
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
uv run pytest
uv run ruff check .
uv run ruff format --check .
uv run basedpyright
cargo mutants --file 'src/*.rs' --exclude src/flip_tables.rs -j 8
cargo llvm-cov --html
cargo llvm-cov --fail-under-lines 80
cargo test --release perft_long_initial_position_mode_one_to_depth_fifteen -- --ignored --nocapture
cargo test --release perft_long_initial_position_mode_two_to_depth_fifteen -- --ignored --nocapture
```

`make perft-long` は初期局面の Perft 既知値を深さ 9 から 15 まで確認する長時間検証用コマンドです。`--release` で実行し、ルートの合法手単位で進捗を表示します。

合法手生成と反転計算には runtime dispatch を入れており、`x86_64` かつ `avx2` が利用可能な環境では AVX2 経路を使い、それ以外の環境では generic 実装へ自動でフォールバックします。盤面更新は `x86_64` では既定で SSE2 経路を使います。

SIMD 経路は `VELOVERSI_SIMD` で比較用に強制できます。

- `VELOVERSI_SIMD=auto`: 自動選択
- `VELOVERSI_SIMD=generic`: generic 経路を強制
- `VELOVERSI_SIMD=sse2`: SSE2 盤面更新経路を強制
- `VELOVERSI_SIMD=avx2`: AVX2 合法手生成 + AVX2 反転計算経路を強制

現在の意味は次のとおりです。

- `generic`: movegen=generic / flip=generic / board=generic
- `sse2`: movegen=generic / flip=generic / board=sse2
- `avx2`: movegen=avx2 / flip=avx2 / board=sse2
- `auto`: CPU 機能に応じて自動選択

比較用には次のコマンドを使います。

- `make perft-bench-auto`
- `make perft-bench-generic`
- `make perft-bench-sse2`
- `make perft-bench-avx2`

Python ライブラリとしての実使用速度を確認するには次のコマンドを使います。

- `make api-bench-rust-legal`
- `make api-bench-rust-apply-unchecked`
- `make api-bench-rust-apply`
- `make api-bench-python`

Rust 側ベンチは初期局面の単発 API を大量反復し、Python 側ベンチは同じワークロードを PyO3 経由で測ります。
Step 10 時点の Python 公開面は仕様に合わせて `Board` ベース API に整理しています。
公開するのは `initial_board`、`board_from_bits`、`validate_board`、`generate_legal_moves`、`legal_moves_list`、`is_legal_move`、`apply_move`、`apply_forced_pass`、`board_status`、`disc_count`、`game_result`、`final_margin_from_black` です。
`apply_move_unchecked` と bits helper API は Python 非公開です。

Step 11 では symmetry API を追加しています。
公開するのは `all_symmetries`、`transform_board`、`transform_square` です。
Python では symmetry を次の固定文字列で扱います。

- `identity`
- `rot90`
- `rot180`
- `rot270`
- `flip_horizontal`
- `flip_vertical`
- `flip_diag`
- `flip_anti_diag`

Step 12 では serialize API を追加しています。
公開するのは `pack_board`、`unpack_board` です。
Python では packed 形式を `(black_bits, white_bits, side_to_move)` の tuple で扱います。

Step 13 では random_play API を追加しています。
公開するのは `play_random_game`、`sample_reachable_positions` です。
`play_random_game` はランダム対局トレースを返し、`boards`、`moves`、`final_result`、`final_margin_from_black`、`plies_played`、`reached_terminal` を含みます。
パスは `moves` 内で `None` として表現します。

Step 14 では feature API を追加しています。
公開するのは `encode_planes`、`encode_planes_batch`、`encode_flat_features`、`encode_flat_features_batch` です。
planes は `channels_first` で返り、shape は単一局面で `(C, 8, 8)`、batch で `(B, C, 8, 8)` です。
flat は shape が単一局面で `(F,)`、batch で `(B, F)` です。
どちらも `numpy.ndarray` の `float32` を返し、`history` は新しい順で受け取ります。

Step 21 では supervised example API を追加しています。
公開するのは `supervised_examples_from_trace`、`supervised_examples_from_traces` です。
各 example は `board`、`ply`、`moves_until_here`、`final_result`、`final_margin_from_black` を持ちます。

Step 22 では保存向けの supervised example API を追加しています。
公開するのは `packed_supervised_examples_from_trace`、`packed_supervised_examples_from_traces` です。
各 example は `PackedBoard` 相当の board と、value ラベル (`final_result`, `final_margin_from_black`) に加えて、
policy ラベルとして `policy_target_index` を持ちます。
`policy_target_index` は `-1` が target なし、`0..63` が次手のマス、`64` が pass です。

Step 23 では学習用 batch API を追加しています。
公開するのは `prepare_planes_learning_batch`、`prepare_flat_learning_batch` です。
返り値は `features`、`value_targets`、`policy_targets`、`legal_move_masks` を持つ `dict` です。
shape は planes が `(B, C, 8, 8)`、flat が `(B, F)`、legal move mask が `(B, 64)` です。

Step 25 では recording / game record API を追加しています。
公開するのは `random_start_board`、`start_game_recording`、`record_move`、`record_pass`、`current_board`、`finish_game_recording`、`append_game_record`、`load_game_records` です。
recording は immutable で、Python では `dict` として扱います。
game record は JSONL の 1 行 1 試合で保存し、`start_board`、`moves`、`final_result`、石数、`final_margin_from_black` を含みます。

配布用の `whl` でも、バイナリ全体を特定 CPU 向けに固定せず、実行時に CPU 機能を見て適切な経路を選ぶ構成にしています。

現在の Perft 実装では、`ref` 配下の参考実装を参照しつつ、合法手生成と反転計算を oriented ビットボード寄りのホットパスへ寄せています。あわせて、`board_status` を経由しない Perft 専用経路、深さ 1 / 2 / 3 の末端特殊化、長時間検証時のルート手単位並列化を入れています。

## Python 拡張モジュールのビルド

```bash
uv run maturin develop
```

## Examples

基本的な使い方は `examples/` に置いています。

```bash
uv run python examples/basic_usage.py
```

```bash
uv run python examples/generate_training_data.py --output-dir examples/generated_data --num-games 2 --seed 123
```

```bash
uv run python examples/game_recording.py
```

## Release artifact

バージョンタグ (`v*`) を push すると、GitHub Actions の release workflow が GitHub Release 向け artifact を生成します。

- wheel
  - Linux: `x86_64`, `aarch64`
  - macOS: `x86_64`, `arm64`
  - Windows: `x86_64`
- sdist

`abi3` wheel を使っているため、配布 artifact は OS / arch 中心です。
一方で、workflow 内では Python `3.12`, `3.13`, `3.14` の install / import smoke test を別 matrix で実行します。

## Release からのインストール

GitHub Release の Assets から、自分の OS / arch に合う wheel をダウンロードして `pip` でインストールします。

`cp312-abi3` は「Python 3.12 以上で共通に使える abi3 wheel」を意味します。
そのため、Python 3.13 や 3.14 でも別 wheel は不要です。

### Linux x86_64

```bash
uv add "https://github.com/Kotetsu0000/veloversi/releases/download/v0.0.1/veloversi-0.0.1-cp312-abi3-manylinux_2_34_x86_64.whl"
```

### Linux aarch64

```bash
uv add "https://github.com/Kotetsu0000/veloversi/releases/download/v0.0.1/veloversi-0.0.1-cp312-abi3-manylinux_2_34_aarch64.whl"
```

### macOS Intel

```bash
uv add "https://github.com/Kotetsu0000/veloversi/releases/download/v0.0.1/veloversi-0.0.1-cp312-abi3-macosx_10_12_x86_64.whl"
```

### macOS Apple Silicon

```bash
uv add "https://github.com/Kotetsu0000/veloversi/releases/download/v0.0.1/veloversi-0.0.1-cp312-abi3-macosx_11_0_arm64.whl"
```

### Windows x86_64

```powershell
uv add "https://github.com/Kotetsu0000/veloversi/releases/download/v0.0.1/veloversi-0.0.1-cp312-abi3-win_amd64.whl"
```

### sdist からインストール

wheel が合わない環境では、Release に含まれる `veloversi-0.0.1.tar.gz` からインストールできます。
この場合は Rust toolchain が必要です。

```bash
uv add "https://github.com/Kotetsu0000/veloversi/releases/download/v0.0.1/veloversi-0.0.1.tar.gz"
```

## 一時的な疎通確認テスト

Step 01 では、検証基盤が機能することを確認するための最小テストを Rust / Python に追加しています。
これらは恒久的な仕様テストではなく、後続ステップで実際の仕様テストに置き換えるか削除する前提です。

## cargo-mutants

`cargo-mutants` は Cargo 管理外の開発ツールです。未導入の場合は次で追加します。

```bash
cargo install cargo-mutants
```

実行コマンド:

```bash
cargo mutants --file 'src/*.rs' --exclude src/flip_tables.rs -j 8
```

## cargo-llvm-cov

`cargo-llvm-cov` は Cargo 管理外の開発ツールです。未導入の場合は次で追加します。

```bash
cargo install cargo-llvm-cov
```

実行コマンド:

```bash
cargo llvm-cov --html
```

閾値確認:

```bash
cargo llvm-cov --fail-under-lines 80
```
