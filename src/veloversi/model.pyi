from typing import Any

NNUE_FORMAT: str
NNUE_ARCHITECTURE: str
NNUE_VERSION: int
NNUE_PATTERN_FAMILY_SIZES: tuple[int, ...]
NNUE_PATTERN_FAMILIES: int
NNUE_PATTERN_SLOTS: int
NNUE_SCALAR_BUCKET_SIZES: tuple[int, ...]
NNUE_SCALAR_SLOTS: int
NNUE_INPUT_LEN: int
NNUE_ACCUMULATOR_DIM: int
NNUE_HIDDEN_DIM: int
NNUE_SLOT_FAMILIES: tuple[int, ...]

class NNUEModel:
    def reset_parameters(self) -> None: ...
    def forward(self, x: Any) -> Any: ...
    def forward_value_raw(self, x: Any) -> Any: ...
    def forward_value(self, x: Any) -> Any: ...
    def forward_policy_logits(self, x: Any) -> Any: ...
    def forward_policy(self, x: Any) -> Any: ...

def NNUE() -> NNUEModel: ...
