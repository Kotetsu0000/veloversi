# pyright: reportMissingImports=false

from functools import lru_cache
from typing import Any, cast

NNUE_FORMAT = "veloversi-vvm"
NNUE_ARCHITECTURE = "nnue-v1"
NNUE_VERSION = 1

NNUE_PATTERN_FAMILY_SIZES = (8, 9, 8, 9, 8, 9, 7, 10, 10, 10, 10, 10, 10, 10, 10, 10)
NNUE_PATTERN_FAMILIES = len(NNUE_PATTERN_FAMILY_SIZES)
NNUE_PATTERN_SLOTS = NNUE_PATTERN_FAMILIES * 4
NNUE_SCALAR_BUCKET_SIZES = (65, 65, 65)
NNUE_SCALAR_SLOTS = len(NNUE_SCALAR_BUCKET_SIZES)
NNUE_INPUT_LEN = NNUE_PATTERN_SLOTS + NNUE_SCALAR_SLOTS
NNUE_ACCUMULATOR_DIM = 32
NNUE_HIDDEN_DIM = 16
NNUE_SLOT_FAMILIES = tuple(slot // 4 for slot in range(NNUE_PATTERN_SLOTS))

__all__ = [
    "NNUE",
    "NNUE_FORMAT",
    "NNUE_ARCHITECTURE",
    "NNUE_VERSION",
    "NNUE_PATTERN_FAMILY_SIZES",
    "NNUE_PATTERN_FAMILIES",
    "NNUE_PATTERN_SLOTS",
    "NNUE_SCALAR_BUCKET_SIZES",
    "NNUE_SCALAR_SLOTS",
    "NNUE_INPUT_LEN",
    "NNUE_ACCUMULATOR_DIM",
    "NNUE_HIDDEN_DIM",
    "NNUE_SLOT_FAMILIES",
]


def _import_torch_for_model() -> object:
    try:
        import torch
    except ModuleNotFoundError as exc:
        raise RuntimeError("veloversi.model を使うには PyTorch (`torch`) の導入が必要です") from exc
    return torch


@lru_cache(maxsize=1)
def _nnue_class() -> type[object]:
    torch = _import_torch_for_model()
    nn = cast(Any, torch).nn
    init = cast(Any, nn).init

    class _NNUE(nn.Module):
        """PyTorch 学習用の NNUE 風 value model です。

        入力:
            x: `(B, 67)` の整数 tensor
                - 先頭 64 要素: pattern index
                - 後ろ 3 要素: scalar bucket index

        注意:
            現在の NNUE 入力は value 推論専用です。合法手マスクを含まないため、
            policy head は提供しません。

        出力:
            `forward(x)` / `forward_value_raw(x)`:
                `(B, 1)` の生値
            `forward_value(x)`:
                `tanh` を通した `(B, 1)` の値域 `[-1, 1]`
        """

        def __init__(self) -> None:
            super().__init__()
            self.pattern_tables = nn.ModuleList(
                [
                    nn.Embedding(3**pattern_size, NNUE_ACCUMULATOR_DIM)
                    for pattern_size in NNUE_PATTERN_FAMILY_SIZES
                ]
            )
            self.scalar_tables = nn.ModuleList(
                [
                    nn.Embedding(bucket_size, NNUE_ACCUMULATOR_DIM)
                    for bucket_size in NNUE_SCALAR_BUCKET_SIZES
                ]
            )
            self.accumulator_bias = nn.Parameter(
                cast(Any, torch).zeros(NNUE_ACCUMULATOR_DIM, dtype=cast(Any, torch).float32)
            )
            self.fc1 = nn.Linear(NNUE_ACCUMULATOR_DIM, NNUE_HIDDEN_DIM)
            self.fc2 = nn.Linear(NNUE_HIDDEN_DIM, 1)
            self.reset_parameters()

        @staticmethod
        def _validate_input(x: object) -> None:
            if not hasattr(x, "dim") or not hasattr(x, "shape"):
                raise TypeError("NNUE expects a torch.Tensor input")
            tensor = cast(Any, x)
            if tensor.dim() != 2 or tensor.shape[1] != NNUE_INPUT_LEN:
                raise ValueError(f"NNUE input shape must be (B, {NNUE_INPUT_LEN})")

        def reset_parameters(self) -> None:
            for table in self.pattern_tables:
                init.uniform_(table.weight, -0.05, 0.05)
            for table in self.scalar_tables:
                init.uniform_(table.weight, -0.05, 0.05)
            init.zeros_(self.accumulator_bias)
            init.kaiming_uniform_(self.fc1.weight, a=0.0, nonlinearity="relu")
            init.zeros_(self.fc1.bias)
            init.uniform_(self.fc2.weight, -0.01, 0.01)
            init.zeros_(self.fc2.bias)

        def _encode(self, x: object) -> object:
            self._validate_input(x)
            tensor = cast(Any, x)
            indices = tensor.long()
            acc = self.accumulator_bias.unsqueeze(0).expand(indices.shape[0], -1).clone()
            pattern_indices = indices[:, :NNUE_PATTERN_SLOTS]
            scalar_indices = indices[:, NNUE_PATTERN_SLOTS:]
            for slot, family in enumerate(NNUE_SLOT_FAMILIES):
                acc = acc + self.pattern_tables[family](pattern_indices[:, slot])
            for scalar_slot in range(NNUE_SCALAR_SLOTS):
                acc = acc + self.scalar_tables[scalar_slot](scalar_indices[:, scalar_slot])
            acc = cast(Any, torch).clamp(acc, 0.0, 127.0)
            return cast(Any, torch).clamp(self.fc1(acc), 0.0, 127.0)

        def forward_value_raw(self, x: object) -> object:
            """value head の生値 `(B, 1)` を返します。"""
            hidden = self._encode(x)
            return self.fc2(hidden)

        def forward_value(self, x: object) -> object:
            """`tanh` 済みの value `(B, 1)` を返します。"""
            return cast(Any, torch).tanh(self.forward_value_raw(x))

        def forward_policy_logits(self, x: object) -> object:
            raise NotImplementedError(
                "NNUE policy head is not supported: current NNUE input format does not include "
                "legal-move information"
            )

        def forward_policy(self, x: object) -> object:
            raise NotImplementedError(
                "NNUE policy head is not supported: current NNUE input format does not include "
                "legal-move information"
            )

        def forward(self, x: object) -> object:
            """Rust export と推論整合のため、生の value `(B, 1)` を返します。"""
            return self.forward_value_raw(x)

    _NNUE.__name__ = "NNUE"
    _NNUE.__qualname__ = "NNUE"
    return _NNUE


def NNUE() -> object:
    """PyTorch 学習用の NNUE 風 value model を生成します。"""
    return _nnue_class()()
