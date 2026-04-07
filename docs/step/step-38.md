# Step 38: SIMD-friendly Rust Value Model with PyTorch Definition

## このステップの目的

PyTorch で学習しつつ、実運用では Rust 側で高速推論できる value モデル導線を追加する。

目標は次の 2 系統を揃えること:

- Python / PyTorch 側
  - `vv.model.NNUE()` で学習用モデル定義を取得できる
  - `load_state_dict(...)` で通常の PyTorch 学習フローに乗る
- Rust 推論側
  - `vv.load_model("model_weights.pth")` で学習済み重みを読み込める
  - `board.select_move_with_model(vv_model, ...)` で Rust 側推論を使える

## version 方針

- このステップは `0.2.x` の patch より大きい
- Step 38 を含めて release する場合は `0.3.0` を推奨する

## このステップで行うこと

### Phase 1: モデル仕様固定

- flat 入力のみ
- value 出力のみ
- SIMD に乗せやすい固定レイアウトの MLP とする
- 厳密な疎 NNUE ではなく、NNUE を意識した「高速 value evaluator」として扱う

想定アーキテクチャ候補:

- input: `192`
- hidden1: `256`
- hidden2: `32`
- output: `1`
- 活性化: `ReLU`

### Phase 2: PyTorch モデル定義

- `veloversi.model` サブモジュールを追加する
- `vv.model.NNUE()` で PyTorch `nn.Module` を返す
- `torch` は package dependency にしない
- `veloversi.model` 利用時のみ optional runtime import にする

### Phase 3: Rust 推論モデル

- Rust 側に固定構造の value model を実装する
- 重みは連続メモリに保持する
- 推論は CPU / SIMD friendly な実装にする
- Python には `RustValueModel` 相当の公開型を出す

### Phase 4: state_dict 読み込み

- `vv.load_model("model_weights.pth")` を追加する
- `torch` が利用可能な環境では `torch.load(...)` で state_dict を読む
- key 名と shape を固定し、合わない場合は error にする
- 読み込んだ重みを Rust 側モデルへコピーする

### Phase 5: 既存 API との接続

- `select_move_with_model(...)` が
  - PyTorch `nn.Module`
  - Rust value model
  の両方を受けられるようにする
- Rust value model の場合は Python 側で Tensor を作らず、Rust 側推論を使う

### Phase 6: 文書と example

- PyTorch 学習側の最小例
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
- 量子化形式の確定 export
- true sparse NNUE の実装

## 固定した前提

- 入力は flat `(1, 192)` / `(B, 192)`
- 出力は value のみ
- value は現在手番視点
- Rust 推論器は PyTorch `state_dict` 互換の固定 key/shape を前提にする
- `torch` は package dependency にしない
- `load_model(...)` と `veloversi.model` 利用時のみ optional runtime import する
- `load_model(...)` は CPU 読み込みを前提にする
  - GPU 保存済み `state_dict` でも `map_location="cpu"` で読み込む
- dtype は `float32` に固定する
- `select_move_with_model(...)` に Rust value model を渡した場合、`device` 引数は無視する
  - Rust 推論は CPU で実行する

## 受け入れ条件

- [ ] `vv.model.NNUE()` が学習用の PyTorch モデル定義として使える
- [ ] `vv.load_model("model_weights.pth")` で Rust 推論モデルを読み込める
- [ ] Rust 推論モデルを `select_move_with_model(...)` に渡せる
- [ ] Python/Torch 推論と Rust 推論で同じ重みに対して近い value が出る
- [ ] README / examples / stub が更新されている
- [ ] `make check` が成功する

## 懸念点

### `.pth` / `state_dict` の読み込み経路

- 懸念:
  - PyTorch の保存形式は `torch` 非導入環境では直接読めない
- 解決策:
  - `load_model(...)` 自体は optional runtime import で `torch` を使う
  - package dependency には追加しない
  - `torch` が無い環境では明確な error を返す

### `torch.load(...)` の安全性と互換性

- 懸念:
  - `.pth` は pickle ベースであり、安全でないファイルをそのまま読むのは危険
  - GPU 保存済み重みもそのままでは CPU 環境で扱いづらい
- 解決策:
  - `load_model(...)` は state_dict 前提で読む
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
