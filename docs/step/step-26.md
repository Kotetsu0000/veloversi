# Step 26: PyTorch Dataset / DataLoader 例の再設計

## このステップの目的

`examples/pytorch_dataloader.py` を、PyTorch の map-style `Dataset` / `DataLoader` の基本契約に沿った形へ直す。

主目的は次の3つ。

- 1 index = 1 盤面サンプルにする
- `DataLoader(batch_size=...)` で可変 batch を自然に扱えるようにする
- value 学習用と policy 学習用の両方で使える例にする

## version 方針

- 学習支援基盤の拡張中は `0.0.1` を維持する
- example の設計修正は version を上げる節目にはしない

## このステップで行うこと

- `examples/pytorch_dataloader.py` を map-style dataset 前提で作り直す
- `Dataset.__getitem__` は 1 サンプルだけ返すようにする
- batch 化は `DataLoader` の `batch_size` と `collate_fn` に委ねる
- `board` / `recording` の現在局面から直接モデル入力を作る正式 API を追加する
- value 学習用の DataLoader 例を追加 / 整理する
- policy 学習用の DataLoader 例を追加 / 整理する
- README / `examples/README.md` を必要最小限更新する

## このステップの対象範囲

### 対象

- `examples/pytorch_dataloader.py`
- `examples/README.md`
- `README.md`

### 対象外

- 学習ループ本体
- モデル推論 API
- parquet / Arrow / HDF5 など別保存形式
- Rust 側の feature / batch API 自体の仕様変更

## 固定した前提

- 1 index = 1 盤面サンプル
- `Dataset` は map-style dataset にする
- `__len__` と `__getitem__` を備える
- `batch_size` は `DataLoader` 側で可変にする
- batch 化は `collate_fn` で行う
- `prepare_planes_learning_batch` / `prepare_flat_learning_batch` は `collate_fn` 内でまとめて呼ぶ
- value 用と policy 用は example 上で分けて見せる
- feature は少なくとも次の2系統を扱う
  - planes: `(B, C, 8, 8)`
  - flat: `(B, F)`
- 保存データは生の `final_margin_from_black` を保持し、value target の正規化は DataLoader / 学習入力 helper 側で行う
- CNN 用入力は Python API 本体に追加し、shape は `(B, 3, 8, 8)` に固定する
  - 自分の石
  - 相手の石
  - 合法手
- flat/NNUE 風入力も Python API 本体に追加する
- 単発推論や手元確認向けに、`board` または `recording` の現在局面から
  - CNN 用入力
  - flat/NNUE 風入力
  を返す正式 API を用意する

## 受け入れ条件

- [ ] `Dataset.__getitem__` が 1 サンプルだけ返す
- [ ] `DataLoader(batch_size=N)` を変えても同じ dataset を使える
- [ ] `collate_fn` で batch 化している
- [ ] value 用 example がある
- [ ] policy 用 example がある
- [ ] README / examples README が現状と一致する

## 実装方針

- 保存済み JSONL は 1 レコード = 1 サンプルとして扱う
- `Dataset.__getitem__` は生の record `dict` を返す
- `collate_fn` が `list[dict]` をまとめて `prepare_*_learning_batch` へ渡す
- value 用 / policy 用で返す項目は分ける
- `legal_move_masks` は policy 用で明示的に返す
- `B == 1` でも同じ DataLoader / collate_fn を使う
- value target の正規化は `collate_fn` 側で行う
- `board | recording` を受けるモデル入力 API は convenience helper ではなく公開 API として定義する
- `recording` を受けた場合は現在局面を使う

## PyTorch 公式情報との対応

- map-style dataset は `__getitem__()` と `__len__()` を実装し、index から 1 サンプルを返す  
  参考: `torch.utils.data` docs
- `DataLoader` は `batch_size` と `collate_fn` で自動 batch 化する  
  参考: `torch.utils.data` docs
- default `collate_fn` は辞書構造を保ったまま batch 化するが、このライブラリでは feature 化を伴うため custom `collate_fn` を使う

## 現行 example の問題点

- `__getitem__` の中で `prepare_planes_learning_batch([record], ...)` を呼んでおり、実質的に 1 サンプルずつ疑似 batch 化している
- `DataLoader` の `batch_size` は、その後 `default_collate` が積むだけなので、feature 生成責務の位置が不自然
- value 用 / policy 用の責務分離が弱い
- `legal_move_masks` を出していない
- 単発のモデル入力 API が無く、`board` や `recording` からそのまま PyTorch 入力に繋げにくい

## 懸念点

- `collate_fn` に feature 生成を寄せると example は正しくなる一方、利用者に「どこで重い処理が走るか」を明示する必要がある
- value 用と policy 用を1つの example に詰め込みすぎると読みづらい
- `history_len > 0` は Step 24 未完了なので、この step では扱わない前提を明記する必要がある
- `recording` を直接受ける正式 API を入れるため、`board` と `recording` の判定規約を明確にする必要がある
