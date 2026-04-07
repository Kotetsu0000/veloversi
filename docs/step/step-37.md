# Step 37: `always_try_exact` Opt-in for Model Move Selection

## このステップの目的

`select_move_with_model(...)` は現在、

- `empty_count <= exact_from_empty_threshold`
  - exact-only
- `empty_count > exact_from_empty_threshold`
  - model のみ

という切替になっている。

このステップでは、opt-in 設定 `always_try_exact` を追加し、

- model を主経路にしつつ
- `exact_from_empty_threshold` を超える局面でも exact を試せる
- 閾値より手前では exact / model を並列実行し、先に妥当な結果を返した側を採用する
- 閾値以下の最終盤では exact のみを使い、確定行動にする

という挙動を実装する。

## version 方針

- このステップ中は `0.2.4` を維持する
- 単体で release するなら次は `0.2.5`
- ただし Step 38 とまとめるなら version は後で再判断する

## このステップで行うこと

### Phase 1: API 拡張

- `select_move_with_model(...)`
- `Board.select_move_with_model(...)`
- `RecordedBoard.select_move_with_model(...)`

に `always_try_exact: bool = False` を追加する。

## Phase 2: 実行条件の整理

- `always_try_exact=False`
  - `empty_count <= exact_from_empty_threshold` では exact-only
  - `empty_count > exact_from_empty_threshold` では model のみ
- `always_try_exact=True`
  - `empty_count > exact_from_empty_threshold`
    - exact / model を並列開始する
    - 先に妥当な結果を返した側を採用する
  - `empty_count <= exact_from_empty_threshold`
    - exact のみを使う
    - model fallback は行わない

### Phase 3: 文書とテスト

- README / docstring / stub 更新
- `always_try_exact=False` の既存挙動維持テスト
- `always_try_exact=True` の exact 先着ケース / model fallback ケースを追加
- `make check` を通す

## 対象範囲

### 対象

- `src/veloversi/__init__.py`
- `src/veloversi/__init__.pyi`
- `src/veloversi/_core.pyi`
- `src/test_python_api.py`
- `README.md`
- `docs/step/step-37.md`
- `docs/step/todo.md`

### 対象外

- exact solver 自体の高速化
- model 推論器の内部実装変更
- Rust 側の新しい推論モデル追加

## 固定した前提

- `always_try_exact` は opt-in
- 既定値は `False`
- `always_try_exact=True` でも timeout は `timeout_seconds` を共有する
- `exact_from_empty_threshold` は残す
  - `always_try_exact=False` 時の既定切替条件として使う
- `empty_count <= exact_from_empty_threshold` では `always_try_exact` の値に関係なく exact-only で動作する
- `always_try_exact=True` は `exact_from_empty_threshold` より優先する
  - `exact_from_empty_threshold=None` でも exact を並列開始する
- terminal / 強制パス局面では `always_try_exact` を見ずに既存の終局/強制パス経路を優先する
- `always_try_exact=True` かつ `empty_count > exact_from_empty_threshold` では
  - exact / model の両方を開始する
  - 先に成功結果を返した側を採用する
- `always_try_exact=True` かつ `empty_count <= exact_from_empty_threshold` では
  - exact のみを使う
  - exact が timeout / failure した場合は失敗結果を返す
- `source` は実際に返した側を表す
  - 競走区間では `"exact"` / `"policy"` / `"value_search"` / `"forced_pass"`
  - 最終盤 exact-only 区間では成功時は常に `"exact"`
- `timeout_reached` は timeout に達した場合のみ `True` にする

## 実装結果

- `always_try_exact` を
  - module-level `select_move_with_model(...)`
  - `Board.select_move_with_model(...)`
  - `RecordedBoard.select_move_with_model(...)`
  に追加した
- `always_try_exact=False`
  - `empty_count <= exact_from_empty_threshold` では exact-only
  - `empty_count > exact_from_empty_threshold` では既存どおり model のみ
- `always_try_exact=True`
  - `empty_count > exact_from_empty_threshold` では exact / model を競走させ、先着した成功結果を返す
  - `empty_count <= exact_from_empty_threshold` では exact-only で動作する
- README / docstring / stub を更新した

## 受け入れ条件

- [x] `always_try_exact` が 3 経路の API に追加されている
- [x] `always_try_exact=False` で、閾値以下 exact-only / 閾値超え model-only の挙動になっている
- [x] `always_try_exact=True` で `empty_count > exact_from_empty_threshold` の局面で exact / model を並列開始できる
- [x] `always_try_exact=True` で `empty_count <= exact_from_empty_threshold` の局面では exact-only で動作する
- [x] README / docstring / stub が更新されている
- [x] `make check` が成功する

## 検証

- `uv run pytest -q src/test_python_api.py -k "select_move_with_model or search_best_move_exact"`
  - `16 passed`
- `make check`
  - Rust `161 passed; 0 failed; 14 ignored`
  - Python `73 passed`

## 懸念点

### `always_try_exact=True` が CPU を食いすぎる

- 懸念:
  - 空きが多い局面で exact を毎回起動すると model 側も遅くなる可能性がある
- 解決策:
  - 既定値は `False` に固定する
  - README で opt-in かつ高コストであることを明記する
  - 特に `device="cpu"` では exact/model が同じ CPU 資源を奪い合うことを明記する

### `exact_from_empty_threshold` と `always_try_exact` の役割が曖昧

- 懸念:
  - 似た役割の引数が並ぶと使い分けが分かりにくい
- 解決策:
  - `exact_from_empty_threshold` は既定の exact 開始条件
  - `always_try_exact` はその条件を無視して常に exact も試す opt-in
  - と明記する

### exact がほぼ勝てない局面でも並列で走る

- 懸念:
  - `always_try_exact=True` のとき、実際には model しか返らない局面が多い
- 解決策:
  - これは opt-in の意図されたコストとして許容する
  - 後続で必要なら `always_try_exact` の追加ヒューリスティックを検討する

### `source` / `timeout_reached` の解釈が崩れる

- 懸念:
  - 閾値より手前では競走、閾値以下では exact-only になるため、結果の意味を誤読しやすい
- 解決策:
  - `source` は実際に返した側だけを表す
  - 競走区間では先着した側の `source` を返す
  - 最終盤 exact-only 区間では成功時は `"exact"` に固定する
  - `timeout_reached` は timeout に達した場合のみ `True` にする

### `always_try_exact` が terminal / 強制パスで無意味に走る

- 懸念:
  - legality 判定より前に exact を開始すると、不要な thread 起動や余計な分岐が入る
- 解決策:
  - terminal / 強制パス判定を先に行い、そこで返せる局面では exact/model の並列開始自体を行わない

### 閾値以下で model fallback が無いと失敗が増える

- 懸念:
  - 最終盤を exact-only にすると、timeout が短い設定では以前より failure が増える
- 解決策:
  - これは確定行動を優先する仕様として受け入れる
  - README で、閾値以下では exact 完了に必要な時間を確保すべきことを明記する
