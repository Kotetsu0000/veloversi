from pathlib import Path
from typing import Any

def get_dataloader(
    paths: str | Path | list[str | Path],
    batch_size: int,
    *,
    mode: str = "value_only",
    shuffle: bool = True,
    num_workers: int = 0,
    drop_last: bool = False,
    pin_memory: bool = False,
) -> Any: ...
