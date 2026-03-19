# オセロ AI 学習仕様書

## 1. 目的

本仕様書は、オセロ教師モデル学習および NNUE 蒸留の学習フロー、データ仕様、モデル仕様、評価仕様を定義する。

本書の対象は以下である。

- データ生成パイプライン
- 教師モデル学習
- 自己対局による反復強化
- NNUE 蒸留学習
- 重みエクスポート前提

アルゴリズムの細部やハイパーパラメータ最適化戦略は本仕様書の対象外とする。

## 2. プロジェクト構成

```text
python/
  common/
    config.py
    seed.py
    logging.py
    types.py
  data_pipeline/
    schemas.py
    generator.py
    replay_buffer.py
    exact_labeler.py
    dataset_writer.py
    dataset_reader.py
  train_teacher/
    model.py
    losses.py
    trainer.py
    evaluator.py
    selfplay.py
    checkpoint.py
  train_nnue/
    model.py
    losses.py
    trainer.py
    exporter.py
    quantize.py
```

Rust エンジンとの連携は `othello_rs` Python モジュールを通す。

## 3. データ仕様

### 3.1 盤面サンプル単位

1 レコードは 1 局面を表す。

```python
from dataclasses import dataclass
from typing import Optional

@dataclass
class PositionSample:
    black_bits: int
    white_bits: int
    side_to_move: int
    move_index: int
    legal_moves_mask: int
    history_black_bits: list[int]
    history_white_bits: list[int]
    policy_target: Optional[list[float]]
    policy_mask: list[int]
    margin_target: float
    wdl_target: list[float]
    source_kind: str
    exact_label: bool
    game_id: str
    ply_in_game: int
    symmetry_id: int
```

### 3.2 各フィールド説明

- `black_bits`, `white_bits`
  - 絶対色で保持する盤面
- `side_to_move`
  - 0 = black, 1 = white
- `move_index`
  - 初期局面から何手目か
- `legal_moves_mask`
  - 合法手位置ビットマスク
- `history_*`
  - 過去 H 局面の盤面履歴
- `policy_target`
  - 64 次元の教師分布。パス局面では `None` 可
- `policy_mask`
  - 非合法手を 0、合法手を 1 とする 64 次元マスク
- `margin_target`
  - 現局面の手番視点での最終石差を 64 で割った実数値
- `wdl_target`
  - `[p_win, p_draw, p_loss]`
- `source_kind`
  - `random`, `selfplay`, `solver_exact`, `teacher_search`
- `exact_label`
  - `margin_target` が完全読み由来かどうか
- `symmetry_id`
  - 対称変換 ID

### 3.3 データ保存形式

学習用データは以下の 2 系統を持つ。

- 中間保存: Parquet または Arrow
- 学習キャッシュ: `.pt`, `.npy`, `.npz`, memory-mapped binary

要件:
- 盤面ビット列をロスレスに保存できること
- `policy_target is None` を表現できること
- exact ラベルかどうかを保持できること

## 4. 特徴生成仕様

### 4.1 教師モデル入力

教師モデルの基本入力は `[batch, channel, height, width]` の `float32` テンソルとする。

デフォルト構成:

- 現在局面 self stones: 1
- 現在局面 opp stones: 1
- 履歴 H 局面 self stones: H
- 履歴 H 局面 opp stones: H
- legal mask: 1
- phase plane: 1

合計チャンネル数:

```text
channels = 2 + 2H + 1 + 1
```

`phase plane` は以下のいずれかで実装可能とする。

- empty_count / 64
- ply / 60

標準は `empty_count / 64` とする。

### 4.2 教師モデル出力

教師モデルは 3 ヘッド構成とする。

- policy head: 64 logits
- margin head: 1 scalar
- WDL head: 3 logits

### 4.3 policy target 仕様

- パスは行動空間に含めない
- 合法手が存在しない局面では `policy_target = None` を許可する
- 学習時はそのサンプルの policy loss を 0 とする

### 4.4 margin target 仕様

```python
margin_target = final_margin_from_side_to_move / 64.0
```

範囲:
- `[-1.0, 1.0]`

### 4.5 WDL target 仕様

教師ラベルは原則 one-hot とする。

```python
if final_margin > 0:
    wdl_target = [1.0, 0.0, 0.0]
elif final_margin == 0:
    wdl_target = [0.0, 1.0, 0.0]
else:
    wdl_target = [0.0, 0.0, 1.0]
```

将来、探索分布やモンテカルロ集約値から soft target を与える拡張を許容する。

## 5. 教師モデル仕様

### 5.1 モデル方針

- 2D 盤面 CNN ベース
- policy / margin / WDL のマルチヘッド
- バックボーンは残差 CNN を標準とする
- attention 混在型への拡張余地を残す

### 5.2 PyTorch 標準モデル仕様例

```python
import torch
import torch.nn as nn
import torch.nn.functional as F


class ResidualBlock(nn.Module):
    def __init__(self, channels: int):
        super().__init__()
        self.conv1 = nn.Conv2d(channels, channels, kernel_size=3, padding=1, bias=False)
        self.bn1 = nn.BatchNorm2d(channels)
        self.conv2 = nn.Conv2d(channels, channels, kernel_size=3, padding=1, bias=False)
        self.bn2 = nn.BatchNorm2d(channels)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        identity = x
        x = self.conv1(x)
        x = self.bn1(x)
        x = F.relu(x, inplace=True)
        x = self.conv2(x)
        x = self.bn2(x)
        x = x + identity
        x = F.relu(x, inplace=True)
        return x


class TeacherModel(nn.Module):
    def __init__(self, in_channels: int, trunk_channels: int = 128, num_blocks: int = 24):
        super().__init__()
        self.stem = nn.Sequential(
            nn.Conv2d(in_channels, trunk_channels, kernel_size=3, padding=1, bias=False),
            nn.BatchNorm2d(trunk_channels),
            nn.ReLU(inplace=True),
        )
        self.blocks = nn.Sequential(*[ResidualBlock(trunk_channels) for _ in range(num_blocks)])

        self.policy_head = nn.Sequential(
            nn.Conv2d(trunk_channels, 32, kernel_size=1, bias=False),
            nn.BatchNorm2d(32),
            nn.ReLU(inplace=True),
            nn.Flatten(),
            nn.Linear(32 * 8 * 8, 64),
        )

        self.margin_head = nn.Sequential(
            nn.Conv2d(trunk_channels, 32, kernel_size=1, bias=False),
            nn.BatchNorm2d(32),
            nn.ReLU(inplace=True),
            nn.Flatten(),
            nn.Linear(32 * 8 * 8, 128),
            nn.ReLU(inplace=True),
            nn.Linear(128, 1),
            nn.Tanh(),
        )

        self.wdl_head = nn.Sequential(
            nn.Conv2d(trunk_channels, 32, kernel_size=1, bias=False),
            nn.BatchNorm2d(32),
            nn.ReLU(inplace=True),
            nn.Flatten(),
            nn.Linear(32 * 8 * 8, 128),
            nn.ReLU(inplace=True),
            nn.Linear(128, 3),
        )

    def forward(self, x: torch.Tensor) -> dict[str, torch.Tensor]:
        x = self.stem(x)
        x = self.blocks(x)
        return {
            "policy_logits": self.policy_head(x),
            "margin": self.margin_head(x),
            "wdl_logits": self.wdl_head(x),
        }
```

### 5.3 forward 契約

入力:
- `x: torch.Tensor[batch, channels, 8, 8]`

出力:
- `policy_logits: torch.Tensor[batch, 64]`
- `margin: torch.Tensor[batch, 1]`
- `wdl_logits: torch.Tensor[batch, 3]`

## 6. 損失関数仕様

### 6.1 総損失

```python
loss = policy_weight * policy_loss + margin_weight * margin_loss + wdl_weight * wdl_loss
```

### 6.2 policy loss

- masked cross entropy を用いる
- 非合法手は損失計算から除外する
- `policy_target is None` のサンプルは loss weight 0

### 6.3 margin loss

- Huber loss を標準とする
- exact ラベルと非 exact ラベルで重みを変えてよい

### 6.4 wdl loss

- cross entropy または KL divergence

### 6.5 サンプル重み

次の重み付け拡張を許容する。

- exact label サンプルを重くする
- 終盤サンプルを重くする
- 拮抗局面を重くする

## 7. 学習フロー仕様

### 7.1 Phase 0: エンジン検証

- Rust エンジンの合法手生成、着手、終局判定、完全読み API を検証する
- ここではモデル学習を行わない

### 7.2 Phase 1: 初期データ生成

目的:
- ランダム合法対局により到達可能局面を集める

フロー:
1. 初期局面から合法手ランダム対局を生成する
2. 各対局から局面を抽出する
3. 空きマス閾値以下の局面には exact solver を適用する
4. それ以外は終局結果から margin/WDL を付与する
5. policy target は空でもよい

出力:
- 初期教師学習データセット

### 7.3 Phase 2: 初期教師学習

目的:
- margin/WDL を安定して予測できる初期教師を得る

フロー:
1. Phase 1 データで教師モデルを学習
2. validation で margin 誤差、WDL 精度を監視
3. 探索組み込み前の初期教師を確定

### 7.4 Phase 3: 自己対局による反復強化

目的:
- 教師モデルの局面分布を実戦寄りに更新する

フロー:
1. 現行最良教師を固定
2. 教師 + 探索で自己対局を生成
3. 一部にランダム性やノイズを残す
4. 終盤 exact solver により高精度ラベルを追加
5. replay buffer に混合格納
6. 前世代重みから継続学習
7. 候補モデルを自己対戦で評価
8. 勝ち越した候補を新 best とする

### 7.5 継続学習方針

標準方針:
- モデルは毎世代リセットしない
- 前世代最良チェックポイントから warm start する

補助検証:
- 一定周期で from-scratch 再学習を実施し、継続学習依存が強すぎないか確認する

## 8. 自己対局仕様

### 8.1 SelfPlayConfig

```python
from dataclasses import dataclass

@dataclass
class SelfPlayConfig:
    games_per_iteration: int
    search_depth: int | None
    search_nodes: int | None
    exact_solver_empty_threshold: int | None
    temperature_moves: int
    temperature: float
    random_opening_moves: int
    inject_random_ratio: float
```

### 8.2 自己対局出力

- 対局履歴
- 各局面特徴
- 教師評価
- 最善手または探索分布
- 終局石差
- exact solver 使用有無

### 8.3 policy target 生成方針

以下のいずれかを許容する。

- 探索最善手 one-hot
- 探索訪問回数分布
- 探索スコアを温度付き softmax 化した分布

標準は「探索最善手 one-hot」から開始し、後に探索分布へ拡張可能とする。

## 9. 評価仕様

### 9.1 教師モデル評価指標

- `margin_mae`
- `margin_rmse`
- `wdl_accuracy`
- `exact_subset_margin_mae`
- `policy_top1_accuracy`
- `policy_legal_mass`

### 9.2 実戦評価指標

- 同一探索条件での自己対戦勝率
- 石差平均
- 終盤 exact ベンチマーク一致率

### 9.3 モデル採用条件

新候補モデルは以下を満たすこと。

- 前世代 best に対し自己対戦で所定勝率以上
- 重大な過学習や exact subset 劣化がないこと

## 10. NNUE 蒸留仕様

### 10.1 蒸留目的

- 教師の margin 予測を高速 evaluator へ圧縮する
- 必要に応じて WDL を補助的に再現する
- 最終的に SIMD と量子化に適した重みへ変換する

### 10.2 NNUE 学習データ

NNUE 蒸留用データは教師学習用と分離管理する。

必要条件:
- 序盤・中盤・終盤のバランス
- 拮抗局面と大差局面の混在
- exact ラベル多め
- 自己対局由来局面中心

### 10.3 NNUE 入力仕様

NNUE 入力は疎特徴とする。
具体的な特徴設計は別文書とし、本仕様書では抽象契約のみ定義する。

```python
@dataclass
class NnueSample:
    active_indices: list[int]
    active_values: list[float]
    margin_target: float
    wdl_target: list[float]
    exact_label: bool
```

### 10.4 NNUE モデル仕様例

```python
import torch
import torch.nn as nn
import torch.nn.functional as F


class SparseLinear(nn.Module):
    def __init__(self, num_features: int, out_features: int):
        super().__init__()
        self.weight = nn.Parameter(torch.randn(num_features, out_features) * 0.01)
        self.bias = nn.Parameter(torch.zeros(out_features))

    def forward(self, indices: torch.Tensor, values: torch.Tensor) -> torch.Tensor:
        # 実運用では専用 gather 実装に置き換える
        gathered = self.weight[indices] * values.unsqueeze(-1)
        return gathered.sum(dim=1) + self.bias


class NnueModel(nn.Module):
    def __init__(self, num_features: int, hidden1: int = 256, hidden2: int = 32):
        super().__init__()
        self.ft = SparseLinear(num_features, hidden1)
        self.fc1 = nn.Linear(hidden1, hidden2)
        self.margin_head = nn.Linear(hidden2, 1)
        self.wdl_head = nn.Linear(hidden2, 3)

    def forward(self, indices: torch.Tensor, values: torch.Tensor) -> dict[str, torch.Tensor]:
        x = self.ft(indices, values)
        x = F.relu(x, inplace=True)
        x = F.relu(self.fc1(x), inplace=True)
        return {
            "margin": torch.tanh(self.margin_head(x)),
            "wdl_logits": self.wdl_head(x),
        }
```

### 10.5 NNUE 蒸留損失

```python
loss = margin_weight * margin_loss + wdl_weight * wdl_loss
```

policy は標準では蒸留対象に含めない。

### 10.6 NNUE 出力仕様

- `margin: [batch, 1]`
- `wdl_logits: [batch, 3]`

最終実行系で主に使う評価は `margin` とする。

## 11. エクスポート仕様

### 11.1 教師モデル

- PyTorch checkpoint: `.pt`
- 学習再開用 optimizer state を含める

### 11.2 NNUE モデル

必要出力:
- 生重み
- 量子化重み
- Rust 組み込み用バイナリ
- Rust 定数配列ソース

### 11.3 exporter 契約

```python
def export_nnue_weights(model: NnueModel, path: str, format: str) -> None:
    ...
```

`format` 候補:
- `npz`
- `bin`
- `rust_static`
- `wasm_bin`

## 12. 実行スクリプト仕様

### 12.1 データ生成

```bash
python -m data_pipeline.generator --config configs/generate_random.yaml
python -m data_pipeline.exact_labeler --config configs/exact_label.yaml
```

### 12.2 教師学習

```bash
python -m train_teacher.trainer --config configs/train_teacher.yaml
```

### 12.3 自己対局

```bash
python -m train_teacher.selfplay --config configs/selfplay.yaml
```

### 12.4 NNUE 学習

```bash
python -m train_nnue.trainer --config configs/train_nnue.yaml
```

### 12.5 エクスポート

```bash
python -m train_nnue.exporter --config configs/export_nnue.yaml
```

## 13. 設定ファイル仕様

### 13.1 教師学習設定例

```yaml
seed: 42
history_len: 4
train:
  batch_size: 512
  epochs: 10
  lr: 0.001
  policy_weight: 1.0
  margin_weight: 2.0
  wdl_weight: 0.5
model:
  trunk_channels: 128
  num_blocks: 24
data:
  train_path: data/train.parquet
  valid_path: data/valid.parquet
```

### 13.2 NNUE 学習設定例

```yaml
seed: 42
train:
  batch_size: 2048
  epochs: 20
  lr: 0.001
  margin_weight: 1.0
  wdl_weight: 0.25
model:
  num_features: 65536
  hidden1: 256
  hidden2: 32
data:
  train_path: data/nnue_train.parquet
  valid_path: data/nnue_valid.parquet
export:
  quantize: true
  target: rust_static
```

## 14. 品質保証仕様

### 14.1 必須検証

- 入力特徴 shape が常に安定していること
- `policy_target is None` サンプルで例外が出ないこと
- margin target 範囲が `[-1, 1]` に収まること
- WDL one-hot が常に和 1 であること

### 14.2 学習後検証

- exact subset で教師が一定精度以上
- NNUE が教師 margin を所定誤差以内で近似
- エクスポート前後で推論結果が一致

## 15. 将来拡張予約

- policy 蒸留付き NNUE
- attention 混在教師
- mixed precision 学習
- 分散自己対局
- ONNX エクスポート
