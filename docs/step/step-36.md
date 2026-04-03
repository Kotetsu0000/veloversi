# Step 36: Concurrent Exact / Model Fallback for Move Selection

## このステップの目的

Step 35 では、PyTorch `nn.Module` を使った着手選択 API を追加した。
ただし終盤では、現在の挙動は次の順序になっている。

1. exact 探索を試す
2. exact が timeout などで失敗した場合だけ model 探索へフォールバックする

この方式だと、逐次 exact 実行に時間を使い切ると、model 側に十分な時間が残らない。

このステップでは、
- exact 探索
- model 探索
を並列に走らせ、

- exact が間に合えば exact を返す
- exact が間に合わなくても model 側に結果があれば model を返す

という形にして、終盤の実用性を上げる。

## version 方針

- このステップ中は `0.2.3` を維持する
- このステップ完了後に `0.2.4` を切る

## このステップで行うこと

### Phase 1: exact / model 並列実行の追加

- `select_move_with_model(...)` の exact fallback 経路を見直す
- `exact_from_empty_threshold` 以下の局面では
  - exact 探索
  - model 探索
  を同時に開始する

### Phase 2: 優先順位の固定

- exact が制限時間内に成功したら exact を返す
- exact が未完でも、制限時間到達時に model 側の結果があれば model を返す
- どちらも結果が無ければ timeout failure を返す

### Phase 3: GIL / thread の扱い

- exact 探索が本当に model 推論と並列になるように、Rust exact API 呼び出し時の GIL 制約を確認する
- 必要なら PyO3 側で `allow_threads` 相当を使う
- Python 側は root orchestration に集中し、探索ロジック自体は分離する

### Phase 4: 文書と検証

- README / docstring / 型 stub を更新する
- timeout を含む Python テストを追加する
- `make check` を通す

## このステップの対象範囲

### 対象

- `src/python.rs`
- `src/veloversi/__init__.py`
- `src/veloversi/__init__.pyi`
- `src/veloversi/_core.pyi`
- `src/test_python_api.py`
- `README.md`
- `docs/step/step-36.md`
- `docs/step/todo.md`

### 対象外

- exact solver 自体のアルゴリズム高速化
- midgame heuristic search の改善
- `torch` の package dependency 化
- learned model の保存 / 読み込み API

## 固定した前提

- `model` は引き続き PyTorch `nn.Module`
- `torch` は optional runtime import のまま維持する
- 終盤の exact / model 並列化は `select_move_with_model(...)` の内部でのみ扱う
- `search_best_move_exact(...)` 自体の公開 API は変えない
- exact が制限時間内に成功した場合は、常に exact を優先する
- `RecordedBoard` は常に `current_board` を対象にする
- `exact_from_empty_threshold` 以下では、まず exact / model を並列に開始する
- model 結果は
  - `success = True`
  - `best_move is not None`
  のときだけ fallback 候補とする
- exact 側の締切は `timeout_seconds` と共有する
- exact がこの全体締切までに成功しない場合、model 結果があればそれを返す
- model 結果が無い場合だけ、全体締切まで待つ
- 返り値の識別は既存の `source` を使う
  - `"exact"`
  - `"policy"`
  - `"value_search"`
  - `"forced_pass"`
- `timeout_reached` は、exact が timeout / failure して model fallback を返した場合に `True` とする
- exact の timeout 以外の failure でも model fallback を許可する

## 受け入れ条件

- [x] `exact_from_empty_threshold` 以下で exact / model を並列に走らせられる
- [x] exact が制限時間内に成功した場合は exact を返す
- [x] exact が未完でも、model 結果があれば model を返せる
- [x] どちらも結果が無い場合だけ timeout failure になる
- [x] README / docstring / 型 stub が更新されている
- [x] `make check` が成功する

## 実装方針

- `select_move_with_model(...)` の root orchestration を分離する
- exact 経路と model 経路の結果を別スレッド / 別経路で集約する
- exact の公開 Python wrapper が GIL を保持して並列性を阻害しないよう確認する
- model 側の timeout 仕様は Step 35 のまま維持する
- exact と model は `timeout_seconds` を共有する
- Python 側は orchestration を担当し、Rust / PyO3 側は exact 呼び出しの GIL 解放を担当する
- Python 側の並列実装は `ThreadPoolExecutor(max_workers=2)` を基本とする
- 実行する worker は exact 1 本、model 1 本に限定する
- 返却後に片方の worker が完了まで残る可能性は仕様として許容する

## 実装結果

- `select_move_with_model(...)` の終盤 exact fallback を、逐次実行から exact / model 並列実行へ変更した
- exact Python wrapper は PyO3 `Python::detach` で GIL を解放して呼び出すようにした
- exact は `timeout_seconds` 内で成功した場合だけ exact を返す
- exact が timeout / failure した時点で model 側に有効結果があれば、その場で model 結果を返す
- `timeout_reached` は exact timeout / failure 後の model fallback でも `True` にする

## 検証結果

- `uv run pytest -q src/test_python_api.py -k "select_move_with_model or search_best_move_exact"`: `13 passed`
- `make check`: 成功
  - Rust `161 passed; 0 failed; 14 ignored`
  - Python `70 passed`

## 懸念点

### exact と model が本当に並列にならない

- 懸念:
  - exact 探索呼び出しが GIL を保持したままだと、Python 側 model 推論と実質直列になる
- 解決策:
  - PyO3 wrapper で exact 呼び出しを `Python::detach` 下で動かす
  - 実装後に timeout 付きテストで挙動を確認する

### exact を待ちすぎて model 結果を返せない

- 懸念:
  - exact 優先を厳格にしすぎると、制限時間内に model 結果があっても返せない
- 解決策:
  - exact 優先は「全体締切前に exact 成功した場合」に限定する
  - 全体締切時点で exact が未完 / failure でも model 結果があれば model を返す

### model fallback 候補の条件が曖昧

- 懸念:
  - model 側が失敗や着手なし結果でも、そのまま fallback 候補にすると意味がぶれる
- 解決策:
  - model 結果は `success = True` かつ `best_move is not None` のときだけ fallback 候補にする
  - 強制パスは従来どおり別経路で返す

### model 側の forward は途中中断できない

- 懸念:
  - 1 回の model forward 中は timeout で止められない
- 解決策:
  - この制約は Step 35 と同じく維持する
  - timeout は探索ノード間と root orchestration で扱う

### thread 安全性

- 懸念:
  - `model.eval()` / `model.train()` の切り替えと並列実行がぶつかる可能性がある
- 解決策:
  - 1 回の API 呼び出しの中では model 経路を 1 本に限定する
  - exact と model を並列化しても、model 自体は 1 thread からだけ呼ぶ
  - 並列実装は `ThreadPoolExecutor(max_workers=2)` に固定し、worker 数を増やさない

### exact timeout/failure と `timeout_reached` の意味が曖昧

- 懸念:
  - exact が timeout / failure して model fallback を返した場合、API 全体としては成功でも `timeout_reached` の意味がぶれる
- 解決策:
  - exact が timeout / failure して model fallback を返した場合は `timeout_reached = True` とする
  - exact 成功時は `timeout_reached = False` とする

### exact failure の扱いが timeout に偏っている

- 懸念:
  - timeout 以外の exact failure を model fallback に流せないと、終盤 API の頑健性が下がる
- 解決策:
  - exact の failure は timeout に限らず model fallback を許可する
  - 最終的な `source` は実際に返した経路の値を使う

### 並列開始条件が広すぎる

- 懸念:
  - `exact_from_empty_threshold` 以下で常に両方を走らせると、ごく浅い終盤では無駄が出る可能性がある
- 解決策:
  - Step 36 ではまず `exact_from_empty_threshold` 以下で一律並列とする
  - 無駄が目立つ場合は後続 step で閾値を分離する

### 実装位置が曖昧

- 懸念:
  - Python 側だけで並列化しても、exact wrapper が GIL を保持していると意味がない
- 解決策:
  - Python 側は orchestration に限定する
  - Rust / PyO3 側では exact 呼び出しを GIL 解放下で動かす
