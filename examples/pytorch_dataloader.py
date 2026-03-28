from __future__ import annotations

import json
from pathlib import Path
from typing import Any, cast

import numpy as np
import veloversi as vv

try:
    import torch
    from torch.utils.data import DataLoader, Dataset
except ModuleNotFoundError as exc:  # pragma: no cover - example guard
    raise SystemExit(
        "PyTorch is not installed. Install it before running this example."
    ) from exc


FEATURE_CONFIG = {
    "history_len": 0,
    "include_legal_mask": False,
    "include_phase_plane": True,
    "include_turn_plane": True,
    "perspective": "side_to_move",
}

class ReversiTrainingDataset(Dataset[dict[str, torch.Tensor]]):
    def __init__(self, data_dir: str | Path) -> None:
        self.records: list[dict[str, Any]] = []
        for path in sorted(Path(data_dir).glob("*.jsonl")):
            with path.open("r", encoding="utf-8") as f:
                for line in f:
                    self.records.append(json.loads(line))
        if not self.records:
            raise ValueError("no .jsonl files found")

    def __len__(self) -> int:
        return len(self.records)

    def __getitem__(self, index: int) -> dict[str, torch.Tensor]:
        record = self.records[index]
        batch = vv.prepare_planes_learning_batch([record], FEATURE_CONFIG)
        planes = cast(np.ndarray, batch["features"])[0]
        value_target = np.float32(cast(np.ndarray, batch["value_targets"])[0])
        policy_target = np.int64(cast(np.ndarray, batch["policy_targets"])[0])
        has_policy_target = np.float32(
            1.0 if record["has_policy_target"] else 0.0
        )

        return {
            "planes": torch.from_numpy(planes),
            "value_target": torch.tensor(value_target, dtype=torch.float32),
            "policy_target": torch.tensor(policy_target, dtype=torch.int64),
            "has_policy_target": torch.tensor(has_policy_target, dtype=torch.float32),
        }


def main() -> None:
    dataset = ReversiTrainingDataset("examples/generated_data")
    loader = DataLoader(dataset, batch_size=4, shuffle=True)

    batch = next(iter(loader))
    print("planes:", tuple(batch["planes"].shape))
    print("value_target:", tuple(batch["value_target"].shape))
    print("policy_target:", tuple(batch["policy_target"].shape))
    print("has_policy_target:", tuple(batch["has_policy_target"].shape))


if __name__ == "__main__":
    main()
