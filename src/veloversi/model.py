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

    class _NNUE(nn.Module):
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

        def forward(self, x: object) -> object:
            if not hasattr(x, "dim") or not hasattr(x, "shape"):
                raise TypeError("NNUE expects a torch.Tensor input")
            tensor = cast(Any, x)
            if tensor.dim() != 2 or tuple(tensor.shape) != (tensor.shape[0], NNUE_INPUT_LEN):
                raise ValueError(f"NNUE input shape must be (B, {NNUE_INPUT_LEN})")
            indices = tensor.long()
            acc = self.accumulator_bias.unsqueeze(0).expand(indices.shape[0], -1).clone()
            pattern_indices = indices[:, :NNUE_PATTERN_SLOTS]
            scalar_indices = indices[:, NNUE_PATTERN_SLOTS:]
            for slot, family in enumerate(NNUE_SLOT_FAMILIES):
                acc = acc + self.pattern_tables[family](pattern_indices[:, slot])
            for scalar_slot in range(NNUE_SCALAR_SLOTS):
                acc = acc + self.scalar_tables[scalar_slot](scalar_indices[:, scalar_slot])
            acc = cast(Any, torch).clamp(acc, 0.0, 127.0)
            hidden = cast(Any, torch).clamp(self.fc1(acc), 0.0, 127.0)
            return self.fc2(hidden)

    _NNUE.__name__ = "NNUE"
    _NNUE.__qualname__ = "NNUE"
    return _NNUE


def NNUE() -> object:
    """PyTorch 学習用の NNUE 風 value model を生成します。"""
    return _nnue_class()()
