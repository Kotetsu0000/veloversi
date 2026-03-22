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
cargo mutants --file src/lib.rs
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

配布用の `whl` でも、バイナリ全体を特定 CPU 向けに固定せず、実行時に CPU 機能を見て適切な経路を選ぶ構成にしています。

現在の Perft 実装では、`ref` 配下の参考実装を参照しつつ、合法手生成と反転計算を oriented ビットボード寄りのホットパスへ寄せています。あわせて、`board_status` を経由しない Perft 専用経路、深さ 1 / 2 / 3 の末端特殊化、長時間検証時のルート手単位並列化を入れています。

## Python 拡張モジュールのビルド

```bash
uv run maturin develop
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
cargo mutants --file src/lib.rs
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
