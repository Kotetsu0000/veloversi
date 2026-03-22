# Step 08: Perft / Movegen の本格高速化

## このステップの目的

Step 07 で入れたホットパス最適化の次として、まず `ref` 配下と同じ発想の内部基盤を Rust 側へ作り、そのうえで合法手生成、着手適用、Perft を共通の高速経路へ寄せる。
この段階では新しいゲーム機能は追加せず、Perft の高速化を通じてライブラリ全体のホットパスを速くすることを目的とする。

## このステップで行うこと

- `ref` 配下を参照して、同じ発想の内部基盤を Rust で実装する
- `move / undo` ベースの内部経路を検討・実装する
- 反転計算を現状より専用化できるか見直す
- 合法手生成と反転計算を generic / SIMD に分離可能な内部構造へ整理する
- `target_feature` と runtime dispatch を使い、CPU 機能ごとに最適な実装を自動選択できるようにする
- 末端側の専用化を追加で検討する
- 通常 API と Perft の両方を共通の内部基盤へ寄せる
- `make perft-long` の深いところまで既知値確認を進める
- 実行時間の計測結果を記録する
- 速度比較の基準を記録する

## 導入対象

- 共通の `move / undo` helper
- 共通の oriented 内部状態表現
- 必要に応じた `Flip` 相当の内部表現
- 必要に応じた flip 計算の専用 helper
- 必要に応じた movegen / flip の SIMD 前提内部分離
- `target_feature` 付き SIMD helper と runtime dispatch
- 必要に応じた末端最適化
- 計測メモ

## 受け入れ条件

- [x] Step 06 の Perft 既知値テストが引き続き成功する
- [x] `make check` が成功する
- [x] `make coverage-check` が成功する
- [x] `make mutants` を実行し、結果を確認する
- [x] `generate_legal_moves`、`apply_move_unchecked`、`apply_move`、`perft` が共通の内部基盤を利用する構成になっている
- [x] `make perft-long` が完走する
- [x] `make perft-long` により、少なくとも mode 1 / mode 2 の深さ 15 既知値一致を確認できる
- [x] `docs/bench/perft.md` に実行コマンド、測定日、mode、depth、経過時間、備考が記録されている
- [x] generic 最適化で導入した改善点が README か計測メモで説明されている
- [x] SIMD を利用した高速化経路が導入されている、または導入しない理由が計測結果とともに説明されている
- [x] 配布用 `whl` でも runtime dispatch により CPU 機能ごとの実装を自動選択できる構成になっている
- [x] GitHub Actions で作成した Release artifact を使う場合でも、特定 CPU 固定ではなく runtime dispatch 前提で安全に動く方針が README か計画書に明記されている

## 実装開始時点の不足

Step 07 時点では、Perft のホットパスはかなり軽くなったが、深さ 14 以降はまだ重い。
特に、毎手で次局面値を作る経路、反転計算、末端付近の再帰コストが、深い探索では無視できない。
また、現状は Perft 向けの最適化が先行しており、通常 API の着手処理と完全には基盤共有できていない。
`ref` 配下の `move_board` / `undo_board`、`calc_flip`、generic / SIMD 分離の構造に比べると、まだ共通基盤としての整理余地がある。
このため、Perft 専用ではなく、ライブラリ全体で共有できる高速な内部基盤を先に整える必要がある。

## 現在の実装状況

- 共通の oriented 内部状態表現は導入済み
- `generate_legal_moves`、`apply_move_unchecked`、`apply_move`、`perft` は共通の内部基盤を利用する構成へ整理済み
- `move / undo`、`move_copy`、`pass`、`Flip` 相当の内部表現は導入済み
- generic の合法手生成と table-based flip は導入済み
- `x86_64` では runtime dispatch により次を切り替える
  - movegen: `avx2` / generic
  - flip: `avx2` / generic
  - board update: `sse2` / generic
- 配布方針は単一 `whl` のまま runtime dispatch で CPU ごとの経路を選ぶ構成とする
- 実測上は `generic -> sse2` の差は小さく、主な改善は AVX2 経路で出ている
- `make check`、`make coverage-check`、`make perft-long` は通過済み
- `make mutants` は 428 mutants 中 358 caught / 37 missed / 24 unviable / 9 timeouts を確認済み
- `make perft-long` の mode 1 / mode 2 depth 15 は既知値一致を確認済み

## 実装方針

- 実装方針の具体化では `ref` 配下の `perft`, `board`, `flip`, `mobility` 周辺を参照する
- 最初に `ref` と同じ発想の内部基盤を作る
- 公開 API の `perft(board, depth, mode)`、`generate_legal_moves`、`apply_move` の仕様は変えない
- 高速化は内部 helper に閉じ込め、既存の外部仕様を崩さない
- 最初に generic 側で明確に効く構造変更を入れる
- `move / undo` ベースの経路は Perft 専用に閉じず、通常 API からも利用できる内部基盤として扱う
- flip 計算は、読みやすさとのバランスを見つつ、Perft で効く範囲に絞って専用化する
- movegen と flip は SIMD 最適化可能な内部関数へ分割してよい
- `target_feature` を使う関数は runtime check と必ず対にし、未対応 CPU では呼ばない
- SIMD 非対応環境向けフォールバックを必ず持つ
- SIMD 使用有無で結果が変わらないことをテストで確認する
- GitHub Actions で配布物を作る場合も、バイナリ全体を `target-cpu=native` にせず、ベースは保守的に保つ
- 配布用 `whl` は単一 artifact でもよいが、その中で runtime dispatch により `avx2` / `sse2` / generic を切り替える
- 末端最適化は、正しさ検証がしやすい範囲で段階的に入れる
- `make perft-long` の進捗表示は維持し、長時間実行中の状態が見えるようにする
- 実行時間は計測メモへ残し、generic / SIMD の比較ができるようにする
- 共通基盤化と SIMD 導入は同時に行わず、必ず段階を分ける

## 段階的な進め方

### Phase 1. 共通基盤の generic 実装

- `ref` と同じ発想の oriented な内部状態表現を作る
- `move / undo`、flip、movegen の責務分割を固める
- `generate_legal_moves`、`apply_move_unchecked`、`apply_move`、`perft` を generic な共通基盤へ寄せる
- この段階では SIMD を入れない
- この段階の完了条件は、既存テストと品質ゲートが通ること

### Phase 2. Generic 最適化の詰め

- `move_copy` と `move / undo` のどちらが有利かを計測で判断する
- 末端最適化や flip 計算の専用化を追加する
- `make perft-long` の深いところまで既知値確認を進める

### Phase 3. SIMD 導入

- generic 側で目標に届かない場合に限り SIMD を追加する
- SIMD は movegen または flip の効果が高い側から導入する
- 優先順は `avx2`、`sse2/ssse3`、generic とする
- 各 SIMD 実装は `target_feature` 付きの内部関数として実装し、公開 API では runtime dispatch で選択する
- 導入後は generic と SIMD の結果一致を確認する

## 具体的な実装順

### 1. Generic 側の本格最適化

- `player / opponent` を直接更新する共通の内部状態表現を用意する
- `move_copy` だけでなく `move / undo` 方式も試し、計測で有利な方を採用する
- flip 計算を `ref` の `Flip` 構造体に近い責務へ整理する
- 深さ 1 / 2 に加えて、必要なら深さ 3 近辺の末端最適化を検討する

### 2. 通常 API と Perft の共通化

- `generate_legal_moves`、`apply_move_unchecked`、`apply_move` を共通の内部基盤へ寄せる
- `perft` も同じ内部基盤を利用する
- 通常 API と Perft で別々にホットパスを持ちすぎないように整理する

### 3. movegen / flip の SIMD 化

- generic 側で目標に届かない場合、movegen または flip の SIMD 化を検討する
- SIMD は内部専用にし、結果一致を必須条件とする
- まずは合法手生成か flip 計算のどちらか、効果の高い方から適用する
- `x86_64` 環境では `avx2 -> sse2/ssse3 -> generic` の順で dispatch する
- hand-written SIMD を入れる場合でも、配布物は単一 `whl` で運用できる形を保つ

## 採用する構成

### Rust

- 共通の内部状態表現
  - oriented な `player / opponent` を基準に movegen / apply / perft で共有する
- 共通の局面遷移 helper
  - `move / undo` を含め、通常 API と Perft の両方から利用する
- 共通の flip helper
  - movegen 用ロジックと分離し、必要なら `Flip` 相当の内部表現を持たせる
- generic movegen / flip helper
  - SIMD 非対応環境向けの基準実装として維持する
- SIMD movegen / flip helper
  - 必要な場合だけ追加し、内部で切り替える
- runtime dispatch
  - CPU 機能を見て `avx2` / `sse2` / generic を切り替える
- 計測補助
  - 長時間検証結果を記録できるようにする
- 配布方針
  - GitHub Actions で作る Release artifact でも runtime dispatch が効くようにする

### テスト / 計測

- 既知値回帰テスト
  - Step 06 の既知値が壊れていないことを確認する
- 長時間検証
  - `make perft-long` で深さ 15 までの一致を確認する
- 実行時間記録
  - 深さ 15 の mode 1 / mode 2 を中心に計測結果を残す
- generic / SIMD 比較
  - 両方を持つ場合は差分を記録する

## 検証項目

### 1. 正しさの回帰確認

- 初期局面の深さ 0 から 8 の既知値が引き続き一致すること
- 強制パス局面で mode 差が引き続き一致すること
- generic と SIMD の両方がある場合、結果が一致すること
- runtime dispatch により CPU ごとに適切な経路が選ばれること

### 2. 長時間検証の完了確認

- `make perft-long` が完走すること
- mode 1 / mode 2 ともに深さ 15 の既知値が一致すること

### 3. 共通基盤化の確認

- 通常 API と Perft が同じ内部基盤を利用していること
- 速度改善が Perft 専用に閉じていないこと

### 4. 実行時間の記録

- 少なくとも今回の環境での実行時間を残せること
- generic / SIMD の比較ができる形で記録されていること
- GitHub Actions の Release artifact を使う配布形態でも方針が明文化されていること

## 計測メモに残す項目

- 実行日
- 実行コマンド
- ビルド種別
- mode
- depth
- 経過時間
- 備考

## 速度目標の扱い

- このステップでは、`ref` 配下と同等以上の速度を目標として扱う
- ただし受け入れ判定は、まず深さ 15 完走と既知値一致、および計測記録の完備を基準にする
- 速度比較そのものは、計測メモに明示して継続評価できる状態を作る

## 導入時の注意

- このステップでは Perft の意味や既知値を変えない
- 高速化のために公開 API を崩さない
- 長時間検証のためにコード全体を Perft 専用に歪めすぎない
- 高速化は Perft だけでなく通常 API にも波及する形を優先する
- まずは generic な最適化を優先し、SIMD は必要な場合にだけ導入する
- SIMD を入れる場合も、仕様書どおりフォールバックを維持する
- 共通基盤化が安定するまでは SIMD を入れない
- 配布用ビルドでは CPU 固有最適化を全体へ固定しない
- 単一の `whl` で複数 CPU 機能に対応する前提を守る

## このステップを先に行う理由

Step 07 で明確に効く軽量化は入ったが、深さ 15 の長時間検証を完走しやすくし、さらに `ref` 配下と同等以上の速度を狙うには、Perft 専用ではなくライブラリ全体で共有できる内部基盤の本格最適化が必要である。
この段階を独立して行うことで、通常ロジックの保守性を保ったまま、深い検証と全体的な速度改善を現実的に進めやすくなる。
