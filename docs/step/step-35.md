# Step 35: PyTorch Model-Driven Move Selection API

## このステップの目的

学習済みの PyTorch `nn.Module` を受け取り、`Board` / `RecordedBoard` から着手を返せる公開 API を追加する。

主目的は次の 7 つ。

- PyTorch `nn.Module` を受ける着手選択 API を追加する
- `torch` を package dependency に追加せず、runtime でのみ遅延 import する
- model の入力形式を自動判別する
  - CNN 入力
  - flat 入力
- model の出力形式を自動判別する
  - policy
  - value
- policy / value の両経路で timeout を考慮した結果を返す
- `Board` / `RecordedBoard` / module-level の 3 経路で同じように使えるようにする
- 終盤では model ベース探索から exact 探索へ安全に切り替えられるようにする

## version 方針

- このステップ中は `0.2.3` を維持する
- このステップ完了後に、必要なら `0.2.x` の次 patch release を切る

## このステップで行うこと

### Phase 1: 公開 API の追加

- module-level にモデル駆動着手選択 API を追加する
- `Board` に method-style API を追加する
- `RecordedBoard` に同名 method を追加する

想定 API:

- `select_move_with_model(board_or_record, model, depth=1, timeout_seconds=1.0, policy_mode="best", device="cpu", exact_from_empty_threshold=None)`
- `board.select_move_with_model(model, depth=1, timeout_seconds=1.0, policy_mode="best", device="cpu", exact_from_empty_threshold=None)`
- `record.select_move_with_model(model, depth=1, timeout_seconds=1.0, policy_mode="best", device="cpu", exact_from_empty_threshold=None)`

### Phase 2: 入力形式の自動判別

- model に対して CNN 入力と flat 入力のどちらを使うか判別する
- 判別できない場合は error を返す
- 入力テンソルは `device` に移す

固定する入力候補:

- CNN:
  - `prepare_cnn_model_input(...)`
  - shape `(1, 3, 8, 8)`
- flat:
  - `prepare_flat_model_input(...)`
  - shape `(1, 192)`

### Phase 3: 出力形式の自動判別

- model 出力が policy か value かを判別する
- 出力形式は既存学習 API と整合させる

固定する出力候補:

- policy:
  - shape `(64,)` または `(1, 64)`
- value:
  - scalar または shape `(1,)`

### Phase 4: policy 経路

- policy 出力時は合法手に制限した上で着手を決める
- `policy_mode="best"` のときは argmax を返す
- `policy_mode="sample"` のときは確率分布に従って返す
- 出力が確率分布でない場合は合法手の値で softmax をかける

### Phase 5: value 経路

- value 出力時は `depth` まで探索する
- timeout 到達時は、その時点での最善手を返す
- value の視点は現在手番視点に固定する
- `exact_from_empty_threshold` 以下の空き数では `search_best_move_exact` に切り替える
- exact 経路も `timeout_seconds` を共有で使う

### Phase 6: 文書と検証

- README / docstring / 型 stub を更新する
- Python テストを追加する
- `make check` を通す

## このステップの対象範囲

### 対象

- `src/python.rs`
- `src/veloversi/__init__.py`
- `src/veloversi/__init__.pyi`
- `src/veloversi/_core.pyi`
- `src/test_python_api.py`
- `README.md`
- `docs/step/step-35.md`
- `docs/step/todo.md`

### 対象外

- `torch` の package dependency 化
- generic callable (`numpy -> numpy`) の対応
- 学習ループや optimizer の提供
- midgame heuristic search の改善

## 固定した前提

- `model` は PyTorch `nn.Module` とする
- `torch` は依存に入れず、この API 内でのみ遅延 import する
- `torch` が import できない場合は、PyTorch model 経路は error にする
- `device` 引数を持ち、既定値は `cpu`
- generic callable (`numpy -> numpy`) は対象外
- policy 出力は 64 マスのみを扱い、pass は対象外
- value 出力は現在手番視点の value として扱う
- `RecordedBoard` は常に `current_board` を探索対象にする
- 強制パス局面では model を呼ばず、着手なし結果を返す
- exact への切り替え条件は「手数」ではなく空きマス数で持つ
  - `Board` / `RecordedBoard` 単体ではゲーム全体の手数は一意に定まらないため

## 受け入れ条件

- [x] module-level `select_move_with_model(...)` が使える
- [x] `Board.select_move_with_model(...)` が使える
- [x] `RecordedBoard.select_move_with_model(...)` が使える
- [x] `torch` 非導入環境では分かりやすい error を返す
- [x] CNN / flat の入力形式を自動判別できる
- [x] policy / value の出力形式を自動判別できる
- [x] `policy_mode="best"` と `policy_mode="sample"` の両方が動く
- [x] value 出力時に `depth` と `timeout_seconds` が効く
- [x] `exact_from_empty_threshold` 以下で exact 探索へ切り替えられる
- [x] README / docstring / 型 stub が更新されている
- [x] `make check` が成功する

## 実装方針

- まず `torch` を遅延 import する薄い Python helper を用意する
- model 入力は `Board` / `RecordedBoard` から既存の model input API で作る
- 出力形式の自動判別は shape ベースで行う
- `torch.no_grad()` で推論する
- `model.eval()` は一時的に有効化し、終了後に元の training 状態へ戻す
- policy 経路は合法手 mask を必ず適用する
- value 経路は Python 側で深さ優先探索を行い、model を葉評価として使う
- timeout 時は value 経路のみ partial best result を返す
- policy 経路は 1 回推論なので timeout は model 呼び出し全体に対して扱う
- 強制パス局面では model を呼ばずに固定結果を返す
- 終盤は `search_best_move_exact` を優先し、model 探索より正しさを優先する

## 懸念点

### `torch` 非導入環境での利用

- 懸念:
  - この API は PyTorch `nn.Module` 専用なので、`torch` が無い環境では呼べない
- 解決策:
  - API の内部でのみ遅延 import する
  - `torch` が無い場合は、この API は利用不可であることを明確な error で返す

### 入力形式の自動判別が曖昧になる

- 懸念:
  - CNN / flat の両方で model 呼び出しを試すだけだと、副作用や不要な例外が混ざりやすい
- 解決策:
  - 入力 shape の適合性を優先して判定する
  - 失敗した場合の例外を整理し、最終的に「どちらにも合わない」ことを明示する

### 出力形式の自動判別が曖昧になる

- 懸念:
  - model 出力が `(1, 64)` / `(64,)` / scalar / `(1,)` 以外だと解釈が不安定になる
- 解決策:
  - 許容 shape を固定する
  - それ以外は error にする

### policy 出力の確率分布判定

- 懸念:
  - 出力が logits か確率かの判定を雑にすると、softmax を二重にかける可能性がある
- 解決策:
  - 合法手部分の総和が 1 に十分近く、かつ非負である場合だけ確率分布とみなす
  - それ以外は合法手部分に softmax を適用する

### 強制パス局面

- 懸念:
  - policy 出力は pass を持たないため、合法手が 0 の局面で model 出力をどう扱うかを決める必要がある
- 解決策:
  - 強制パス局面では model を呼ばない
  - `best_move = None` の着手なし結果を返す仕様に固定する

### value 探索の timeout

- 懸念:
  - Python 側探索は timeout 管理を雑にすると、深い探索で戻りが悪くなる
- 解決策:
  - 再帰入口で deadline を確認する
  - timeout 到達時は、その時点の最善結果を返す方針を固定する

### 1 回の model forward 中は timeout で止められない

- 懸念:
  - timeout は探索ノード間では確認できるが、1 回の `model(...)` 実行中は中断できない
- 解決策:
  - timeout は探索ノード間で確認する仕様にする
  - 1 回の forward の中断までは保証しないことを明記する

### `model.eval()` と `torch.no_grad()` の扱い

- 懸念:
  - training mode のまま推論すると dropout / batchnorm で結果が不安定になる
- 解決策:
  - API 内部では `torch.no_grad()` を使う
  - `model.eval()` は一時的に有効化し、終了後に元の training 状態へ戻す

### exact 探索への切り替え条件

- 懸念:
  - 「特定の手数から exact 探索」とすると、`Board` / `RecordedBoard` 単体では開始位置によって意味がずれる
- 解決策:
  - 切り替え条件は手数ではなく空きマス数にする
  - `exact_from_empty_threshold` を公開引数にする
  - timeout は API 全体で `timeout_seconds` を共有する

## 実装結果

- Python 公開 API に `select_move_with_model(...)` を追加した
- `Board.select_move_with_model(...)` と `RecordedBoard.select_move_with_model(...)` を追加した
- `torch` は API 内でのみ遅延 import し、未導入環境では明確な error を返すようにした
- 入力形式は root 盤面に対して試行し、CNN / flat のどちらか 1 つに確定する実装にした
  - 両方通る model は曖昧として error にする
- 出力形式は shape ベースで policy / value を判別する
  - policy: `(64,)` / `(1, 64)`
  - value: scalar / `(1,)` / `(1, 1)`
- policy 出力では合法手だけを対象にし、既に確率分布ならそのまま、そうでなければ softmax を適用する
- value 出力では Python 側の depth-limited negamax で探索し、timeout 時はその時点の最善手を返す
- 空き数が `exact_from_empty_threshold` 以下なら、まず `search_best_move_exact(...)` を試みる
  - exact 成功時は exact 結果を返す
  - exact timeout 時は、残り時間があれば model 探索へフォールバックする
- 強制パス局面では model を呼ばず、着手なし成功結果を返す
- `model.eval()` は一時的に有効化し、終了後に元の training 状態へ戻す
- `torch.no_grad()` で推論する

## 検証結果

- `uv run pytest -q src/test_python_api.py -k "select_move_with_model or search_best_move_exact"`: 成功
  - `11 passed`
- `make check`: 成功
  - Rust: `161 passed; 0 failed; 14 ignored`
  - Python: `68 passed`

### `RecordedBoard` の探索対象

- 懸念:
  - `record.select_move_with_model(...)` が record 全体を探索するように読める
- 解決策:
  - 常に `current_board` を対象にする
  - README / docstring で明示する
