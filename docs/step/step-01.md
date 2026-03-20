# Step 01: 検証基盤の整備

## このステップの目的

実装を進める前に、正しさを継続的に確認できる環境を先に整える。
この段階ではゲームロジックの実装には入らず、Rust と Python の両面でテストと静的解析を実行できる状態を作ることを目的とする。

## このステップで行うこと

- Rust のユニットテストを実行できる構成を整える
- Python 側からの動作確認テストを実行できる構成を整える
- フォーマッタと Linter を実行できる構成を整える
- ミューテーションテストを導入できる構成を整える
- Rust クレートをテストしやすい形にするため、`crate-type` に `rlib` を含める
- Python 3.12 方針に合わせて ABI 設定を揃える
- `Makefile` を用意し、開発者が同じ手順で検証できるように実行コマンドを統一する
- README に環境構築手順と検証コマンドを記載する
- 検証基盤の疎通確認を目的とした一時的な最小テストを用意する

## 導入対象

- Rust テスト: `cargo test`
- Rust フォーマット: `cargo fmt`
- Rust Linter: `cargo clippy`
- Rust ミューテーションテスト: `cargo-mutants`
- Python テスト: `pytest`
- Python Linter / Formatter: `ruff`
- Python 型チェック: `basedpyright`
- Python パッケージビルド確認: `maturin develop` または `maturin build`
- タスク実行の統一: `Makefile`
- CI: GitHub Actions

## 受け入れ条件

- [ ] `Cargo.toml` の `crate-type` に `cdylib` と `rlib` が含まれている
- [ ] Python 3.12 方針と一致する ABI 設定になっている
- [ ] Python 開発依存が `uv` 管理下に追加され、`uv.lock` に反映されている
- [ ] Rust 依存が `Cargo.lock` に反映されている
- [ ] `cargo test` が成功する
- [ ] `pytest` が成功する
- [ ] `cargo fmt --check` が成功する
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` が成功する
- [ ] `ruff check` が成功する
- [ ] `ruff format --check` が成功する
- [ ] `basedpyright` が成功する
- [ ] `cargo-mutants --file src/lib.rs` が実行できる
- [ ] `Makefile` に少なくとも `test`, `lint`, `format`, `check`, `mutants` が定義されている
- [ ] CI で上記の常設チェックを再現できる
- [ ] `README.md` に初回セットアップ手順と主要な検証コマンドが記載されている
- [ ] `README.md` に `cargo-mutants` の導入手順または使用方法が記載されている
- [ ] Rust 側に検証基盤の疎通確認を目的とした一時的な最小テストが 1 件以上あり、実行成功する
- [ ] Python 側に検証基盤の疎通確認を目的とした一時的な最小テストが 1 件以上あり、実行成功する

## 現時点の不足

現状のリポジトリには、最低限の `pyo3` と `maturin` の設定はあるが、以下が未整備である。

- Rust テストモジュールまたは `tests/` ディレクトリ
- Python テスト用の `tests/` ディレクトリ
- `pytest` などの開発依存関係
- `ruff` や型チェックの設定
- `basedpyright` の設定
- ミューテーションテスト実行手順
- Rust の `crate-type` における `rlib` 対応
- Python バージョン方針と ABI 設定の整合
- `Makefile`
- CI 定義
- README 上の環境構築手順

## 実装方針

- 最初に「何をどう実行すれば品質確認できるか」を固定する
- Rust 側は `cargo` 標準の流れに寄せ、特殊なランナー依存を避ける
- Rust クレートは `cdylib` と `rlib` を併用する
- Python 側は `uv` 管理を前提に開発依存を追加する
- Python の型チェックは `basedpyright` に統一する
- Python の最低対応バージョンは 3.12 とし、ABI 設定もそれに揃える
- Python 側の開発依存は `uv.lock` で固定する
- Rust 側の依存は `Cargo.lock` で固定する
- `cargo-mutants` は README または CI で導入方法を明示する
- ローカル実行の入口は `Makefile` に統一する
- Linter と Formatter は CI で必ず落とせるものに限定する
- ミューテーションテストは常時フル実行ではなく、対象モジュール限定で始める
- 実装初期は過度に重い品質ゲートを避け、段階的に強化する
- README は「初回セットアップ」「日常的な検証コマンド」「トラブル時の再構築手順」を最小限で記載する
- Step 01 の疎通確認テストは一時的なものであり、後続の仕様テスト整備後に置き換えまたは削除してよいことを明記する

## 型チェックツールの選定

Python の型チェックは `basedpyright` を採用する。

採用理由:

- `pyright` は npm 配布が中心で、CLI 利用に Node.js 前提になりやすい
- `basedpyright` は PyPI から導入でき、Python/uv 中心の開発環境に合わせやすい
- `basedpyright` は `pyright` のフォークで、上流追従を継続している
- `basedpyright` は追加の診断ルールと stricter な既定値を持ち、初期段階で品質ゲートを作りやすい
- エディタと CLI のバージョン差異を減らしやすい

## 採用する構成

### Rust

- `crate-type = ["cdylib", "rlib"]`
  - Python 拡張モジュールと Rust テスト容易性を両立する
- `cargo test`
  - コアロジックの単体検証の主軸にする
- `cargo fmt`
  - 書式統一
- `cargo clippy`
  - Rust 実装の静的解析
- `cargo-mutants`
  - テストの有効性確認

### Python

- Python 最低対応バージョン: 3.12
- PyO3 ABI 設定: Python 3.12 に揃える
- `pytest`
  - Python 公開 API の振る舞い確認
- `ruff`
  - Lint と format を 1 つに集約する
- `basedpyright`
  - Python API の型整合確認
- 開発依存管理: `uv` と `uv.lock`

### 運用

- `Makefile`
  - `test`, `lint`, `format`, `check`, `mutants` などの入口を統一する
- `.github/workflows/ci.yml`
  - PR ごとに最低限の検証を自動実行する
- `README.md`
  - セットアップ手順、必要ツール、主要コマンドを記載する

## README に記載する内容

- 必要ツール: Rust, Python 3.12, uv, maturin, `cargo-mutants`
- 初回セットアップ手順
- Python 拡張モジュールのビルド手順
- `cargo test`, `pytest`, `ruff`, `basedpyright`, `cargo clippy` の実行方法
- `make test`, `make lint`, `make format`, `make check`, `make mutants` の実行方法
- `cargo-mutants` の導入手順または実行方法
- Step 01 で導入する一時的な疎通確認テストの位置づけ

## 検証項目

### 1. Rust 検証

- `cargo test` が成功すること
- `cargo fmt --check` が成功すること
- `cargo clippy --all-targets --all-features -- -D warnings` が成功すること
- 一時的な最小テストが実際に実行されること
- `rlib` を含む構成でテストが実行できること

### 2. Python 検証

- `maturin develop` で Python から拡張モジュールを読み込めること
- `pytest` が成功すること
- `ruff check` と `ruff format --check` が成功すること
- `basedpyright` が成功すること
- 一時的な最小テストが実際に実行されること
- Python 3.12 方針と ABI 設定が一致していること

### 3. ミューテーションテスト検証

- `cargo-mutants --file src/lib.rs` が実行可能であること
- 実行結果をレビュー可能な形で残せること

## 導入時の注意

- `cargo-mutants` は実行時間が長くなりやすいため、CI 常時実行ではなく手動または限定対象で始める
- Python 側の型チェックは、公開 API の境界に対象を絞る
- Linter のルールは初回から厳格化しすぎず、警告よりも失敗条件を明確にする
- 一時的な最小テストは恒久的な仕様テストではないため、後続ステップで実テストに置き換える前提で扱う

## このステップを先に行う理由

「実装が正しいかを確認できる」「問題ない実装である検証が可能」という区切りを成立させるには、各ステップの前提として検証基盤が必要になる。
そのため最初に品質確認の入口を整え、以後のステップではロジック実装そのものの妥当性だけに集中できる状態を作る。
