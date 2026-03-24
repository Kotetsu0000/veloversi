# Step 09: Python 利用前提の API 実使用速度最適化

## このステップの目的

Step 08 では Rust 側のホットパスと Perft の高速化を進めた。
Step 09 では、Python 用リバーシライブラリとしての実使用速度を対象に、`generate_legal_moves`、`apply_move_unchecked`、`apply_move`、および PyO3 境界の呼び出しコストを測定し、実利用で効く最適化を入れる。

この段階では探索や評価関数の強化は扱わず、単局面 API を Python から大量に呼ぶ用途でのスループット改善を目的とする。

## このステップで行うこと

- Rust 側の API 単体ベンチを整備する
- Python 側の API 呼び出しベンチを整備する
- `generate_legal_moves`、`apply_move_unchecked`、`apply_move` の実使用頻度を前提にホットパスを見直す
- PyO3 境界で無駄な変換や不要な検証がないか見直す
- 必要なら Python 向けの軽量 helper やバッチ API を検討する
- 実行時間の計測結果を記録する
- Rust 単体速度と Python 経由速度を分けて記録する

## 導入対象

- Rust API ベンチ
- Python API ベンチ
- PyO3 境界の軽量化
- 必要に応じた Python 向け helper
- 計測メモ

## 受け入れ条件

- [x] `make check` が成功する
- [x] `make coverage-check` が成功する
- [x] `make mutants` を実行し、結果を確認する
- [x] Rust 側で `generate_legal_moves`、`apply_move_unchecked`、`apply_move` の実使用ベンチを回せる
- [x] Python 側で対応する API ベンチを回せる
- [x] Rust 単体と Python 経由の両方について、少なくとも 1 回分の計測結果が記録されている
- [x] `docs/bench` 配下に実行コマンド、測定日、対象 API、入力条件、経過時間、備考が記録されている
- [x] 高速化内容が README か計測メモで説明されている
- [x] API の外部仕様を変えずに速度改善が入っている
- [x] Python からの利用時に、単発 API と必要ならバッチ API の使い分け方針が明記されている

## 実装開始時点の不足

Step 08 により Rust 側の基礎ホットパスはかなり整理されたが、現時点の計測は Perft 寄りであり、Python ライブラリとしてどの API が実際に支配的かはまだ十分に見えていない。
また、Rust 単体で速くても、PyO3 境界のオーバーヘッドや型変換のコストで Python から見た実効速度が落ちる可能性がある。
このため、次は Python 利用を前提とした実使用ベンチを整備し、その結果に基づいてホットパスを詰める必要がある。

## 現在の実装状況

- Python から `Board`、`initial_board`、`board_from_bits`、`generate_legal_moves`、`apply_move_unchecked`、`apply_move` を呼べる最小 API を用意した
- Python から bits helper API として `generate_legal_moves_bits`、`apply_move_unchecked_bits`、`apply_move_bits` も呼べるようにした
- Rust 側には `generate_legal_moves`、`apply_move_unchecked`、`apply_move` の ignored ベンチを追加した
- Python 側には object API と bits helper API を同じ初期局面ワークロードで比較するベンチスクリプトを追加した
- `Makefile` から Rust 側・Python 側の API ベンチを直接実行できるようにした
- Python 側の object API / bits helper API の整合テストを追加した
- 初期計測値は `docs/bench/api.md` に記録した

## このステップの結果

- Rust 側の API ベンチを `generate_legal_moves`、`apply_move_unchecked`、`apply_move` の 3 本で整備した
- Python 側の API ベンチを object API と bits helper API の両方で整備した
- Python 公開 API に `Board` ベースの最小操作群と bits helper API を追加した
- object API と bits helper API の整合テストを追加し、公開仕様の回帰確認をできるようにした
- 計測の結果、object API と bits helper API の優劣は API と実行条件でぶれうるため、比較基盤を残して継続確認できる形にした
- 通常の Python 利用では object API を推奨し、bits helper API は分解済み局面データを既に持っている呼び出し元向けの補助 API と位置付ける

注記:
- Step 10 で Python 公開面を仕様へ合わせたため、bits helper API と `apply_move_unchecked` は Python 非公開へ戻している

## 実装方針

- Step 08 で整えた共通内部基盤は維持する
- 公開 API の仕様は変えない
- まず計測を整備し、ボトルネックを確認してから最適化する
- Rust 単体の改善と Python 境界の改善は分けて確認する
- 最適化は `generate_legal_moves`、`apply_move_unchecked`、`apply_move` を優先する
- Python 側の改善では、不要なオブジェクト生成や変換を避ける
- 必要なら Python 向けのバッチ API を検討するが、既存単発 API は維持する
- 実装方針の具体化では `ref` 配下の board / mobility / flip 周辺を引き続き参照してよい
- ベンチ結果は同じ条件で比較可能な形で残す

## 段階的な進め方

### Phase 1. 計測基盤の整備

- Rust 側の API ベンチを追加する
- Python 側の API ベンチを追加する
- 比較対象となる入力局面を固定する

### Phase 2. Rust API ホットパスの最適化

- `generate_legal_moves` の実利用経路を見直す
- `apply_move_unchecked` と `apply_move` の差分コストを見直す
- 共通基盤を保ったまま不要な分岐や変換を減らす

### Phase 3. Python 境界の最適化

- PyO3 の型変換コストを見直す
- 必要なら軽量 helper やバッチ API を検討する
- Rust 単体と Python 経由の差分を再計測する

## 採用する構成

### Rust

- 既存の共通内部基盤
  - oriented な `player / opponent` を基準に維持する
- API 単体ベンチ
  - `generate_legal_moves`
  - `apply_move_unchecked`
  - `apply_move`
- 必要に応じたホットパス改善
  - 分岐削減
  - 変換削減
  - API 間の処理共有

### Python

- Python ベンチ
  - 単発 API 呼び出しの反復計測
- 必要に応じた Python 向け helper
  - 単発呼び出しのオーバーヘッドを減らす構成
- 必要に応じたバッチ API
  - 大量局面処理向けの候補として検討する

## 検証項目

### 1. 正しさの回帰確認

- `generate_legal_moves`、`apply_move_unchecked`、`apply_move` の既存テストが引き続き成功すること
- Python 経由でも Rust 側と同じ結果が得られること

### 2. 速度確認

- Rust 単体ベンチで API ごとの比較ができること
- Python ベンチで API ごとの比較ができること
- 少なくとも 1 つ以上の API で改善内容と差分を説明できること

### 3. 運用確認

- Python ライブラリとしての推奨利用形態が説明できること
- 単発 API と必要ならバッチ API の使い分けが明記されていること

## 計測メモに残す項目

- 実行日
- 実行コマンド
- ベンチ対象 API
- 入力条件
- Rust 単体か Python 経由か
- 経過時間
- 備考

## 導入時の注意

- このステップでは API の外部仕様を変えない
- Perft 向け最適化だけを増やさない
- Python での使い勝手を落とす最適化は避ける
- 計測なしで最適化を入れすぎない
- Rust 単体速度と Python 速度を混同しない

## このステップを次に行う理由

Step 08 で Rust 側の基礎ホットパスはかなり整理できた。
次に重要なのは、このライブラリを Python から使ったときにどこで時間を使うかを明確にし、実利用で効く速度改善を入れることである。
そのため、Perft 中心の最適化から一段進めて、API 実使用ベースの計測と改善へ移る。
