from ._core import (
    Board,
    apply_move,
    apply_move_bits,
    apply_move_unchecked,
    apply_move_unchecked_bits,
    board_from_bits,
    generate_legal_moves,
    generate_legal_moves_bits,
    initial_board,
)

__all__ = [
    "Board",
    "initial_board",
    "board_from_bits",
    "generate_legal_moves",
    "generate_legal_moves_bits",
    "apply_move_unchecked",
    "apply_move_unchecked_bits",
    "apply_move",
    "apply_move_bits",
]


def main() -> None:
    print("hello veloversi")
