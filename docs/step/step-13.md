# Step 13: random_play API の仕様実装

## このステップの目的

Step 12 で `serialize` API を実装した。
Step 13 では、[rust_engine_spec.md](/home/kotetsu0000/program/veloversi/specs/rust_engine_spec.md) の `random_play` モジュールを実装し、合法手のみを使ったランダム対局トレース生成と、到達可能局面サンプリングを Rust / Python の両方で扱えるようにする。

このステップでは、単なるランダム対局ではなく、深層学習用データ生成に使える「局面列・着手列・最終結果」を保持したトレース API を整えることを目的とする。

## このステップで行うこと

- `RandomPlayConfig` を導入する
- `RandomGameTrace` を導入する
- `play_random_game` を実装する
- `PositionSamplingConfig` を導入する
- `sample_reachable_positions` を実装する
- Python でも random_play API を公開する
- Rust / Python の両方に random_play テストを追加する
- 学習用途に必要なトレース情報の形を仕様書・計画書で固定する

## 導入対象

- `RandomPlayConfig`
- `RandomGameTrace`
- `play_random_game`
- `PositionSamplingConfig`
- `sample_reachable_positions`
- Python 向け random_play 公開関数
- random_play テスト

## このステップの対象範囲

### Rust で追加する対象

- `RandomPlayConfig`
- `RandomGameTrace`
- `play_random_game`
- `PositionSamplingConfig`
- `sample_reachable_positions`

### Python で追加する対象

- `play_random_game(seed: int, config: dict) -> dict`
- `sample_reachable_positions(seed: int, config: dict) -> list[Board]`

### 定義として固定する事項

- `RandomPlayConfig` は少なくとも `max_plies: Option<u16>` を持つ
- `RandomGameTrace` は少なくとも次を持つ
  - `boards: Vec<Board>`
  - `moves: Vec<Option<Move>>`
  - `final_result: GameResult`
  - `final_margin_from_black: i8`
  - `plies_played: u16`
  - `reached_terminal: bool`
- `moves` ではパスを `None` で表す
- `max_plies = None` は終局まで進める
- `max_plies = Some(n)` は最大 `n` plies で停止する
- 乱数生成器は seed 再現可能な軽量 PRNG をライブラリ内に持つ
- `sample_reachable_positions` は初期版では単純 sampling とし、重複除去や複雑な分布制御は行わない

## このステップの対象外

このステップでは次を扱わない。

- feature
- `encode_planes`
- `encode_flat_features`
- `engine-search`
- `search_best_move`
- `can_solve_exact`
- `solve_exact`
- 深層学習モデル自体の学習処理
- WASM 公開 API の本実装

## 受け入れ条件

- [x] `make check` が成功する
- [x] `make coverage-check` が成功する
- [x] `make mutants` を実行し、結果を確認する
- [x] `RandomPlayConfig` と `RandomGameTrace` が Rust 公開型として実装されている
- [x] `play_random_game` と `sample_reachable_positions` が Rust 公開 API として実装されている
- [x] Python 側で `play_random_game` と `sample_reachable_positions` が公開されている
- [x] 同じ seed と同じ config で再現可能な結果になることを確認するテストがある
- [x] 返るトレースが合法手列だけで構成されることを確認するテストがある
- [x] パスが `None` として記録されることを確認するテストがある
- [x] `max_plies` による途中停止を確認するテストがある
- [x] `sample_reachable_positions` が到達可能局面のみを返すことを確認するテストがある

## 実装開始時点の不足

Step 12 時点では、盤面の core API、対称変換、serialize API は揃ったが、学習データ生成に必要なランダム対局トレース API は未実装である。
深層学習用には、単発局面だけでなく「どの手順で現在局面に至ったか」と「その最終勝敗や石差」を保持したトレースが必要である。
このため、Step 13 では random_play を独立して実装し、学習用局面生成の基盤を先に固める。

## 実装方針

- `play_random_game` は単なる終局関数ではなく、学習用に使える完全なトレースを返す
- `boards` と `moves` は対応関係が分かる形で保持する
- 強制パスは明示的に `None` として保持し、手順情報を失わないようにする
- 乱数生成器は軽量・再現可能・ seed 固定で deterministic なものをライブラリ内に持つ
- `sample_reachable_positions` はまず単純な reachability sampling を実装し、分布制御は後続ステップへ回す
- Python 公開面では最初は `dict` / `list[Board]` ベースで扱い、学習用に取り回しやすい形を優先する
- `cargo-mutants` は Rust 側 random_play ロジック中心で評価し、PyO3 ラッパ層は pytest で補う

## 段階的な進め方

### Phase 1. 仕様固定

- `RandomPlayConfig` の最小項目を固定する
- `RandomGameTrace` の最小項目を固定する
- パス表現と `max_plies` の意味を固定する

### Phase 2. Rust API 実装

- PRNG を追加する
- `play_random_game` を実装する
- `sample_reachable_positions` を実装する

### Phase 3. Python 公開

- `play_random_game` を公開する
- `sample_reachable_positions` を公開する
- Python で扱いやすい `dict` / `list` 形式へ変換する

### Phase 4. テストと整合確認

- Rust 側に random_play 単体テストを追加する
- Python 側に random_play pytest を追加する
- `make check`、`make coverage-check`、`make mutants` を回して結果を確認する

## 採用する構成

### Rust

- `RandomPlayConfig`
- `RandomGameTrace`
- `PositionSamplingConfig`
- `play_random_game`
- `sample_reachable_positions`

### Python

- `play_random_game(seed, config)`
- `sample_reachable_positions(seed, config)`
- `play_random_game` の返り値は `boards` / `moves` / `final_result` / `final_margin_from_black` / `plies_played` / `reached_terminal` を含む `dict`

## 検証項目

### 1. Rust API の正しさ

- 同じ seed と config で同じトレースが返ること
- 各着手が常に合法手から選ばれていること
- パス局面では `moves` に `None` が入ること
- `max_plies` 到達時には途中停止し、`reached_terminal == false` になること
- 終局まで進んだ場合、`final_result` と `final_margin_from_black` が盤面と一致すること

### 2. Python 公開面の整合

- Python でも同じ seed と config で再現可能な結果になること
- トレースの `boards` / `moves` / `final_result` / `final_margin_from_black` が期待どおりに返ること
- `sample_reachable_positions` が `Board` リストを返すこと

## 品質ゲートの扱い

- `make check` は必須とする
- `make coverage-check` は必須とする
- `make mutants` は必須実行とするが、評価は「結果確認」までとする
- `mutants` の残件は、既存 hotpath 起因か random_play 起因かを分けて記録する

## 導入時の注意

- trace の field 名と意味は途中で変えない
- `moves: Vec<Option<Move>>` のパス表現は固定する
- `max_plies` の意味を「最大 plies 数」で統一する
- Step 13 対象外の feature / search / 深層学習本体に着手して計画を広げない

## このステップを先に行う理由

random_play は深層学習用の局面生成、ベンチ用局面作成、将来の feature 抽出や `ref` AI 推論ラベル付けの前提として再利用しやすい。
ここでランダム対局トレースの形を先に固定しておくことで、後続の feature / `ref` AI 再現 / 学習データ生成を揺れの少ない前提の上で進められる。

## 実装結果

- `RandomPlayConfig { max_plies: Option<u16> }` を Rust 公開型として追加した
- `RandomGameTrace` を Rust 公開型として追加した
- `PositionSamplingConfig` を Rust 公開型として追加した
- `play_random_game(seed, &config)` を実装した
- `sample_reachable_positions(seed, &config)` を実装した
- trace は `boards` / `moves` / `final_result` / `final_margin_from_black` / `plies_played` / `reached_terminal` を保持する
- 強制パスは `moves` 内で `None` として記録する
- `max_plies` で trace 記録は途中停止できるが、終局ラベルは内部で最後まで進めて計算する
- Python 側では `play_random_game(seed, config) -> dict` と `sample_reachable_positions(seed, config) -> list[Board]` を公開した

## 検証結果

- `make check`: 成功
  - Rust: `84 passed; 0 failed; 6 ignored`
  - Python: `23 passed`
- `make coverage-check`: 成功
  - line coverage: `86.93%`
- `make mutants`: 実行・結果確認済み
  - `669 mutants tested in 36m: 148 missed, 414 caught, 89 unviable, 18 timeouts`

## 補足

- `mutants` の残件は主に既存 hotpath と PyO3 ラッパ層に集中している
- Step 13 固有では `XorShift64Star` と `sample_reachable_positions` の一部に未捕捉が残るが、`make check` と Python/Rust 双方の再現性・合法手列・途中停止テストは通過している
- Python 公開面は学習用途を優先して `dict` / `list[Board]` で固定した
