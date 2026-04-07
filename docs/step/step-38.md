# Step 38: SIMD-friendly Rust Value Model with PyTorch Definition

## このステップの目的

PyTorch で学習しつつ、実運用では Rust 側で高速推論できる value モデル導線を追加する。

目標は次の 2 系統を揃えること:

- Python / PyTorch 側
  - `vv.model.NNUE()` で学習用モデル定義を取得できる
  - `load_state_dict(...)` で通常の PyTorch 学習フローに乗る
- Rust 推論側
  - `vv.export_model("model_weights.pth", "model_weights.vvm")` で学習済み重みを Rust 推論用形式へ変換できる
  - `vv.load_model("model_weights.vvm")` で Rust 推論モデルを読み込める
  - `board.select_move_with_model(vv_model, ...)` で Rust 側推論を使える

## version 方針

- このステップは `0.2.x` の patch より大きい
- Step 38 を含めて release する場合は `0.3.0` を推奨する

## このステップで行うこと

### Phase 1: モデル仕様固定

- 専用 flat 入力のみ
- value 出力のみ
- SIMD に乗せやすい固定レイアウトのモデルとする
- 厳密な疎 NNUE ではなく、NNUE を意識した「高速 value evaluator」として扱う
- Step 38 第1版は `int8` 前提とし、2 値化は後段に回す
- 汎用 `(192,)` をそのまま使うのではなく、推論専用の flat 特徴を別途設計する
- その特徴は `coord -> affected features` を持てる形にして、将来の差分更新や accumulator 化に繋げられるようにする
- `ref/Egaroucid` の pattern / incremental update / SIMD layout を、特徴設計とレイアウト設計の参考として使う
- 学習モデルと推論モデルは同一物でなくてよい
  - 学習時に扱いやすい PyTorch モデル
  - export 時に圧縮・量子化・packing された Rust 推論モデル
  の 2 段階を前提にする
- 専用 flat 特徴は利用者が直接取得できるようにする
  - `Board`
  - `RecordedBoard`
  - `RecordDataset`
  のすべてから取得可能にする

想定 API:

- `board.prepare_nnue_model_input()`
- `record.prepare_nnue_model_input()`
- `dataset.get_nnue_input(global_index)`

具体的な特徴量仕様:

- 盤面は常に「現在手番視点」に正規化する
  - current player の石
  - opponent の石
  - empty
  の 3 値で扱う
- `ref/Egaroucid` の 16 pattern family を採用する
  - `hv2`
  - `d6 + 2C + X`
  - `hv3`
  - `d7 + 2corner`
  - `hv4`
  - `corner9`
  - `d5 + 2X`
  - `d8 + 2C`
  - `edge + 2x`
  - `triangle`
  - `corner + block`
  - `cross`
  - `edge + y`
  - `narrow triangle`
  - `fish`
  - `anvil`
- 各 family は 4 方向/対称の oriented feature を持ち、合計 64 pattern feature とする
- 各 oriented feature は ternary pattern index を 1 つだけ持つ
  - cell state は `empty=0`, `self=1`, `opp=2`
  - index は `sum(state_i * 3^i)` で計算する
- 追加の scalar bucket feature を持つ
  - `empty_count`
  - `self_mobility`
  - `opp_mobility`
- 第1版の `NNUE` 入力は
  - `pattern_indices[64]`
  - `scalar_buckets[3]`
  の合計 67 要素の整数入力とする
- `Board.prepare_nnue_model_input()` / `record.prepare_nnue_model_input()` は `(1, 67)` の `np.ndarray(int32)` を返す
- `dataset.get_nnue_input(global_index)` は `(67,)` の `np.ndarray(int32)` を返す
- 学習用 PyTorch モデルは dense float 入力ではなく、index 入力を受ける embedding-sum 型とする
  - 64 個の pattern embedding
  - 3 個の scalar bucket embedding
  を加算して accumulator を作る
- Rust 推論モデルも同じ index 入力を受ける
  - export 時に embedding table を `int8` / packed 形式へ変換する
  - 将来的な差分更新のため、`coord -> affected pattern slots` を事前計算する

想定アーキテクチャ候補:

- input: `pattern index 64 + scalar bucket 3`
- accumulator: SIMD 幅に揃えた固定次元
- hidden: 同上
- output: `1`
- 活性化: `ReLU` または clipped `ReLU`

### Phase 2: PyTorch モデル定義

- `veloversi.model` サブモジュールを追加する
- `vv.model.NNUE()` で PyTorch `nn.Module` を返す
- `torch` は package dependency にしない
- `veloversi.model` 利用時のみ optional runtime import にする
- 学習しやすさのため、必要なら次も同時に提供する
  - fake quantization / binarization layer
  - straight-through estimator 前提の補助関数
  - export 用の weight projection helper
  - 必要な regularization / auxiliary loss helper
- 学習済み PyTorch model 自体の読み込みは通常どおり
  - `model = vv.model.NNUE()`
  - `model.load_state_dict(torch.load("model_weights.pth"))`
  で行う

### Phase 3: Rust 推論モデル

- Rust 側に固定構造の value model を実装する
- 重みは連続メモリに保持する
- 推論は CPU / SIMD friendly な実装にする
- Python には `RustValueModel` 相当の公開型を出す

### Phase 4: export / load 導線

- `vv.export_model("model_weights.pth", "model_weights.vvm")` を追加する
- `vv.export_model(...)` は
  - `vv.model.NNUE()` 用に保存した `state_dict` (`.pth`)
  を読み、
  - 量子化
  - packing
  - 必要な metadata 付与
  を行った上で Rust 推論用の軽量形式 (`.vvm`) を出力する
- `vv.load_model("model_weights.vvm")` を追加する
- `vv.load_model(...)` は PyTorch model を返さない
  - 返すのは Rust 側で高速推論するための value model
- key 名と shape を固定し、合わない場合は error にする
- export 時に重みを Rust 側モデルが読める連続形式へ変換する

### Phase 5: 既存 API との接続

- `select_move_with_model(...)` が
  - PyTorch `nn.Module`
  - Rust value model
  の両方を受けられるようにする
- Rust value model の場合は Python 側で Tensor を作らず、Rust 側推論を使う
- 専用 flat 特徴取得 API は既存の
  - `prepare_cnn_model_input`
  - `prepare_flat_model_input`
  - `get_cnn_input`
  - `get_flat_input`
  と同じ操作感で使えるようにする

### Phase 6: 文書と example

- PyTorch 学習側の最小例
- `export_model(...)` の最小例
- `load_model(...)` の最小例
- `board.select_move_with_model(vv_model, ...)` の最小例
- README / stub / docstring 更新

## 対象範囲

### 対象

- `src/veloversi/model.py` または同等の Python モジュール
- `src/veloversi/__init__.py`
- `src/veloversi/__init__.pyi`
- `src/veloversi/_core.pyi`
- Rust 側の新規 model 実装ファイル
- `src/python.rs`
- `src/lib.rs`
- `src/test_python_api.py`
- `README.md`
- `examples/`
- `docs/step/step-38.md`
- `docs/step/todo.md`

### 対象外

- policy 出力付きモデル
- GPU 専用高速化
- true sparse NNUE の実装
- 2 値化モデルの本実装

## 固定した前提

- 入力は flat `(1, 192)` / `(B, 192)`
- 出力は value のみ
- value は現在手番視点
- Rust 推論器は PyTorch `state_dict` 互換の固定 key/shape を前提にする
- `torch` は package dependency にしない
- `load_model(...)` と `veloversi.model` 利用時のみ optional runtime import する
- `export_model(...)` は CPU 読み込みを前提にする
  - GPU 保存済み `state_dict` でも `map_location="cpu"` で読む
- Step 38 第1版の Rust 推論モデルは `int8` 重みを前提にする
- 4bit / 2bit / 1bit 変換はこのステップでは扱わない
- PyTorch 学習側は `float32` を基本としつつ、export 規約に合わせた fake quantization を許容する
- export 形式内では packed / quantized 表現を前提にする
- `select_move_with_model(...)` に Rust value model を渡した場合、`device` 引数は無視する
  - Rust 推論は CPU で実行する
- `vv.export_model("model_weights.pth", "model_weights.vvm")` は
  - `vv.model.NNUE()` と同じ key/shape の `state_dict`
  を入力にする
- `vv.load_model("model_weights.vvm")` は
  - `torch` 不要
  - Rust 推論モデルを返す
- `select_move_with_model(...)` は model 種別を先に判定する
  - Rust value model の場合は `torch` を import しない
- Rust value model の推論経路は、Step 39 の iterative mode でも再利用できる共通 adapter に切り出す
- 専用 flat 特徴は学習時と Rust 推論時で順序と意味を完全一致させる
- 専用 flat 特徴は `coord -> affected features` を持ち、将来の差分更新に備える
- export は「PyTorch 学習モデル -> Rust 推論モデルへの変換フェーズ」として扱う
- 圧倒的な高速演算を優先し、PyTorch 側の都合より Rust 側の推論効率を設計の主軸にする
- 専用 flat 特徴は `Board` / `RecordedBoard` / `RecordDataset` から同系統の API 名で取得できるようにする

## 受け入れ条件

- [ ] `vv.model.NNUE()` が学習用の PyTorch モデル定義として使える
- [ ] `vv.export_model("model_weights.pth", "model_weights.vvm")` で Rust 推論用形式へ変換できる
- [ ] `vv.load_model("model_weights.vvm")` で Rust 推論モデルを読み込める
- [ ] 専用 flat 特徴を `Board` / `RecordedBoard` / `RecordDataset` から取得できる
- [ ] Rust 推論モデルを `select_move_with_model(...)` に渡せる
- [ ] Python/Torch 推論と Rust 推論で同じ重みに対して近い value が出る
- [ ] README / examples / stub が更新されている
- [ ] `make check` が成功する

## 懸念点

### `.pth` / `state_dict` の読み込み経路

- 懸念:
  - `.pth` を推論時に毎回読む設計だと、推論専用環境でも `torch` が必要になる
  - また、`vv.load_model(...)` が PyTorch model を返す API と誤解されやすい
- 解決策:
  - `.pth` 読み込みは `vv.export_model(...)` に分離する
  - `vv.load_model(...)` は Rust 推論用形式だけを読む API にする
  - PyTorch model 自体は `vv.model.NNUE()` + `load_state_dict(torch.load(...))` で扱うと明記する
  - `export_model(...)` 自体は optional runtime import で `torch` を使う
  - package dependency には追加しない
  - `torch` が無い環境では明確な error を返す

### `torch.load(...)` の安全性と互換性

- 懸念:
  - `.pth` は pickle ベースであり、安全でないファイルをそのまま読むのは危険
  - GPU 保存済み重みもそのままでは CPU 環境で扱いづらい
- 解決策:
  - `export_model(...)` は state_dict 前提で読む
  - 可能な環境では `weights_only=True` を使う
  - 常に `map_location="cpu"` で読み込む
  - README に trusted な重みファイルだけを対象にすることを明記する

### モデル形状の曖昧さ

- 懸念:
  - 任意の `state_dict` を許すと Rust 側推論器を安定実装できない
- 解決策:
  - key 名と shape を固定する
  - 違う形は読み込み error にする

### `select_move_with_model(...)` の入力型が増える

- 懸念:
  - PyTorch `nn.Module` と Rust value model の両対応で判定が曖昧になる
- 解決策:
  - Python 側で型を明示判定する
  - PyTorch `nn.Module`
  - Rust value model
  の 2 種だけを受ける

### Rust value model が既存の `torch` 前提コードと衝突する

- 懸念:
  - 現在の `select_move_with_model(...)` は PyTorch `nn.Module` を前提に `torch` を遅延 import している
  - Rust value model 追加後も同じ流れだと、Rust model を使うだけで `torch` が必要になってしまう
- 解決策:
  - model 種別の判定を `torch` import より前に行う
  - Rust value model 経路では `torch` を一切使わない
  - `torch` 必須なのは PyTorch `nn.Module` 経路と `load_model(...)` だけに限定する

### Rust value model を Python の background thread から安全に使える必要がある

- 懸念:
  - Step 36/37 の `select_move_with_model(...)` は `ThreadPoolExecutor` で並列実行する
  - Rust value model が immutable / thread-safe に使えないと、この経路に載せられない
- 解決策:
  - Rust value model は immutable な重みコンテナとして設計する
  - Python からは pyclass として扱いつつ、推論はその object を background thread から読める形にする
  - 並列 exact/model 経路でも使えることをテストで固定する

### `select_move_with_model(...)` の value 評価実装を二重管理しやすい

- 懸念:
  - PyTorch `nn.Module` 用と Rust value model 用で別々に探索コードを増やすと、Step 39 の iterative mode 追加時に分岐が爆発する
- 解決策:
  - 「1 局面を value にする evaluator adapter」を共通化する
  - 探索本体は既存の value search 実装を再利用し、model 種別ごとの差は evaluator 生成で吸収する

### `state_dict` の key 名が学習環境依存で揺れる

- 懸念:
  - DataParallel / DDP 経由の保存では key に `module.` prefix が付くことがある
  - これを完全拒否すると、運用上かなり不便になる
- 解決策:
  - 固定 key/shape は維持しつつ、先頭の `module.` prefix だけは許容して正規化する
  - それ以外の key 差異は error にする

### Rust 推論用形式の将来互換性

- 懸念:
  - `.vvm` 形式を設計せずに書き出すと、将来のモデル構造変更で読み込み互換が壊れる
- 解決策:
  - `.vvm` に version / architecture / dtype / shape 情報を持たせる
  - `load_model(...)` は header を検証し、不一致なら明確な error を返す

### 量子化 / 2値化モデルで学習時と推論時の挙動がずれる

- 懸念:
  - 重みや活性を 2 値 / 低 bit 幅にすると、PyTorch 学習時の内部表現と Rust 推論時の離散表現が一致しない
  - そのままでは、学習できても export 後に別モデルになる
- 解決策:
  - export 時の離散化規約を先に固定する
  - PyTorch 側でも同じ規約を使う fake quantization / binarization を提供する
  - backward は straight-through estimator 前提で扱う
  - export 前の PyTorch 推論結果と、export 後 Rust 推論結果の差を直接比較するテストを追加する

### `state_dict` だけでは離散化パラメータが足りない可能性がある

- 懸念:
  - 2 値 / 低 bit 幅モデルでは、scale、zero-point、threshold、packing 規約などが必要になることがある
  - これを `state_dict` の重みだけで表せないと export が曖昧になる
- 解決策:
  - 必要な離散化パラメータは model buffer / metadata として明示的に持たせる
  - `export_model(...)` は weight だけでなく、その metadata も検証して `.vvm` に埋め込む

### 推論専用特徴が学習側で扱いにくい

- 懸念:
  - SIMD 最適化を優先した専用 flat 特徴は、通常の MLP 学習より扱いにくくなる可能性がある
- 解決策:
  - PyTorch 側に専用特徴へ合わせたモデル定義を用意する
  - 必要なら fake quantization / 補助 loss / projection helper を追加する
  - ただし feature 定義自体は Rust 推論効率を優先して固定する

### `ref` を参考にした特徴設計が中途半端になる

- 懸念:
  - 汎用 192 次元と pattern 特徴の中間のような曖昧な設計にすると、学習も推論も中途半端になる
- 解決策:
  - `ref/Egaroucid` の
    - pattern feature の固定長設計
    - `coord -> feature` の事前計算
    - 着手ごとの差分更新
    - SIMD 用 packing
  の発想を使い、専用 flat 特徴を最初から推論側主導で設計する

### SIMD 最適化を優先しすぎると PyTorch 側の学習性が落ちる

- 懸念:
  - 推論向けに特殊化しすぎると、勾配が流れにくくなり学習が不安定になる
- 解決策:
  - Step 38 では「推論で高速に扱えること」を最優先にしつつ、
    学習側には補助 layer / helper / loss を用意して成立させる
  - その上で、必要なら特殊化の度合いを段階的に強める

### SIMD 実装の範囲

- 懸念:
  - 最初から AVX2/SSE2/generic 全部を最適化するとスコープが広すぎる
- 解決策:
  - まずは generic + 連続メモリ前提で正しい Rust 推論器を作る
  - その上で hot path を SIMD 化する

### 学習時の flat 入力と Rust 推論時の flat 入力がずれる

- 懸念:
  - `prepare_flat_model_input()` の要素順と、PyTorch 学習で想定する `NNUE` 入力順がずれると重み互換が崩れる
- 解決策:
  - flat 入力順を仕様として固定する
  - PyTorch モデルと Rust 推論器の両方で同じ順序を使う
  - 同一重み・同一入力で近い value が出る比較テストを追加する

### 専用 flat 特徴が board / record / dataset でずれる

- 懸念:
  - `Board`、`RecordedBoard`、`RecordDataset` で別々に特徴生成すると、要素順や符号規約がずれる危険がある
- 解決策:
  - 専用 flat 特徴の生成は 1 箇所に集約する
  - `RecordedBoard` は current board を流すだけにする
  - `RecordDataset` は復元した `Board` に同じ実装を適用するだけにする
  - 固定局面で 3 経路の出力一致テストを追加する

### PyTorch 推論と Rust 推論の数値差

- 懸念:
  - `ReLU` や行列積の実装差で、完全一致しない可能性がある
- 解決策:
  - 完全一致は要求せず、`float32` 前提の許容誤差を決める
  - テストでは固定入力に対して最大誤差/平均誤差を確認する

### `veloversi.model` が通常 import を壊す

- 懸念:
  - `veloversi.model` を package import 時に eager import すると、`torch` 非導入環境でも `import veloversi` が失敗しうる
- 解決策:
  - `veloversi.model` は遅延 import にする
  - `import veloversi` 自体は `torch` 不要を維持する
