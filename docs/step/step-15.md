# Step 15: モジュール分割による構造整理

## このステップの目的

Step 14 までで、core API、symmetry、serialize、random_play、feature が揃った。
一方で、実装の大半が [src/lib.rs](/home/kotetsu0000/program/veloversi/src/lib.rs) に集約されており、可読性、保守性、レビュー容易性が下がっている。

Step 15 では機能追加を止め、既存実装を責務ごとにモジュール分割して、以後の実装を進めやすい構造へ整理することを目的とする。

## このステップで行うこと

- `src/lib.rs` の責務を分割する
- public API の見え方を変えずに内部構造だけ整理する
- PyO3 wrapper を core ロジックから分離する
- feature / random_play / symmetry / serialize を独立モジュールへ切り出す
- 実装移動後に、その責務に対応するテストも段階的に移動する
- movegen / apply / perft も可能な範囲で整理する
- テストが通る状態を保ったまま構造を改善する

## 導入対象

- `src/lib.rs` の整理
- 新規 Rust モジュール群
- 必要な `pub use`
- 既存テストの追随修正
- テスト配置の整理

## このステップの対象範囲

### Rust で整理する対象

- `Color`
- `Board`
- `BoardError`
- `Move`
- `LegalMoves`
- `BoardStatus`
- `MoveError`
- `PerftError`
- symmetry
- serialize
- random_play
- feature
- PyO3 wrapper

### 目標構成

- `src/lib.rs`
  - module 宣言と public re-export を中心にする
- `src/board.rs`
  - `Color` / `Board` / `BoardError` / `DiscCount` / `GameResult`
- `src/movegen.rs`
  - `Move` / `LegalMoves` / `generate_legal_moves` / `is_legal_move`
- `src/apply.rs`
  - `apply_move` / `apply_forced_pass` / flip / move-undo 周辺
- `src/perft.rs`
  - `perft`
- `src/symmetry.rs`
  - `Symmetry` / `transform_board` / `transform_square` / `all_symmetries`
- `src/serialize.rs`
  - `PackedBoard` / `pack_board` / `unpack_board`
- `src/random_play.rs`
  - `RandomPlayConfig` / `RandomGameTrace` / `PositionSamplingConfig` / sampling
- `src/feature.rs`
  - `FeaturePerspective` / `FeatureConfig` / encode 系
- `src/python.rs`
  - PyO3 wrapper
- `src/flip_tables.rs`
  - 現状維持

## このステップの対象外

このステップでは次を扱わない。

- 新しい機能追加
- NNUE feature
- `ref` AI の探索実装
- exact solver
- 深層学習モデル本体
- WASM 公開 API の本実装
- 既存アルゴリズムの大幅変更

## 受け入れ条件

- [x] `make check` が成功する
- [x] `make coverage-check` が成功する
- [x] `make mutants` を実行し、結果を確認する
- [x] `src/lib.rs` の主責務が module 宣言と re-export に整理されている
- [x] `feature` / `random_play` / `symmetry` / `serialize` が独立ファイルになっている
- [x] PyO3 wrapper が core ロジックから分離されている
- [x] 既存 public API の呼び出し方が変わっていない
- [x] 既存の Rust / Python テストが通る

## 実装開始時点の不足

現在は単一ファイルに責務が集まりすぎており、後続の `ref` AI 実装や学習支援 API を追加するには見通しが悪い。
特に feature と PyO3 wrapper が同居しているため、core ロジックの追跡と Python 公開面の追跡が混ざっている。
このため、Step 15 では機能を増やさず構造だけを整理し、以後の変更コストを下げる。

## 実装方針

- 振る舞いは変えず、責務分割だけを行う
- public API 名と import パスは維持する
- `src/lib.rs` は薄く保つ
- Python wrapper は独立モジュールへ寄せる
- 依存方向は `core -> higher level -> python` に寄せる
- 大きい一括移動ではなく、まとまりごとに段階的に切り出す
- 分割単位は public API 単位を基本にする
- 実行順は低リスク順にする
- visibility は必要最小限の `pub(crate)` に留める
- 公開 API は `lib.rs` で再 export する

### 推奨する分割戦略

- Step 15 では「public API 単位で、低リスク順に段階分割する」方針を採る
- まず依存の単純なモジュールから切り出す
- その後に PyO3 wrapper を分離する
- core の密結合な部分は最後に回す

### 推奨する実行順

1. `symmetry.rs`
2. `serialize.rs`
3. `random_play.rs`
4. `feature.rs`
5. `python.rs`
6. 余力があれば `board.rs` / `movegen.rs` / `apply.rs` / `perft.rs`

### 雑に切らないためのガード

- 1 phase で扱う責務は 1 つか 2 つまでに限定する
- 機能変更を混ぜない
- 実装中に「ついでにここも」と見えた項目はその場で着手せず、`todo.md` に記載して後続 step へ送る
- `python.rs` 分離は core 分離より先にやる
- `board` / `movegen` / `apply` / `perft` は最後に回す
- モジュール移動ごとに `use` / `pub use` / `wrap_pyfunction!` の参照を確認する
- 各責務について「実装移動 → `make check` → テスト移動 → `make check`」の順を守る
- テストの大移動を先にやらない

## 段階的な進め方

### Phase 1. 低リスク分割

- `symmetry`
- `serialize`
- `random_play`
- `feature`
- それぞれ「実装移動 → 検証 → テスト移動 → 再検証」で進める

### Phase 2. Python wrapper 分離

- PyO3 wrapper を `src/python.rs` へ移す
- `cfg(not(any(test, coverage)))` の位置も整理する

### Phase 3. core 分割

- `board`
- `movegen`
- `apply`
- `perft`

### Phase 4. テストと整合確認

- Rust / Python テストを通す
- `make check`、`make coverage-check`、`make mutants` を実行する

## 品質ゲートの扱い

- `make check` は必須とする
- `make coverage-check` は必須とする
- `make mutants` は必須実行とするが、評価は「結果確認」までとする
- mutation quality の改善は副次効果に留め、主目的は構造整理とする

## 導入時の注意

- public API の import パスを変えない
- Step 15 で機能追加しない
- 既存テストの意味を変えない
- 1 つの移動で大量の意味変更を混ぜない

## このステップを先に行う理由

次に予定している `ref` AI 実装や学習支援の拡張は、feature、random_play、Python wrapper との接続点が増える。
その前にモジュール境界を整理しておく方が、以後の差分を小さくしやすく、レビューもしやすい。

## 実装結果

- [src/lib.rs](/home/kotetsu0000/program/veloversi/src/lib.rs) を薄いエントリポイントに整理し、module 宣言と public re-export に集約した
- [src/feature.rs](/home/kotetsu0000/program/veloversi/src/feature.rs)、[src/random_play.rs](/home/kotetsu0000/program/veloversi/src/random_play.rs)、[src/serialize.rs](/home/kotetsu0000/program/veloversi/src/serialize.rs)、[src/symmetry.rs](/home/kotetsu0000/program/veloversi/src/symmetry.rs)、[src/python.rs](/home/kotetsu0000/program/veloversi/src/python.rs) を新設した
- core の残りは [src/engine.rs](/home/kotetsu0000/program/veloversi/src/engine.rs) に集約し、Step 15 で切り出した責務の duplicate 実装は削除した
- 移動した責務の Rust テストも対応モジュールへ移した
- `make mutants` は、分割後に [src/lib.rs](/home/kotetsu0000/program/veloversi/src/lib.rs) 単体では 0 mutants になるため、`src/*.rs` を対象にし `src/flip_tables.rs` を除外する形へ更新した

## 検証結果

- `make check`: 成功
  - Rust: `89 passed; 0 failed; 6 ignored`
  - Python: `28 passed`
- `make coverage-check`: 成功
  - line coverage: `88.30%`
- `make mutants`: 実行・結果確認済み
  - `1178 mutants tested in 12m: 177 missed, 494 caught, 481 unviable, 26 timeouts`

## 残課題

- `mutants` の未捕捉は依然として `engine.rs` の既存 hotpath、`feature.rs` の helper、`python.rs` の wrapper に残っている
- Step 15 の目的は構造整理であり、mutation quality の改善は後続 step へ送る
