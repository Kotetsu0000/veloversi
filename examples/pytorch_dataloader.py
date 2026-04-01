from __future__ import annotations

from pathlib import Path
from typing import Any

import numpy as np
import veloversi as vv

try:
    import torch
    from torch.utils.data import DataLoader, Dataset
except ModuleNotFoundError as exc:  # pragma: no cover - example guard
    raise SystemExit("PyTorch is not installed. Install it before running this example.") from exc


class ValueOnlyDataset(Dataset[dict[str, Any]]):
    def __init__(self, paths: str | Path | list[str | Path]) -> None:
        self.dataset = vv.open_game_record_dataset(paths)

    def __len__(self) -> int:
        return len(self.dataset)

    def __getitem__(self, index: int) -> dict[str, Any]:
        item = self.dataset.get(index)
        targets = self.dataset.get_targets(index)
        return {
            "board": item["board"],
            "value_target": targets["value_target"],
        }


class PolicyValueDataset(Dataset[dict[str, Any]]):
    def __init__(self, paths: str | Path | list[str | Path]) -> None:
        self.dataset = vv.open_game_record_dataset(paths)

    def __len__(self) -> int:
        return len(self.dataset)

    def __getitem__(self, index: int) -> dict[str, Any]:
        item = self.dataset.get(index)
        targets = self.dataset.get_targets(index)
        return {
            "board": item["board"],
            "value_target": targets["value_target"],
            "policy_target": targets["policy_target"],
        }


def collate_value_only_cnn(records: list[dict[str, Any]]) -> dict[str, torch.Tensor]:
    boards = [record["board"] for record in records]
    features = vv.prepare_cnn_model_input_batch(boards)
    value_targets = np.asarray([record["value_target"] for record in records], dtype=np.float32)
    return {
        "features": torch.from_numpy(features),
        "value_targets": torch.from_numpy(value_targets),
    }


def collate_policy_value_cnn(records: list[dict[str, Any]]) -> dict[str, torch.Tensor]:
    boards = [record["board"] for record in records]
    features = vv.prepare_cnn_model_input_batch(boards)
    value_targets = np.asarray([record["value_target"] for record in records], dtype=np.float32)
    policy_targets = np.stack([record["policy_target"] for record in records], axis=0).astype(
        np.float32,
        copy=False,
    )
    legal_move_masks = np.stack(
        [board.prepare_cnn_model_input()[0, 2].reshape(64) for board in boards],
        axis=0,
    ).astype(np.float32, copy=False)
    return {
        "features": torch.from_numpy(features),
        "value_targets": torch.from_numpy(value_targets),
        "policy_targets": torch.from_numpy(policy_targets),
        "legal_move_masks": torch.from_numpy(legal_move_masks),
    }


def collate_value_only_flat(records: list[dict[str, Any]]) -> dict[str, torch.Tensor]:
    boards = [record["board"] for record in records]
    features = vv.prepare_flat_model_input_batch(boards)
    value_targets = np.asarray([record["value_target"] for record in records], dtype=np.float32)
    return {
        "features": torch.from_numpy(features),
        "value_targets": torch.from_numpy(value_targets),
    }


def collate_policy_value_flat(records: list[dict[str, Any]]) -> dict[str, torch.Tensor]:
    boards = [record["board"] for record in records]
    features = vv.prepare_flat_model_input_batch(boards)
    value_targets = np.asarray([record["value_target"] for record in records], dtype=np.float32)
    policy_targets = np.stack([record["policy_target"] for record in records], axis=0).astype(
        np.float32,
        copy=False,
    )
    legal_move_masks = np.stack(
        [board.prepare_cnn_model_input()[0, 2].reshape(64) for board in boards],
        axis=0,
    ).astype(np.float32, copy=False)
    return {
        "features": torch.from_numpy(features),
        "value_targets": torch.from_numpy(value_targets),
        "policy_targets": torch.from_numpy(policy_targets),
        "legal_move_masks": torch.from_numpy(legal_move_masks),
    }


def main() -> None:
    data_path = Path("examples/generated_records.jsonl")

    value_loader = DataLoader(
        ValueOnlyDataset(data_path),
        batch_size=4,
        shuffle=True,
        collate_fn=collate_value_only_cnn,
    )
    policy_loader = DataLoader(
        PolicyValueDataset(data_path),
        batch_size=4,
        shuffle=True,
        collate_fn=collate_policy_value_flat,
    )

    value_batch = next(iter(value_loader))
    policy_batch = next(iter(policy_loader))

    print("value-only cnn features:", tuple(value_batch["features"].shape))
    print("value-only targets:", tuple(value_batch["value_targets"].shape))
    print("policy+value flat features:", tuple(policy_batch["features"].shape))
    print("policy+value targets:", tuple(policy_batch["value_targets"].shape))
    print("policy targets:", tuple(policy_batch["policy_targets"].shape))
    print("legal move masks:", tuple(policy_batch["legal_move_masks"].shape))


if __name__ == "__main__":
    main()
