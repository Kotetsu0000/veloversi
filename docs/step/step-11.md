# Step 11: symmetry API の仕様実装

## このステップの目的

Step 10 で `engine-core` と Python 基本 API の仕様一致を進めた。
Step 11 では、[rust_engine_spec.md](/home/kotetsu0000/program/veloversi/specs/rust_engine_spec.md) の `symmetry` モジュールを実装し、盤面対称変換とマス変換を Rust / Python の両方で扱えるようにする。

この段階では feature や search までは扱わず、対称変換の定義と公開 API を安定化させることを目的とする。

## このステップで行うこと

- `Symmetry` enum を導入する
- `transform_board` を実装する
- `transform_square` を実装する
- `all_symmetries` を実装する
- Python では文字列ベースで symmetry API を公開する
- Rust / Python の両方に symmetry テストを追加する
- 対称変換の定義と順序を仕様書・計画書で固定する

## 導入対象

- `Symmetry` enum
- `transform_board`
- `transform_square`
- `all_symmetries`
- Python 向け symmetry 公開関数
- symmetry テスト

## このステップの対象範囲

### Rust で追加する対象

- `Symmetry`
- `transform_board`
- `transform_square`
- `all_symmetries`

### Python で追加する対象

- `transform_board(board: Board, sym: str) -> Board`
- `transform_square(square: int, sym: str) -> int`
- `all_symmetries() -> list[str]`

### 定義として固定する事項

- `Symmetry` は次の 8 要素 enum とする
  - `Identity`
  - `Rot90`
  - `Rot180`
  - `Rot270`
  - `FlipHorizontal`
  - `FlipVertical`
  - `FlipDiag`
  - `FlipAntiDiag`
- Python では次の文字列名を使う
  - `"identity"`
  - `"rot90"`
  - `"rot180"`
  - `"rot270"`
  - `"flip_horizontal"`
  - `"flip_vertical"`
  - `"flip_diag"`
  - `"flip_anti_diag"`
- `all_symmetries()` は上記の順で返す
- `transform_board` / `transform_square` は純関数として扱う
- 未知の対称名は Python では `ValueError` とする

## このステップの対象外

このステップでは次を扱わない。

- feature
- serialize
- random_play
- `engine-search`
- `search_best_move`
- `can_solve_exact`
- `solve_exact`
- WASM 公開 API の本実装

## 受け入れ条件

- [x] `make check` が成功する
- [x] `make coverage-check` が成功する
- [x] `make mutants` を実行し、結果を確認する
- [x] `Symmetry` enum が Rust に実装されている
- [x] `transform_board`、`transform_square`、`all_symmetries` が Rust 公開 API として実装されている
- [x] Python 側で `transform_board`、`transform_square`、`all_symmetries` が公開されている
- [x] Python の symmetry 文字列名が固定されている
- [x] `all_symmetries()` の順序が Rust / Python ともに固定されている
- [x] `transform_board` が手番を保持することを確認するテストがある
- [x] `transform_square` と `transform_board` の整合を確認するテストがある
- [x] 対称変換後も legal move と石差の整合が保たれることを確認するテストがある
- [x] Python で未知の対称名が `ValueError` になることを確認するテストがある

## 実装開始時点の不足

Step 10 時点では、盤面・合法手・着手・ゲーム状態判定の基本 API は揃ったが、仕様書の `symmetry` モジュールは未実装である。
feature やデータ拡張、学習用途では対称変換を前提とする設計が自然であり、その前提となる盤面変換とマス変換を先に安定化させる必要がある。
このため、Step 11 では symmetry を独立して実装し、後続の feature 系処理が依存できる土台を作る。

## 実装方針

- 変換定義は盤面インデックス規約に厳密に従う
- `transform_square` を定義の基準実装とし、各変換は座標式で固定する
  - `square = rank * 8 + file`
  - `Identity`: `(file, rank) -> (file, rank)`
  - `Rot90`: `(file, rank) -> (7 - rank, file)`
  - `Rot180`: `(file, rank) -> (7 - file, 7 - rank)`
  - `Rot270`: `(file, rank) -> (rank, 7 - file)`
  - `FlipHorizontal`: `(file, rank) -> (7 - file, rank)`
  - `FlipVertical`: `(file, rank) -> (file, 7 - rank)`
  - `FlipDiag`: `(file, rank) -> (rank, file)`
  - `FlipAntiDiag`: `(file, rank) -> (7 - rank, 7 - file)`
- `transform_square` を基礎とし、`transform_board` はそれを使って純粋にビット配置を写像する
- `transform_board` は合法局面専用にせず、単にビット配置と手番を変換する純関数とする
- `all_symmetries()` の順序は仕様と Python 公開の両方で固定する
- Python では enum を直接見せず、文字列で入出力する
- 未知の symmetry 文字列は `ValueError` にする
- 実装はまず `ref` 配下を参照し、類似の最適化実装があれば寄せる
- `ref` に十分近い実装がない場合は、Step 11 では正しさ優先で素直に実装する
- `cargo-mutants` は Rust 側の変換ロジック中心で評価し、PyO3 ラッパ層は pytest で補う

## 段階的な進め方

### Phase 1. 対称変換定義の固定

- `Symmetry` enum を追加する
- 8 種類の変換定義をコードコメントと仕様へ揃える
- `all_symmetries()` の順序を固定する

### Phase 2. Rust API 実装

- `transform_square` を実装する
- `transform_board` を実装する
- 盤面変換が手番を保持することを確認する

### Phase 3. Python 公開

- `transform_board`、`transform_square`、`all_symmetries` を公開する
- 文字列名との相互変換を実装する
- 未知の symmetry 名で `ValueError` を返す

### Phase 4. テストと整合確認

- Rust 側に symmetry 単体テストを追加する
- Python 側に symmetry pytest を追加する
- legal move と石差の整合を確認する
- `make check`、`make coverage-check`、`make mutants` を回して結果を確認する

## 採用する構成

### Rust

- `Symmetry` enum
- `transform_square`
  - 盤面インデックスを 1 マス単位で変換する
- `transform_board`
  - 黒白ビットボードの各マスを対称変換する
- `all_symmetries`
  - 固定順の 8 要素配列を返す

### Python

- `transform_board(board, sym)`
- `transform_square(square, sym)`
- `all_symmetries()`
- 入出力の `sym` は固定文字列

## 検証項目

### 1. Rust API の正しさ

- `Identity` が恒等変換であること
- 回転 4 回で元に戻ること
- 反転 2 回で元に戻ること
- `transform_square` と `transform_board` が整合すること

### 2. 盤面整合

- 対称変換後も黒白ビットが重ならないこと
- `transform_board` が手番を保持すること
- legal move 集合を変換した結果が、変換後盤面の legal move と一致すること
- `disc_count` と `final_margin_from_black` が不変であること

### 3. Python 公開面の整合

- `all_symmetries()` が固定順文字列リストを返すこと
- `transform_board` と `transform_square` が Rust と同じ結果を返すこと
- 未知の symmetry 名で `ValueError` になること

## 品質ゲートの扱い

- `make check` は必須とする
- `make coverage-check` は必須とする
- `make mutants` は必須実行とするが、評価は「結果確認」までとする
- `mutants` の残件は、既存 hotpath 起因か symmetry 起因かを分けて記録する

## 導入時の注意

- symmetry 定義の名前と順序は途中で変えない
- Python 文字列名は将来の feature / augmentation でもそのまま使う前提で決め打つ
- `transform_board` は検証や augmentation でも使える純関数とする
- Step 11 対象外の feature / search に着手して計画を広げない

## このステップを先に行う理由

symmetry は `engine-core` の次に自然な拡張であり、feature や学習データ生成の基盤として再利用しやすい。
ここで変換定義と公開 API を先に固定しておくことで、後続の feature / serialize / random_play を揺れの少ない前提の上で進められる。

## 実装結果

- `src/lib.rs` に `Symmetry`、`all_symmetries`、`transform_square`、`transform_board` を追加した
- Python 公開面として `transform_board(board, sym)`、`transform_square(square, sym)`、`all_symmetries()` を追加した
- Python の `sym` は固定文字列名のみ受け付け、未知の値は `ValueError` にした
- `transform_board` は手番を保持し、黒白ビット配置のみを対称変換する純関数として実装した
- 実装は座標式を基準にした square-wise 変換で固定した

## 検証結果

- `make check`: 成功
  - Rust: `73 passed; 0 failed; 6 ignored`
  - Python: `9 passed`
- `make coverage-check`: 成功
  - line coverage: `85.80%`
- `make mutants`: 実行・結果確認済み
  - `594 mutants tested in 31m: 98 missed, 401 caught, 80 unviable, 15 timeouts`

## mutants 所見

- Step 11 追加分の対称変換ロジックは Rust テストで押さえられている
- `transform_bits` では timeout が 3 件出ているが、これは無限ループ化に近い変異であり、symmetry 定義の誤りを示すものではない
- 残る `missed` の大半は既存の hotpath と PyO3 ラッパ層に集中している
- 計画どおり、`cargo-mutants` は結果確認までとし、PyO3 公開面は pytest で補完した
