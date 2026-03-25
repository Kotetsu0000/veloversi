# Step 14: feature API の仕様実装

## このステップの目的

Step 13 で `random_play` API を実装し、学習用の到達可能局面トレースを生成できるようにした。
Step 14 では、[rust_engine_spec.md](/home/kotetsu0000/program/veloversi/specs/rust_engine_spec.md) の feature モジュールを、深層学習向けの dense feature API として実装する。

このステップでは、MLP 向けの 1 次元特徴と CNN 向けの 2 次元 plane 特徴を、単一局面・複数局面 batch の両方で Python から `numpy.ndarray` として取得できるようにすることを目的とする。

## このステップで行うこと

- `FeatureConfig` を導入する
- `FeaturePerspective` を導入する
- `encode_planes` を実装する
- `encode_planes_batch` を実装する
- `encode_flat_features` を実装する
- `encode_flat_features_batch` を実装する
- Python でも feature API を公開する
- Rust / Python の両方に feature テストを追加する
- 仕様書を Step 14 の対象範囲に合わせて更新する

## 導入対象

- `FeatureConfig`
- `FeaturePerspective`
- `encode_planes`
- `encode_planes_batch`
- `encode_flat_features`
- `encode_flat_features_batch`
- Python 向け feature 公開関数
- feature テスト

## このステップの対象範囲

### Rust で追加する対象

- `FeatureConfig`
- `FeaturePerspective`
- `encode_planes`
- `encode_planes_batch`
- `encode_flat_features`
- `encode_flat_features_batch`

### Python で追加する対象

- `encode_planes(board: Board, history: list[Board], config: dict) -> numpy.ndarray`
- `encode_planes_batch(boards: list[Board], histories: list[list[Board]], config: dict) -> numpy.ndarray`
- `encode_flat_features(board: Board, history: list[Board], config: dict) -> numpy.ndarray`
- `encode_flat_features_batch(boards: list[Board], histories: list[list[Board]], config: dict) -> numpy.ndarray`

### 定義として固定する事項

- planes は `channels_first` を採用する
- 単一局面の plane shape は `(C, 8, 8)` とする
- batch plane shape は `(B, C, 8, 8)` とする
- 単一局面の flat shape は `(F,)` とする
- batch flat shape は `(B, F)` とする
- Python 返り値は `numpy.ndarray` とする
- dtype は `float32` に固定する
- `history` は新しい順で受け取る
- `history` 長が不足する場合は 0 埋めする
- 単一局面版と batch 版は内部実装を共通化する
- Python 返却には `numpy` crate を使う
- `FeatureConfig` は仕様書どおり `history_len` / `include_legal_mask` / `include_phase_plane` / `include_turn_plane` / `perspective` を持つ
- Step 14 では plane の中身は最小構成から始める
- flat は planes の単純 flatten ではなく、固定長 split-flat を別に実装する

## このステップの対象外

このステップでは次を扱わない。

- `encode_nnue_features`
- `engine-search`
- `search_best_move`
- `can_solve_exact`
- `solve_exact`
- 深層学習モデル自体の学習処理
- 学習データ保存フォーマットの最終設計
- WASM 公開 API の本実装

## 受け入れ条件

- [x] `make check` が成功する
- [x] `make coverage-check` が成功する
- [x] `make mutants` を実行し、結果を確認する
- [x] `FeatureConfig` と `FeaturePerspective` が Rust 公開型として実装されている
- [x] `encode_planes` と `encode_planes_batch` が Rust / Python の両方で実装されている
- [x] `encode_flat_features` と `encode_flat_features_batch` が Rust / Python の両方で実装されている
- [x] `encode_planes` の shape が `(C, 8, 8)` / `(B, C, 8, 8)` になることを確認するテストがある
- [x] `encode_flat_features` の shape が `(F,)` / `(B, F)` になることを確認するテストがある
- [x] batch 版が単一局面版と一致することを確認するテストがある
- [x] `history` が新しい順で解釈され、足りない場合は 0 埋めされることを確認するテストがある
- [x] `perspective` の違いが期待どおりに反映されることを確認するテストがある

## 実装開始時点の不足

Step 13 時点では、core API、symmetry、serialize、random_play は揃ったが、学習向けの dense feature API は未実装である。
深層学習用途では、ランダムに生成した局面をそのままモデル入力へ変換できることが重要であり、単一局面だけでなく batch で特徴量を生成できる必要がある。
このため、Step 14 では dense feature API を独立して実装し、後続の学習基盤や `ref` AI との接続前提を先に固める。

## 実装方針

- まず dense feature に限定して実装する
- `encode_nnue_features` は後続ステップへ回す
- Python では `numpy.ndarray` を返し、PyTorch へそのまま流しやすい shape と dtype を優先する
- Python 側の ndarray 返却には `numpy` crate を用いる
- planes は `channels_first` に固定する
- batch 版は単一局面版の内部ロジックを使い回す
- `history` は新しい順で受け取り、足りない分は 0 埋めとする
- `FeatureConfig` の項目は仕様書どおり揃える
- Step 14 の dense feature は最小構成から始め、NNUE 疎特徴や拡張チャネルは後続へ回す
- `cargo-mutants` は Rust 側 feature ロジック中心で評価し、NumPy 公開面の整合は pytest で補う

## 段階的な進め方

### Phase 1. 仕様固定

- `FeatureConfig` の最小項目を固定する
- planes / flat の shape と dtype を固定する
- `history` の順序と不足時の扱いを固定する

### Phase 2. Rust API 実装

- `FeaturePerspective` を追加する
- `FeatureConfig` を追加する
- `encode_planes` / `encode_planes_batch` を実装する
- `encode_flat_features` / `encode_flat_features_batch` を実装する

### Phase 3. Python 公開

- `numpy.ndarray` を返す Python 公開 API を追加する
- 単一局面版と batch 版を公開する
- Python で扱いやすい `dict` config を受ける形にする

### Phase 4. テストと整合確認

- Rust 側に feature 単体テストを追加する
- Python 側に feature pytest を追加する
- `make check`、`make coverage-check`、`make mutants` を回して結果を確認する

## 採用する構成

### Rust

- `FeaturePerspective`
- `FeatureConfig`
- `encode_planes`
- `encode_planes_batch`
- `encode_flat_features`
- `encode_flat_features_batch`

### Python

- `encode_planes(board, history, config)`
- `encode_planes_batch(boards, histories, config)`
- `encode_flat_features(board, history, config)`
- `encode_flat_features_batch(boards, histories, config)`

## 検証項目

### 1. Rust API の正しさ

- plane 出力 shape が想定どおりであること
- flat 出力 shape が想定どおりであること
- batch 出力が単一局面版と一致すること
- `history` が新しい順で解釈されること
- `history` 不足時に 0 埋めされること
- `perspective` に応じて feature 値が変わること

### 2. Python 公開面の整合

- Python で `numpy.ndarray` が返ること
- dtype が `float32` であること
- shape が Rust 側の期待と一致すること
- batch API が想定どおり動くこと

## 品質ゲートの扱い

- `make check` は必須とする
- `make coverage-check` は必須とする
- `make mutants` は必須実行とするが、評価は「結果確認」までとする
- `mutants` の残件は、既存 hotpath 起因か feature 起因かを分けて記録する

## 導入時の注意

- planes / flat の shape は途中で変えない
- `history` の順序は新しい順で固定する
- dtype は `float32` で固定する
- Step 14 対象外の NNUE / search / 深層学習本体に着手して計画を広げない

## このステップを先に行う理由

feature API は random_play に続く自然な学習基盤であり、MLP / CNN の両方に対して再利用しやすい。
ここで dense feature の shape、dtype、history の解釈を先に固定しておくことで、後続の学習データ生成や `ref` AI との接続を揺れの少ない前提の上で進められる。

## 実装結果

- `FeaturePerspective` と `FeatureConfig` を Rust 公開型として追加した
- `EncodedPlanes` / `EncodedPlanesBatch` / `EncodedFlatFeatures` / `EncodedFlatFeaturesBatch` を追加した
- `encode_planes` / `encode_planes_batch` を実装した
- `encode_flat_features` / `encode_flat_features_batch` を実装した
- planes は `channels_first`、flat は split-flat で実装した
- `history` は新しい順で扱い、不足分は 0 埋めにした
- Python では `numpy` crate を使って `numpy.ndarray(float32)` を返すようにした
- Python 公開面では `dict` config を検証してから core を呼ぶ形にした

## 検証結果

- `make check`: 成功
  - Rust: `89 passed; 0 failed; 6 ignored`
  - Python: `28 passed`
- `make coverage-check`: 成功
  - line coverage: `88.44%`
- `make mutants`: 実行・結果確認済み
  - `1178 mutants tested in 47m: 177 missed, 496 caught, 481 unviable, 24 timeouts`

## 補足

- `mutants` の残件は既存 hotpath と PyO3 ラッパ層に多い
- Step 14 固有では `phase_value` / `turn_value` / bit 書き込み helper / dense feature の一部に未捕捉が残る
- ただし Rust / Python の shape、dtype、batch 一致、history 0 埋め、perspective 反映はテストで通過している
