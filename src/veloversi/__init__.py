from ._core import (
    _unpack_board_parts,
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
    pack_board,
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
    "pack_board",
    "unpack_board",
    "transform_board",
    "transform_square",
]


def unpack_board(packed: tuple[int, int, str]) -> Board:
    if type(packed) is not tuple or len(packed) != 3:
        raise ValueError("packed must be a tuple[int, int, str]")

    black_bits, white_bits, side_to_move = packed
    if type(black_bits) is not int:
        raise ValueError("packed[0] must be int")
    if type(white_bits) is not int:
        raise ValueError("packed[1] must be int")
    if type(side_to_move) is not str:
        raise ValueError("packed[2] must be str")

    return _unpack_board_parts(black_bits, white_bits, side_to_move)


def main() -> None:
    print("hello veloversi")
