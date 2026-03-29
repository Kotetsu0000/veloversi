# Step 27: 0.1.0 Preparation

## このステップの目的

`0.1.0` を出せる状態まで、深層学習支援ライブラリとしての残件をまとめて閉じる。

主目的は次の4つ。

- `history_len > 0` を含む学習 batch API を完成させる
- DataLoader / examples / README を現在の API と整合させる
- `make check` / `make coverage-check` / `make mutants` を通す
- mutation 残件を、equivalent / timeout / 現実的に除去困難なもの以外は潰す

## version 方針

- このステップ中は `0.0.1` を維持する
- このステップの完了条件を満たした時点で `0.1.0` を切る

## このステップで行うこと

- `prepare_planes_learning_batch` の `history_len > 0` 対応
- `prepare_flat_learning_batch` の `history_len > 0` 対応
- `moves_until_here` から replay して history を復元する
- `value-only` / `policy + value` の example を history 対応後の API に合わせて整える
- `README.md` と `examples/README.md` を最終整備する
- `make check` を通す
- `make coverage-check` を通す
- `make mutants` を実行し、残件を整理する

## このステップの対象範囲

### 対象

- `src/learning.rs`
- 必要なら `src/random_play.rs`
- Python 公開面
- examples
- README
- mutation quality の改善

### 対象外

- 学習ループ本体
- 学習済みモデル推論 runtime
- parquet / Arrow / HDF5 など別保存形式
- 新しい AI / 探索器

## 固定した前提

- `history` は `moves_until_here` を初期局面から replay して復元する
- history の順序は既存 feature API に合わせて「新しい順」
- `value-only` は全 sample を対象にする
- `policy + value` は policy 有効 sample のみを対象にする
- value target は `final_margin_from_side_to_move / 64.0`
- pass は policy 学習の対象にしない
- `make mutants` の残件は
  - equivalent
  - timeout
  - 現実的に除去困難
  だけを許容する
- Step 27 完了後の commit はユーザーが行う
- こちらは commit を作らず、必要な commit コマンドだけ提示する

## 受け入れ条件

- [x] `prepare_planes_learning_batch` が `history_len > 0` を扱える
- [x] `prepare_flat_learning_batch` が `history_len > 0` を扱える
- [x] replay 由来の history が既存 feature 契約どおり「新しい順」で入る
- [x] `value-only` / `policy + value` の example が現状 API と一致する
- [x] `README.md` / `examples/README.md` が最新状態に揃っている
- [x] `make check` が成功する
- [x] `make coverage-check` が成功する
- [x] `make mutants` を実行済みである
- [x] `make mutants` の残件が equivalent / timeout / 現実的に除去困難なものだけである

## 実装方針

- Step 24 の history 対応をこの step に取り込む
- replay は正しさ優先で初期局面から行う
- `None` は `apply_forced_pass` で厳密に進める
- `collate_fn` / model input API の契約は Step 26 を維持する
- mutation 残件は機械的に全潰しせず、理由を区別して整理する
- history replay では、replay 完了後の board が example の current board と一致することを直接検証する
- `make mutants` は
  - まず history 対応を通す
  - その後に実行する
  - 残件を分類して潰す
  の順で進める
- 残す `missed` は
  - equivalent
  - timeout
  - 現実的に除去困難
  のいずれかに明示分類する

## 懸念点

- history replay は batch ごとにコストがかかる
  - Step 27 では正しさ優先で進める
- `make mutants` は探索・feature・python wrapper に広く影響する
  - 途中で scope を広げすぎないようにする
- equivalent 判定を雑にすると品質確認の意味が薄くなる
  - 残す場合は理由を明文化する
- pass を含む replay は forced pass と一致しないと壊れる
  - `None` は forced pass としてのみ受理する

## このステップを先に行う理由

ここまでで深層学習支援に必要な API 群は概ね揃っている。
残っている本命は、history を含む batch 化と、`0.1.0` に出せる品質の確認である。
これをまとめて閉じることで、`0.1.0` の区切りを明確にできる。

## 実装結果

- `prepare_planes_learning_batch` / `prepare_flat_learning_batch` で `history_len > 0` を有効化
- `moves_until_here` を初期局面から replay して history を復元
- replay 完了後の board が current board と一致しない場合は `ReplayMismatch`
- `None` は forced pass としてのみ受理
- DataLoader / examples / README を Step 26 の model input API と整合させた
- `0.1.0` へ version を更新した

## 検証結果

- `make check`
  - Rust: `158 passed; 0 failed; 6 ignored`
  - Python: `47 passed`
- `make coverage-check`
  - total line coverage: `90.58%`
  - `feature.rs`: `100.00%`
  - `learning.rs`: `99.61%`
- `make mutants`
  - `1676 mutants tested in 20m: 190 missed, 850 caught, 594 unviable, 42 timeouts`
  - missed 内訳
    - `engine.rs`: `40`
    - `python.rs`: `105`
    - `random_play.rs`: `2`
    - `search.rs`: `43`
  - timeout 内訳
    - `engine.rs`: `21`
    - `feature.rs`: `6`
    - `learning.rs`: `3`
    - `search.rs`: `9`
    - `symmetry.rs`: `3`

## mutation 残件の整理

残件は次の 3 分類で整理した。Step 27 の完了判定では、この分類に入らない missed は残していない。

- equivalent
  - `engine.rs`
    - `Board::new_initial` の `| -> ^`
    - `read_h2vline` の `| -> ^`
    - `flips_for_move_bits` の `| -> ^`
  - `search.rs`
    - `mid_evaluate_diff` の `empty_bits` `| -> ^`
    - `2 * parity_term` の `* -> /`
    - `corner_closeness_penalty` の C-mask 定義 `| -> ^`
    - `corner occupancy` 判定の `player_bits | opponent_bits -> ^`
  - `random_play.rs`
    - `sample_reachable_positions` の早期 return `|| -> &&`
- timeout
  - `engine.rs`
    - `Board::empty_bits`
    - `OrientedBoard::legal_moves` / `calc_flip`
    - `board_status`
    - `legal_moves_to_vec`
    - `perft_count_depth_two`
    - `perft_with_mode_oriented`
  - `feature.rs`
    - `write_bit_plane`
    - `write_bit_vector`
  - `learning.rs`
    - `legal_move_masks`
  - `search.rs`
    - `can_solve_exact`
    - `solve_exact`
    - `nega_scout` の一部算術変異
  - `symmetry.rs`
    - `transform_bits`
- 現実的に除去困難
  - `python.rs`
    - PyO3 wrapper 変換関数群全般
    - `cargo test` ベースの `cargo-mutants` では Python 側の assert を直接効かせにくく、実質的な検証主体は `pytest`
  - `engine.rs`
    - 非 `x86_64` 分岐の backend selector
    - 現行 `x86_64` 環境で到達しない generic fallback (`move_board` / `undo_board` / `move_copy` / `flips_for_move_bits_unchecked`)
    - curated / deterministic-random / long-ray oracle を追加しても残る generic movegen / flip の内部 propagation
  - `search.rs`
    - `search_best_move` / `search_root` / `nega_scout` / `ordered_moves` の内部探索制御
    - depth 1-3 の brute-force 比較、TT on/off、time-limit 形状確認後も残るため、公開挙動ではなく内部探索分岐に強く依存する

## 補足

- `python.rs` の coverage は Rust 側の `cargo llvm-cov` では 0% のままだが、これは wrapper を coverage build から外し、pytest 側で検証する現行設計による
- Step 27 完了後の commit はユーザーが行う
- こちらは commit を作らず、必要な commit コマンドだけ提示する
