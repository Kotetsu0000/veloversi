# pyright: reportMissingImports=false

import importlib
from pathlib import Path
from typing import Any, cast

import numpy as np

__all__ = ["get_dataloader"]


def _import_torch_for_dataloader() -> object:
    try:
        import torch
    except ModuleNotFoundError as exc:
        raise RuntimeError(
            "veloversi.get_dataloader を使うには PyTorch (`torch`) の導入が必要です"
        ) from exc
    return torch


def _import_veloversi_package() -> Any:
    return importlib.import_module("veloversi")


def get_dataloader(
    paths: str | Path | list[str | Path],
    batch_size: int,
    *,
    mode: str = "value_only",
    shuffle: bool = True,
    num_workers: int = 0,
    drop_last: bool = False,
    pin_memory: bool = False,
) -> object:
    """学習用 DataLoader を返します。

    Args:
        paths:
            JSONL の単一パス、または複数パス。
        batch_size:
            DataLoader の batch size。
        mode:
            `"value_only"` / `"policy_only"` / `"policy_value"`。

    Returns:
        `torch.utils.data.DataLoader`。

        `value_only` の batch は少なくとも次を持ちます。
        - `board_cnn`: `(B, 3, 8, 8)` `torch.float32`
        - `board_flat`: `(B, 192)` `torch.float32`
        - `board_nnue`: `(B, 67)` `torch.int32`
        - `value`: `(B, 1)` `torch.float32`

        `policy_only` の batch は少なくとも次を持ちます。
        - `board_cnn`: `(B, 3, 8, 8)` `torch.float32`
        - `board_flat`: `(B, 192)` `torch.float32`
        - `board_nnue`: `(B, 67)` `torch.int32`
        - `policy`: `(B, 64)` `torch.float32`
        - `policy_index`: `(B,)` `torch.int64`
        - `legal_mask`: `(B, 64)` `torch.float32`

        `policy_value` の batch は上記に加えて次を持ちます。
        - `policy`: `(B, 64)` `torch.float32`
        - `policy_index`: `(B,)` `torch.int64`
        - `legal_mask`: `(B, 64)` `torch.float32`
    """
    if type(batch_size) is not int or batch_size <= 0:
        raise ValueError("batch_size must be a positive int")
    if type(mode) is not str or mode not in {"value_only", "policy_only", "policy_value"}:
        raise ValueError("mode must be 'value_only', 'policy_only', or 'policy_value'")

    torch = _import_torch_for_dataloader()
    vv = _import_veloversi_package()
    data = cast(Any, torch).utils.data
    Dataset = data.Dataset
    DataLoader = data.DataLoader

    record_dataset = vv.open_game_record_dataset(paths)

    class _BaseTrainingDataset(Dataset):
        def __len__(self) -> int:
            return len(record_dataset)

        @staticmethod
        def _base_item(index: int) -> dict[str, np.ndarray]:
            return {
                "board_cnn": record_dataset.get_cnn_input(index).astype(np.float32, copy=False),
                "board_flat": record_dataset.get_flat_input(index).astype(np.float32, copy=False),
                "board_nnue": record_dataset.get_nnue_input(index).astype(np.int32, copy=False),
            }

        @staticmethod
        def _policy_item(index: int) -> dict[str, np.ndarray]:
            targets = record_dataset.get_targets(index)
            policy = cast(np.ndarray, targets["policy_target"]).astype(np.float32, copy=False)
            return {
                "policy": policy,
                "policy_index": np.asarray(int(np.argmax(policy)), dtype=np.int64),
                "legal_mask": record_dataset.get(index)["board"]
                .prepare_cnn_model_input()[0, 2]
                .reshape(64)
                .astype(np.float32, copy=False),
            }

    class _ValueOnlyDataset(_BaseTrainingDataset):
        def __getitem__(self, index: int) -> dict[str, np.ndarray]:
            targets = record_dataset.get_targets(index)
            item = self._base_item(index)
            item["value"] = np.asarray([targets["value_target"]], dtype=np.float32)
            return item

    class _PolicyOnlyDataset(_BaseTrainingDataset):
        def __getitem__(self, index: int) -> dict[str, np.ndarray]:
            item = self._base_item(index)
            item.update(self._policy_item(index))
            return item

    class _PolicyValueDataset(_BaseTrainingDataset):
        def __getitem__(self, index: int) -> dict[str, np.ndarray]:
            targets = record_dataset.get_targets(index)
            item = self._base_item(index)
            item["value"] = np.asarray([targets["value_target"]], dtype=np.float32)
            item.update(self._policy_item(index))
            return item

    dataset: object
    if mode == "value_only":
        dataset = _ValueOnlyDataset()
    elif mode == "policy_only":
        dataset = _PolicyOnlyDataset()
    else:
        dataset = _PolicyValueDataset()

    return DataLoader(
        dataset,
        batch_size=batch_size,
        shuffle=shuffle,
        num_workers=num_workers,
        drop_last=drop_last,
        pin_memory=pin_memory,
    )
