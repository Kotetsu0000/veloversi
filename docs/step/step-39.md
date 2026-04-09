# Step 39: Iterative Deepening for Model-based Search

## このステップの目的

`select_move_with_model(...)` の value 探索は現在、

- 固定 depth
- root 手を順番に深く読む
- timeout 時は、その時点までに評価完了した手の最善結果を返す

という形になっている。

このステップでは、探索を anytime 化するモードを追加する。

具体的には、

- 既存の fixed-depth value 探索は維持
- その上で opt-in の iterative deepening mode を追加
- 1 手先から順に深さを伸ばす iterative deepening
- 各 iteration 完了時点で最善手を保持
- timeout 時は「最後に完了した深さ」の最善手を返す
- move ordering を改善して、次 iteration を効率化する

を実装する。

## version 方針

- このステップ中は `0.2.5` を維持する
- Step 38 とまとめて release する場合は `0.3.0` を推奨する

## このステップで行うこと

### Phase 1: value 探索モードの拡張

- `select_move_with_model(...)`
- `Board.select_move_with_model(...)`
- `RecordedBoard.select_move_with_model(...)`

に、value 探索の mode 引数を追加する。

想定は次の 2 モード:

- `search_mode="fixed"`
  - 現在の fixed-depth value 探索
- `search_mode="iterative"`
  - 反復深化で deadline まで掘る

`search_mode="fixed"` を既定値にし、既存挙動を維持する。
許容値は `"fixed"` と `"iterative"` の 2 つに固定し、それ以外は `ValueError` とする。

`search_mode="iterative"` の場合のみ、

- depth=1
- depth=2
- ...
- depth=`depth`

の順に反復する形へ変更する。

各 depth の探索が完了した時点で、その depth における最善手と評価値を保持する。

### Phase 2: timeout 時の返却規約整理

- timeout 時は「最後に完了した iteration」の結果を返す
- まだ depth=1 すら完了していなければ failure を返す
- `timeout_reached=True` は維持する
- `source="value_search"` は維持する
- 返り値には `completed_depth` を追加し、最後に完了した深さを明示する

この規約は `search_mode="iterative"` のときだけ適用する。
`search_mode="fixed"` は現在の挙動を維持する。

### Phase 3: move ordering 改善

- 次 iteration の root ordering に前回 iteration の最善手を使う
- policy 出力を持つ model の場合は、その順位も ordering に使う
- value-only model の場合は cheap heuristic を使う
  - 合法手数が少ない手を優先
  - corner 優先
  - 自然順

ordering の優先順は次で固定する。

1. 前回 iteration の best move
2. policy 出力がある場合は policy 降順
3. corner
4. 次局面の合法手数が少ない手
5. 自然順

### Phase 4: exact との接続維持

- `exact_from_empty_threshold`
- `always_try_exact`

の挙動は維持する。

exact/model 並列開始条件に入った場合、

- exact 経路は Step 36/37 の規約どおり
- model 側の value 探索だけを iterative deepening 化する

### Phase 5: 文書とテスト

- README / docstring / stub 更新
- fixed / iterative の 2 モードを明記
- 既定値が fixed であることを明記
- timeout 時に「最後に完了した深さ」の結果が返ることをテストで固定
- `make check` を通す

## 対象範囲

### 対象

- `src/veloversi/__init__.py`
- `src/veloversi/__init__.pyi`
- `src/veloversi/_core.pyi`
- `src/test_python_api.py`
- `README.md`
- `docs/step/step-39.md`
- `docs/step/todo.md`

### 対象外

- exact solver 自体の内部アルゴリズム変更
- Rust 側の通常探索器追加
- transposition table の新規導入
- true best-first / MCTS 系探索

## 固定した前提

- 対象は value 探索経路
- `policy` 出力経路は 1 回 forward ベースを維持する
- iterative deepening は opt-in mode とする
- 既定値は fixed-depth mode
- iterative mode でも最大探索深さは `depth` を上限にする
- `search_mode` は value 出力経路にのみ意味を持つ
  - policy 出力経路では探索しないため、挙動は変えない
- `search_mode="iterative"` のときだけ
  - timeout 時は「最後に完了した depth」の結果を返す
  - 途中 depth の partial subtree 結果は返却基準にしない
- `always_try_exact` / `exact_from_empty_threshold` の規約は維持する
- `search_mode` の許容値は `"fixed"` / `"iterative"` に固定する
- root ordering の優先順は
  - 前回 iteration の best move
  - policy 降順
  - corner
  - 次局面の合法手数が少ない手
  - 自然順
  の順に固定する

## 受け入れ条件

- [ ] value 探索に fixed / iterative の mode が追加されている
- [ ] 既定値は fixed で、既存挙動を維持している
- [ ] `search_mode="iterative"` で iterative deepening が動作する
- [ ] `search_mode="iterative"` の timeout 時に最後に完了した depth の結果を返す
- [ ] iterative result に `completed_depth` が含まれる
- [ ] 前回 iteration の最善手を次 iteration の ordering に再利用している
- [ ] exact/model 並列 fallback の既存仕様を壊していない
- [ ] README / docstring / stub が更新されている
- [ ] `make check` が成功する

## 懸念点

### iteration のオーバーヘッドで逆に遅くなる

- 懸念:
  - 毎回 depth=1 から読み直すため、深い固定 depth より無駄が増える可能性がある
- 解決策:
  - fixed-depth mode を残す
  - root ordering に前回 iteration の最善手を再利用する
  - policy 順や cheap heuristic を併用し、読み直しコストを抑える

### timeout 直前の iteration が未完だと結果が弱い

- 懸念:
  - 最後の iteration が途中まで進んでも返却に使えないため、時間を無駄にしたように見える
- 解決策:
  - 返却規約は「最後に完了した depth」に固定する
  - 代わりに浅い depth を確実に積み上げる設計を優先する
  - この規約は iterative mode に限定し、fixed mode は現行仕様を維持する

### `completed_depth` の返り値が曖昧になる

- 懸念:
  - fixed / iterative で返却 schema が変わると利用側が扱いづらい
- 解決策:
  - `completed_depth` は常に返す
  - fixed mode では `None`
  - iterative mode では最後に完了した深さを `int` で返す

### root ordering の quality が足りない

- 懸念:
  - ordering が弱いと iterative deepening の利点が出にくい
- 解決策:
  - 前回最善手を最優先
  - policy 出力がある場合は policy 順
  - value-only では合法手数が少ない手、corner、自然順を使う

### exact/model 並列時に model 側の反復深化が重い

- 懸念:
  - exact と model が並列の場面では、model 側の探索も重くなる
- 解決策:
  - exact/model 並列の規約は変えず、model 側だけ iterative deepening 化する
  - timeout は共有し、exact が成功したらそちらを優先する

### exact 並列時に iterative の途中結果を混ぜやすい

- 懸念:
  - `always_try_exact=True` では exact / model を並列で走らせるため、exact 成功時に model 側の途中結果を返してしまうと仕様が崩れる
- 解決策:
  - exact 成功時は常に exact を返す
  - model iterative の結果は exact failure / timeout 時だけ採用する
  - threshold 以下では従来どおり exact-only を維持する

### `search_mode` が policy 出力で曖昧になる

- 懸念:
  - policy model には探索 depth の概念がないため、`search_mode="iterative"` を指定しても意味がない
- 解決策:
  - `search_mode` は value 出力経路にのみ適用する
  - policy 出力では現行の 1 回 forward を維持し、README / docstring で no-op であることを明記する

### root ordering の実装が分岐しやすい

- 懸念:
  - fixed / iterative / torch / Rust model で root ordering を別々に書くと、挙動差と保守負荷が増える
- 解決策:
  - root ordering は専用 helper に切り出す
  - 前回 iteration の best move
  - policy 順
  - cheap heuristic
  を 1 箇所で扱う

### テスト不足で iterative 専用仕様が壊れやすい

- 懸念:
  - 現在の timeout テストは fixed mode 前提で、iterative の返却規約や Rust model 経路を固定できていない
- 解決策:
  - 少なくとも次を追加する
    - iterative で timeout 時に `completed_depth` が返る
    - depth=1 すら完了しない場合は failure
    - Rust value model でも iterative が動く
    - `always_try_exact=True` + iterative で exact 勝ち / model 勝ち
    - threshold 以下では iterative に行かず exact-only

### timeout は model の 1 回の forward 中には止められない

- 懸念:
  - iterative deepening にしても、各ノードの model forward 実行中は中断できない
  - 特に GPU 利用時は deadline をまたぎやすい
- 解決策:
  - timeout 判定は探索ノード間で行う
  - 1 回の forward 中断は保証しないことを docstring に明記する
  - `completed_depth` により、どこまで確定したかを返り値で判断できるようにする

### iteration 内では root の後半手が未探索のまま終わる可能性がある

- 懸念:
  - iterative deepening でも、各 iteration の内部では root 手を順に読むため、短い timeout では後半手が未探索になりうる
- 解決策:
  - 返却規約は「完了した iteration 単位」に固定する
  - ordering を強化し、前回最善手・policy 順・合法手数・corner を使って有望手を先に読む
  - 幅優先の保証まではこの step では行わない
