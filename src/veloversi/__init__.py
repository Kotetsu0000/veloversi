from ._core import (
    Board,
    all_symmetries,
    apply_move,
    apply_forced_pass,
    board_from_bits,
    board_status,
    disc_count,
    final_margin_from_black,
    game_result,
    generate_legal_moves,
    initial_board,
    is_legal_move,
    legal_moves_list,
    transform_board,
    transform_square,
    validate_board,
)

__all__ = [
    "Board",
    "initial_board",
    "board_from_bits",
    "all_symmetries",
    "validate_board",
    "generate_legal_moves",
    "legal_moves_list",
    "is_legal_move",
    "apply_move",
    "apply_forced_pass",
    "board_status",
    "disc_count",
    "game_result",
    "final_margin_from_black",
    "transform_board",
    "transform_square",
]


def main() -> None:
    print("hello veloversi")
