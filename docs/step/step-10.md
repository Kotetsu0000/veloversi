# Step 10: engine-core / Python 基本 API の仕様一致

## このステップの目的

Step 09 までで、盤面、合法手生成、着手適用、`board_status`、`perft`、および Python 利用前提の計測基盤までは整った。
Step 10 では、[rust_engine_spec.md](/home/kotetsu0000/program/veloversi/specs/rust_engine_spec.md) のうち、`engine-core` と Python 基本 API に関する仕様へ実装を揃える。

この段階では feature / search / exact solver までは扱わず、まずはコア盤面操作と Python 公開面を仕様どおりに閉じることを目的とする。

## このステップで行うこと

- `specs/rust_engine_spec.md` を正として、`engine-core` 範囲の公開 Rust API を埋める
- Python 基本 API を仕様どおりに揃える
- 仕様と現実装でずれている項目を整理し、必要な範囲で仕様書を実装可能な形へ修正する
- Python 非公開 API は PyO3 モジュールへ載せない構成へ整理する
- Rust / Python のクロス言語整合テストを追加する
- `cargo-mutants` は Rust 中心で評価し、Python ラッパ層は pytest で担保する方針を明記する

## 導入対象

- Rust の `engine-core` 公開 API
- Python 基本公開 API
- 必要なエラー型の整理
- Rust / Python の整合テスト
- Step 10 計画と仕様差分メモ

## このステップの対象範囲

### Rust で追加・整理する対象

- `GameResult`
- `DiscCount`
- `is_legal_move`
- `legal_moves_to_vec`
- `disc_count`
- `final_margin_from_black`
- `final_margin_from_side_to_move`
- `game_result`

### Python で追加・整理する対象

- `validate_board`
- `legal_moves_list`
- `is_legal_move`
- `apply_forced_pass`
- `board_status`
- `disc_count`
- `game_result`
- `final_margin_from_black`

### 仕様修正対象

- PyO3 モジュール名は現実装の `veloversi._core` を正として仕様書を修正する
- `BoardError` は現実に実装可能な定義へ仕様書を修正する
- `final_margin_*` と `game_result` は全局面で定義される関数として仕様書を修正する

## このステップの対象外

このステップでは次を扱わない。

- symmetry
- feature
- serialize
- random_play
- `engine-search`
- `search_best_move`
- `can_solve_exact`
- `solve_exact`
- WASM 公開 API の本実装

対象外の項目は「未実装だが Step 10 の範囲外」として扱い、Step 10 の未完了理由にはしない。

## 受け入れ条件

- [x] `make check` が成功する
- [x] `make coverage-check` が成功する
- [x] `make mutants` を実行し、結果を確認する
- [x] `specs/rust_engine_spec.md` の対象範囲に関する記述が実装可能な形へ更新されている
- [x] Rust 側で `GameResult`、`DiscCount`、`is_legal_move`、`legal_moves_to_vec`、`disc_count`、`final_margin_*`、`game_result` が実装されている
- [x] Python 側で `validate_board`、`legal_moves_list`、`is_legal_move`、`apply_forced_pass`、`board_status`、`disc_count`、`game_result`、`final_margin_from_black` が公開されている
- [x] `apply_move_unchecked` が Python 公開面から外れている
- [x] bits helper API が Python 公開面から外れている
- [x] Rust と Python で legal move 出力が一致するテストがある
- [x] Rust と Python で `apply_move` / `board_status` / `disc_count` / `game_result` の整合を確認するテストがある
- [x] `final_margin_from_black` と `final_margin_from_side_to_move` が全局面で定義されることが仕様書またはコードコメントで明記されている
- [x] README または計画書に Python 公開 / 非公開の整理方針が反映されている

## 実装開始時点の不足

現状の実装は、盤面・合法手生成・着手適用・`board_status`・`perft` までは進んでいるが、仕様書全体から見ると `engine-core` の公開 API がまだ足りない。
また、Python 公開面は Step 09 の計測都合で最小 API と bits helper API を先行導入しており、仕様書にある関数一覧と一致していない。
さらに、仕様書側にも現状の実装方針と合っていない箇所が残っている。
このため、Step 10 ではまず範囲を `engine-core` と Python 基本 API に限定し、仕様・実装・テストを同時に揃える必要がある。

## このステップの結果

- Rust 側に `GameResult`、`DiscCount`、`is_legal_move`、`legal_moves_to_vec`、`disc_count`、`final_margin_from_black`、`final_margin_from_side_to_move`、`game_result` を追加した
- Python 公開面を仕様に合わせ、`validate_board`、`legal_moves_list`、`is_legal_move`、`apply_forced_pass`、`board_status`、`disc_count`、`game_result`、`final_margin_from_black` を公開した
- `apply_move_unchecked` と bits helper API は PyO3 モジュールへ登録しない形で Python 非公開へ戻した
- `specs/rust_engine_spec.md` は、PyO3 モジュール名、`BoardError`、`final_margin_*`、`game_result` の定義を実装可能な形へ更新した
- Rust 側のユニットテストと Python 側の pytest を追加し、legal move、着手結果、`board_status`、`disc_count`、`game_result` の整合を確認できるようにした
- `make check` と `make coverage-check` は通過した
- `make mutants` は実行済みで、`528 mutants tested in 27m: 84 missed, 369 caught, 63 unviable, 12 timeouts` を確認した
- `mutants` の残件は大きく 2 系統で、既存 hotpath の equivalent / timeout 系と、`cargo test` だけでは拾いにくい PyO3 ラッパ層である

## 実装方針

- Step 10 では `specs/rust_engine_spec.md` を正とする
- ただし PyO3 モジュール名については現実装の `veloversi._core` を採用し、仕様書を修正する
- Python 非公開 API は `__all__` から外すだけでなく、PyO3 モジュール自体へ登録しない
- Step 09 で追加した bits helper API は公開面から外す
- `final_margin_from_black` は常に `black_count - white_count` を返す
- `final_margin_from_side_to_move` は常に `side_to_move` 視点の石差を返す
- `game_result` は常に現在局面の石数比較に基づく勝敗を返す
- 上の 3 関数は終局局面で最終結果として解釈できる関数としてコメントと仕様を揃える
- `cargo-mutants` は Rust 公開 API と内部ロジックの健全性確認として使い、Python ラッパ層の保証は pytest とクロス言語整合テストで補う
- Step 08 / 09 で整えた共通内部基盤と API ベンチ基盤は壊さない

## 段階的な進め方

### Phase 1. 仕様差分の解消

- `specs/rust_engine_spec.md` の Step 10 対象範囲を読み直す
- PyO3 モジュール名、`BoardError`、`final_margin_*`、`game_result` の記述を実装可能な形へ更新する
- Step 10 の対象外を仕様との差分メモとして明確化する

### Phase 2. Rust engine-core API の追加

- `GameResult` と `DiscCount` を追加する
- `is_legal_move` と `legal_moves_to_vec` を追加する
- `disc_count`、`final_margin_from_black`、`final_margin_from_side_to_move`、`game_result` を追加する
- 既存の `Board` / movegen / apply / game 状態判定と矛盾しないように実装する

### Phase 3. Python 基本 API の仕様一致

- 仕様にある Python 公開関数を追加する
- `apply_move_unchecked` を Python 非公開へ戻す
- bits helper API を Python 非公開へ戻す
- `.pyi` と `__init__.py` を仕様に合わせる

### Phase 4. テストと整合確認

- Rust 側のユニットテストを追加する
- Python 側の pytest を追加する
- Rust / Python のクロス言語整合テストを追加する
- `make check`、`make coverage-check`、`make mutants` を回して結果を確認する

## 採用する構成

### Rust

- `Board` / `Move` / `LegalMoves` / `BoardStatus` を中心にした既存コアは維持する
- `GameResult` / `DiscCount` を追加して game 系 API を揃える
- `is_legal_move` は既存 movegen のビットマスク結果を利用して実装する
- `legal_moves_to_vec` はビット走査で昇順列挙する
- `disc_count` / `final_margin_*` / `game_result` はヒープ確保なしで計算する

### Python

- 仕様にある基本関数だけを PyO3 モジュールへ載せる
- `Board` の `black_bits` / `white_bits` / `side_to_move` / `to_bits` は維持する
- 仕様外の helper は公開面から外す

### テスト

- Rust 単体テスト
- Python の pytest
- Rust / Python の整合テスト

## 検証項目

### 1. Rust API の正しさ

- `generate_legal_moves` と `is_legal_move` が一致すること
- `legal_moves_to_vec` が昇順列挙になること
- `disc_count` が黒石数・白石数・空きマス数の整合を保つこと
- `final_margin_*` と `game_result` が定義どおりに計算されること

### 2. Python 公開面の整合

- Python から仕様どおりの関数だけが使えること
- `apply_move_unchecked` と bits helper API が Python 公開面に存在しないこと
- Python 例外が `ValueError` として扱えること

### 3. クロス言語整合

- Rust と Python で legal move 出力が一致すること
- Rust と Python で `apply_move` 結果が一致すること
- Rust と Python で `board_status`、`disc_count`、`game_result` が一致すること

## 品質ゲートの扱い

- `make check` は必須とする
- `make coverage-check` は必須とする
- `make mutants` は必須実行とするが、評価は「結果確認」までとする
- `cargo-mutants` の残件は Step 10 の追加 API と既存ロジックのどちらに属するかを分けて記録する

## 導入時の注意

- このステップでは公開 API の整理に伴う破壊的変更を許容する
- ただし変更は仕様一致のためのものに限定し、仕様外の新 API は増やさない
- Step 09 のベンチ基盤は維持するが、仕様外の helper を Python 公開のまま残さない
- Step 10 対象外の項目に着手して計画を肥大化させない
- 仕様修正は実装を正当化するためではなく、実装可能性と意味の明確化のためにだけ行う

## このステップを先に行う理由

Step 09 時点では、コア盤面処理の性能確認と Python 利用前提の計測基盤は整ったが、仕様書に対する公開 API の一致度はまだ低い。
ここで `engine-core` と Python 基本 API を先に仕様一致させておくことで、後続の symmetry / feature / search などを、土台の揺れが少ない状態で追加できる。
