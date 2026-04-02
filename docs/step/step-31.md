# Step 31: Timed Exact Best-Move API for Board / RecordedBoard

## このステップの目的

`Board` と `RecordedBoard` の両方から、全探索で最善手を探せる公開 API を追加する。

主目的は次の 5 つ。

- 全探索ベースで最善手を返す exact 探索 API を追加する
- timeout を超えた場合は partial result ではなく探索失敗を返す
- timeout の既定値を `1.0` 秒にする
- `Board` と `RecordedBoard` の両方で同じ method 名で使えるようにする
- Python から使いやすい戻り値と docstring を用意する

## version 方針

- このステップ中は `0.2.2` を維持する
- このステップは `0.2.x` 系の機能追加として扱う

## このステップで行うこと

- Rust 側に deadline 付き exact 探索 API を追加する
  - 既存の `solve_exact` とは別に、timeout で失敗を返せる API にする
- Python 公開面に module-level API を追加する
  - `search_best_move_exact(board, timeout_seconds=1.0)`
- `Board` に method-style API を追加する
  - `board.search_best_move_exact(timeout_seconds=1.0)`
- `RecordedBoard` に同名 method を追加する
  - `record.search_best_move_exact(timeout_seconds=1.0)`
- README / examples / docstring を更新する
- `make check` を通す

## このステップの対象範囲

### 対象

- `src/search.rs`
- `src/python.rs`
- `src/veloversi/__init__.py`
- `src/veloversi/__init__.pyi`
- `src/veloversi/_core.pyi`
- `src/test_python_api.py`
- README
- examples

### 対象外

- midgame heuristic search の Python 公開
- `search_best_move` の公開設計見直し
- 探索結果の typed object 化
- mutation quality の追加改善

## 固定した前提

- 要求する探索は「全探索」であり、midgame 評価による近似探索ではない
- 既存 `search_best_move` は exact threshold 以下なら exact solver に切り替わるが、timeout 時に失敗を返す仕様ではない
- このステップでは timeout 対応専用の exact API を追加する
- timeout は `float` 秒で受ける
- timeout の既定値は `1.0`
- timeout を超えた場合は部分結果ではなく失敗結果を返す
- `RecordedBoard` は常に `current_board` を探索対象にする
- `Board` と `RecordedBoard` は同じ method 名で使えるようにする

## 推奨 API

- module-level:
  - `search_best_move_exact(board, timeout_seconds=1.0) -> dict`
- `Board` method:
  - `board.search_best_move_exact(timeout_seconds=1.0) -> dict`
- `RecordedBoard` method:
  - `record.search_best_move_exact(timeout_seconds=1.0) -> dict`

戻り値の推奨 shape:

- `success: bool`
- `best_move: int | None`
- `exact_margin: int | None`
- `pv: list[int]`
- `searched_nodes: int`
- `elapsed_seconds: float`
- `failure_reason: str | None`

`failure_reason` は少なくとも次を取り得る。

- `"timeout"`
- `"not_eligible"` もしくはこれに相当する値

## 受け入れ条件

- [ ] `Board.search_best_move_exact(timeout_seconds=1.0)` が使える
- [ ] `RecordedBoard.search_best_move_exact(timeout_seconds=1.0)` が使える
- [ ] module-level `search_best_move_exact(...)` が使える
- [ ] timeout 超過時は partial result ではなく失敗結果を返す
- [ ] timeout 未満で終わる小さな終盤局面では exact 結果が返る
- [ ] README / examples / docstring が更新されている
- [ ] `make check` が成功する

## 実装方針

- exact solver に deadline を渡す別経路を追加する
- 既存 `solve_exact` は保持し、timeout なしの exact API として残す
- timeout 対応版は再帰中に deadline を確認し、超過時は専用の失敗を伝播する
- pruning は既存 exact solver の無駄探索回避を活かす
- Python では `dict` を返す
- `RecordedBoard` では `current_board` を使って module-level helper を呼ぶ

## 懸念点

- 既存 `search_best_move` の exact 経路は timeout 超過でも失敗を返さない
  - このステップではその API を流用せず、timeout 対応 exact API を別に作る
- timeout 判定を細かく入れすぎると exact solver 自体の速度が落ちる
  - 再帰入口での確認を基本にする
- 大きな局面では 1 秒で終わらないことが多い
  - その場合は成功ではなく失敗結果を返す仕様に固定する
- `RecordedBoard` の探索対象は `current_board` だけであり、record 全体を探索対象にするわけではない
  - docstring と README に明記する

## 懸念と解決策

### timeout 判定の粒度

- 懸念:
  - deadline 確認を細かく入れすぎると exact solver 自体が遅くなる
  - 粗すぎると timeout 超過後もしばらく戻らない可能性がある
- 解決策:
  - 再帰入口での確認を基本にする
  - まずは exact solver の主要再帰関数単位で deadline を確認する
  - 必要なら後で hotspot に限定して調整する

### timeout 時の戻り値

- 懸念:
  - timeout 時に partial result を返すと exact API の意味がぶれる
- 解決策:
  - timeout 時は失敗結果に固定する
  - 具体的には
    - `success = False`
    - `best_move = None`
    - `exact_margin = None`
    - `pv = []`
    - `failure_reason = "timeout"`

### exact eligibility の扱い

- 懸念:
  - 空き数が多い局面では exact solver が現実的でない
  - Python API で panic や例外を直接見せると扱いにくい
- 解決策:
  - eligibility を先に確認し、対象外なら失敗結果を返す
  - `failure_reason` には `"not_eligible"` 相当の値を入れる
  - 既存 `SolveConfig` の `exact_solver_empty_threshold` と整合する Rust 側実装にする

### `RecordedBoard` の探索対象

- 懸念:
  - `record.search_best_move_exact(...)` が record 全体を探索対象にするように読める
- 解決策:
  - `RecordedBoard` では常に `current_board` を探索対象にする
  - docstring / README / example で明示する

## version 判断

- このステップの実装前には version は更新しない
- Step 31 完了後に、`0.2.3` として切るか、近い軽微修正を追加してから切るかを判断する

## このステップを先に行う理由

現在の Python 公開面には探索 API がなく、終盤の exact 探索を Python から直接使えない。
また、既存 Rust の `search_best_move` は exact solver への切り替え機能はあるが、timeout 超過時に失敗結果を返す仕様ではない。
終盤の最善手探索を `Board` / `RecordedBoard` の method-style API に揃えるには、この timeout 付き exact API を先に足す必要がある。
