from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import numpy as np
import veloversi as vv

try:
    import torch
    from torch.utils.data import DataLoader, Dataset
except ModuleNotFoundError as exc:  # pragma: no cover - example guard
    raise SystemExit("PyTorch is not installed. Install it before running this example.") from exc


def _load_jsonl_records(data_dir: str | Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for path in sorted(Path(data_dir).glob("*.jsonl")):
        with path.open("r", encoding="utf-8") as f:
            for line in f:
                records.append(json.loads(line))
    if not records:
        raise ValueError("no .jsonl files found")
    return records


def _board_from_record(record: dict[str, Any]) -> vv.Board:
    board_dict = record["board"]
    return vv.board_from_bits(
        board_dict["black_bits"],
        board_dict["white_bits"],
        board_dict["side_to_move"],
    )


def _normalized_value_target(record: dict[str, Any], board: vv.Board) -> np.float32:
    margin = float(record["final_margin_from_black"])
    side_to_move_margin = margin if board.side_to_move == "black" else -margin
    return np.float32(side_to_move_margin / 64.0)


class ValueOnlyDataset(Dataset[dict[str, Any]]):
    def __init__(self, data_dir: str | Path) -> None:
        self.records = _load_jsonl_records(data_dir)

    def __len__(self) -> int:
        return len(self.records)

    def __getitem__(self, index: int) -> dict[str, Any]:
        return self.records[index]


class PolicyValueDataset(Dataset[dict[str, Any]]):
    def __init__(self, data_dir: str | Path) -> None:
        self.records = [
            record
            for record in _load_jsonl_records(data_dir)
            if isinstance(record.get("policy_target_index"), int)
            and 0 <= record["policy_target_index"] <= 63
        ]
        if not self.records:
            raise ValueError("no policy-enabled samples found")

    def __len__(self) -> int:
        return len(self.records)

    def __getitem__(self, index: int) -> dict[str, Any]:
        return self.records[index]


def collate_value_only_cnn(records: list[dict[str, Any]]) -> dict[str, torch.Tensor]:
    boards = [_board_from_record(record) for record in records]
    features = vv.prepare_cnn_model_input_batch(boards)
    value_targets = np.asarray(
        [_normalized_value_target(record, board) for record, board in zip(records, boards)],
        dtype=np.float32,
    )
    return {
        "features": torch.from_numpy(features),
        "value_targets": torch.from_numpy(value_targets),
    }


def collate_policy_value_cnn(records: list[dict[str, Any]]) -> dict[str, torch.Tensor]:
    boards = [_board_from_record(record) for record in records]
    features = vv.prepare_cnn_model_input_batch(boards)
    value_targets = np.asarray(
        [_normalized_value_target(record, board) for record, board in zip(records, boards)],
        dtype=np.float32,
    )
    policy_targets = np.asarray(
        [int(record["policy_target_index"]) for record in records],
        dtype=np.int64,
    )
    legal_move_masks = np.stack(
        [vv.prepare_cnn_model_input(board)[0, 2].reshape(64) for board in boards],
        axis=0,
    ).astype(np.float32, copy=False)
    return {
        "features": torch.from_numpy(features),
        "value_targets": torch.from_numpy(value_targets),
        "policy_targets": torch.from_numpy(policy_targets),
        "legal_move_masks": torch.from_numpy(legal_move_masks),
    }


def collate_value_only_flat(records: list[dict[str, Any]]) -> dict[str, torch.Tensor]:
    boards = [_board_from_record(record) for record in records]
    features = vv.prepare_flat_model_input_batch(boards)
    value_targets = np.asarray(
        [_normalized_value_target(record, board) for record, board in zip(records, boards)],
        dtype=np.float32,
    )
    return {
        "features": torch.from_numpy(features),
        "value_targets": torch.from_numpy(value_targets),
    }


def collate_policy_value_flat(records: list[dict[str, Any]]) -> dict[str, torch.Tensor]:
    boards = [_board_from_record(record) for record in records]
    features = vv.prepare_flat_model_input_batch(boards)
    value_targets = np.asarray(
        [_normalized_value_target(record, board) for record, board in zip(records, boards)],
        dtype=np.float32,
    )
    policy_targets = np.asarray(
        [int(record["policy_target_index"]) for record in records],
        dtype=np.int64,
    )
    legal_move_masks = np.stack(
        [vv.prepare_cnn_model_input(board)[0, 2].reshape(64) for board in boards],
        axis=0,
    ).astype(np.float32, copy=False)
    return {
        "features": torch.from_numpy(features),
        "value_targets": torch.from_numpy(value_targets),
        "policy_targets": torch.from_numpy(policy_targets),
        "legal_move_masks": torch.from_numpy(legal_move_masks),
    }


def main() -> None:
    value_loader = DataLoader(
        ValueOnlyDataset("examples/generated_data"),
        batch_size=4,
        shuffle=True,
        collate_fn=collate_value_only_cnn,
    )
    policy_loader = DataLoader(
        PolicyValueDataset("examples/generated_data"),
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
