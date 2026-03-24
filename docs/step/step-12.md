# Step 12: serialize API の仕様実装

## このステップの目的

Step 11 で `symmetry` API を実装した。
Step 12 では、[rust_engine_spec.md](/home/kotetsu0000/program/veloversi/specs/rust_engine_spec.md) の `serialize` モジュールを実装し、盤面の固定長シリアライズと復元を Rust / Python の両方で扱えるようにする。

この段階では `random_play` や `feature` までは扱わず、盤面データの安定した受け渡し形式を先に固定することを目的とする。

## このステップで行うこと

- `PackedBoard` を導入する
- `pack_board` を実装する
- `unpack_board` を実装する
- Python では `tuple[int, int, str]` ベースで serialize API を公開する
- Rust / Python の両方に serialize テストを追加する
- `PackedBoard` の具体形を仕様書・計画書で固定する

## 導入対象

- `PackedBoard`
- `pack_board`
- `unpack_board`
- Python 向け serialize 公開関数
- serialize テスト

## このステップの対象範囲

### Rust で追加する対象

- `PackedBoard`
- `pack_board`
- `unpack_board`

### Python で追加する対象

- `pack_board(board: Board) -> tuple[int, int, str]`
- `unpack_board(packed: tuple[int, int, str]) -> Board`

### 定義として固定する事項

- `PackedBoard` は次の公開 struct とする
  - `black_bits: u64`
  - `white_bits: u64`
  - `side_to_move: Color`
- `PackedBoard` には `Copy` / `Clone` / `Eq` / `Debug` を付ける
- `pack_board` は情報を落とさず固定長構造へ写像する
- `unpack_board` は `Board::from_bits` と同じ整合条件で `Board` を復元する
- Python では `PackedBoard` を公開 class にせず、`tuple[int, int, str]` で扱う
- Python の `unpack_board` では長さ違い・型違い・不正文字列を `ValueError` にする
- Python の `unpack_board` は `(int, int, str)` の 3 要素 tuple だけを受け付ける
- Python の `unpack_board` は柔軟変換を行わず、境界を厳しめに固定する

## このステップの対象外

このステップでは次を扱わない。

- `random_play`
- feature
- `engine-search`
- `search_best_move`
- `can_solve_exact`
- `solve_exact`
- WASM 公開 API の本実装

## 受け入れ条件

- [x] `make check` が成功する
- [x] `make coverage-check` が成功する
- [x] `make mutants` を実行し、結果を確認する
- [x] `PackedBoard` が Rust 公開型として実装されている
- [x] `pack_board`、`unpack_board` が Rust 公開 API として実装されている
- [x] Python 側で `pack_board`、`unpack_board` が公開されている
- [x] `pack_board -> unpack_board` の往復で盤面が保たれることを確認するテストがある
- [x] `unpack_board -> pack_board` の往復で packed 表現が保たれることを確認するテストがある
- [x] Python で不正 packed 入力が `ValueError` になることを確認するテストがある

## 実装開始時点の不足

Step 11 時点では、盤面の内部表現と対称変換 API は揃ったが、盤面を固定長で受け渡すための `serialize` API は未実装である。
Python 利用やデータ保存、将来の feature 生成では、`Board` をそのまま持ち回るだけでなく、安定した固定長表現で受け渡せることが重要になる。
このため、Step 12 では `serialize` を独立して実装し、盤面データの入出力基盤を先に固める。

## 実装方針

- `PackedBoard` は圧縮形式ではなく、`Board` と 1 対 1 対応する固定長構造体とする
- `pack_board` は単なる抽出関数とし、追加の正規化は行わない
- `unpack_board` は `Board::from_bits` に寄せて実装し、重なり検証などの整合条件を共通化する
- Python では追加の PyO3 class を増やさず、`tuple[int, int, str]` を入出力に使う
- 不正な Python 入力は `ValueError` へ統一する
- Python tuple は長さ・型・文字列値を厳密に検証する
- `cargo-mutants` は Rust 側 serialize ロジック中心で評価し、PyO3 ラッパ層は pytest で補う

## 段階的な進め方

### Phase 1. 仕様固定

- `PackedBoard` の具体形を Rust / Python で固定する
- Python での tuple 表現を固定する

### Phase 2. Rust API 実装

- `PackedBoard` を追加する
- `pack_board` を実装する
- `unpack_board` を実装する

### Phase 3. Python 公開

- `pack_board`、`unpack_board` を公開する
- Python の tuple 入出力とエラー変換を実装する

### Phase 4. テストと整合確認

- Rust 側に serialize 単体テストを追加する
- Python 側に serialize pytest を追加する
- Rust テストでは初期局面の packed 値を期待値固定で確認する
- 追加局面は packed 値の全列挙より round-trip テストを優先する
- `make check`、`make coverage-check`、`make mutants` を回して結果を確認する

## 採用する構成

### Rust

- `PackedBoard`
- `pack_board`
  - `Board` から固定長構造体へ写像する
- `unpack_board`
  - 固定長構造体から `Board` を復元する

### Python

- `pack_board(board)`
- `unpack_board(packed)`
- 入出力の packed は `(black_bits, white_bits, side_to_move)` の tuple

## 検証項目

### 1. Rust API の正しさ

- 初期局面を `pack_board` した内容が期待値と一致すること
- `pack_board -> unpack_board` で元の盤面へ戻ること
- `unpack_board -> pack_board` で packed 表現が保たれること
- 非合法 packed からの復元が `BoardError` になること

### 2. Python 公開面の整合

- `pack_board` が固定順 tuple を返すこと
- `unpack_board` が Rust と同じ盤面を返すこと
- 長さ違い・型違い・不正文字列で `ValueError` になること
- 不正な packed 入力で `ValueError` になること

## 品質ゲートの扱い

- `make check` は必須とする
- `make coverage-check` は必須とする
- `make mutants` は必須実行とするが、評価は「結果確認」までとする
- `mutants` の残件は、既存 hotpath 起因か serialize 起因かを分けて記録する

## 導入時の注意

- `PackedBoard` のフィールド順は途中で変えない
- Python tuple の順序は将来の保存形式でもそのまま使う前提で固定する
- `pack_board` / `unpack_board` は対称変換や feature とは独立した基礎 API として保つ
- Step 12 対象外の `random_play` や `feature` に着手して計画を広げない

## このステップを先に行う理由

serialize は `engine-core` と `symmetry` の次に自然な拡張であり、Python 受け渡し、データセット保存、将来の feature 生成で再利用しやすい。
ここで `PackedBoard` の形と公開 API を先に固定しておくことで、後続の `random_play` や feature 系を揺れの少ない前提の上で進められる。

## 実装結果

- `src/lib.rs` に `PackedBoard`、`pack_board`、`unpack_board` を追加した
- `PackedBoard` は `black_bits` / `white_bits` / `side_to_move` を持つ軽量公開 struct とした
- Python 公開面として `pack_board(board)` と `unpack_board(packed)` を追加した
- Python の `unpack_board` は `src/veloversi/__init__.py` 側で `(int, int, str)` tuple を厳密検証し、内部 helper `_unpack_board_parts` を呼ぶ形にした
- 仕様書と README を Step 12 の serialize API に合わせて更新した

## 検証結果

- `make check`: 成功
  - Rust: `78 passed; 0 failed; 6 ignored`
  - Python: `18 passed`
- `make coverage-check`: 成功
  - line coverage: `86.12%`
- `make mutants`: 実行・結果確認済み
  - `616 mutants tested in 32m: 114 missed, 401 caught, 86 unviable, 15 timeouts`

## 実装メモ

- Rust unit test と `cargo llvm-cov` では Python ランタイムへの依存を避けるため、serialize の PyO3 wrapper を `cfg(not(any(test, coverage)))` で外している
- Python 公開 API 自体は通常ビルドで維持され、`make check` の pytest で検証している

## mutants 所見

- Step 12 追加分の core serialize ロジックは Rust テストで押さえられている
- 残る `missed` の大半は既存 hotpath と PyO3 ラッパ層に集中している
- serialize 周辺でも `pack_board_py` や `_core` 公開補助の未捕捉は残るが、これは計画どおり pytest 側で補完する
