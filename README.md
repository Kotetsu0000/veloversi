# veloversi

Veloversi は、Rust で書かれたオセロ / リバーシライブラリです。
Python から利用するための拡張モジュールも提供します。

## 必要ツール

- Rust
- Python 3.12
- uv
- cargo-mutants

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
```

`make check` は常設 CI 用です。`make mutants` は手動実行用で、`push` / `pull_request` の CI には含めません。

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
```

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
