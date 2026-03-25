from typing import cast

import numpy as np

from ._core import (
    _encode_flat_features_batch_parts,
    _encode_flat_features_parts,
    _encode_planes_batch_parts,
    _encode_planes_parts,
    _play_random_game_parts,
    _sample_reachable_positions_parts,
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
    "encode_planes",
    "encode_planes_batch",
    "encode_flat_features",
    "encode_flat_features_batch",
    "pack_board",
    "play_random_game",
    "sample_reachable_positions",
    "unpack_board",
    "transform_board",
    "transform_square",
]


def _validate_optional_u16(value: object, name: str) -> int | None:
    if value is None:
        return None
    if type(value) is not int or not (0 <= value <= 0xFFFF):
        raise ValueError(f"{name} must be an int in 0..65535 or None")
    return value


def _validate_u16(value: object, name: str) -> int:
    if type(value) is not int or not (0 <= value <= 0xFFFF):
        raise ValueError(f"{name} must be an int in 0..65535")
    return value


def _validate_u32(value: object, name: str) -> int:
    if type(value) is not int or not (0 <= value <= 0xFFFF_FFFF):
        raise ValueError(f"{name} must be an int in 0..4294967295")
    return value


def _validate_bool(value: object, name: str) -> bool:
    if type(value) is not bool:
        raise ValueError(f"{name} must be a bool")
    return value


def _validate_feature_perspective(value: object) -> str:
    if type(value) is not str or value not in {"absolute_color", "side_to_move"}:
        raise ValueError("perspective must be 'absolute_color' or 'side_to_move'")
    return value


def _validate_feature_config(config: object) -> tuple[int, bool, bool, bool, str]:
    if type(config) is not dict:
        raise ValueError("config must be a dict")

    typed_config = cast(dict[object, object], config)
    history_len = _validate_u32(typed_config.get("history_len", 0), "history_len")
    include_legal_mask = _validate_bool(
        typed_config.get("include_legal_mask", False), "include_legal_mask"
    )
    include_phase_plane = _validate_bool(
        typed_config.get("include_phase_plane", False), "include_phase_plane"
    )
    include_turn_plane = _validate_bool(
        typed_config.get("include_turn_plane", False), "include_turn_plane"
    )
    perspective = _validate_feature_perspective(typed_config.get("perspective", "absolute_color"))
    return (
        history_len,
        include_legal_mask,
        include_phase_plane,
        include_turn_plane,
        perspective,
    )


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


def play_random_game(seed: int, config: dict) -> dict:
    if type(config) is not dict:
        raise ValueError("config must be a dict")

    max_plies = _validate_optional_u16(config.get("max_plies"), "max_plies")
    boards_bits, moves, final_result, final_margin_from_black, plies_played, reached_terminal = (
        _play_random_game_parts(seed, max_plies)
    )

    return {
        "boards": [Board(*bits) for bits in boards_bits],
        "moves": moves,
        "final_result": final_result,
        "final_margin_from_black": final_margin_from_black,
        "plies_played": plies_played,
        "reached_terminal": reached_terminal,
    }


def sample_reachable_positions(seed: int, config: dict) -> list[Board]:
    if type(config) is not dict:
        raise ValueError("config must be a dict")

    num_positions = _validate_u32(config.get("num_positions"), "num_positions")
    min_plies = _validate_u16(config.get("min_plies"), "min_plies")
    max_plies = _validate_u16(config.get("max_plies"), "max_plies")

    if min_plies > max_plies:
        raise ValueError("min_plies must be less than or equal to max_plies")

    return [
        Board(*bits)
        for bits in _sample_reachable_positions_parts(seed, num_positions, min_plies, max_plies)
    ]


def encode_planes(board: Board, history: list[Board], config: dict) -> np.ndarray:
    if type(history) is not list:
        raise ValueError("history must be a list[Board]")

    return _encode_planes_parts(board, history, *_validate_feature_config(config))


def encode_planes_batch(
    boards: list[Board], histories: list[list[Board]], config: dict
) -> np.ndarray:
    if type(boards) is not list:
        raise ValueError("boards must be a list[Board]")
    if type(histories) is not list:
        raise ValueError("histories must be a list[list[Board]]")
    if len(boards) != len(histories):
        raise ValueError("boards and histories must have the same length")

    return _encode_planes_batch_parts(boards, histories, *_validate_feature_config(config))


def encode_flat_features(board: Board, history: list[Board], config: dict) -> np.ndarray:
    if type(history) is not list:
        raise ValueError("history must be a list[Board]")

    return _encode_flat_features_parts(board, history, *_validate_feature_config(config))


def encode_flat_features_batch(
    boards: list[Board], histories: list[list[Board]], config: dict
) -> np.ndarray:
    if type(boards) is not list:
        raise ValueError("boards must be a list[Board]")
    if type(histories) is not list:
        raise ValueError("histories must be a list[list[Board]]")
    if len(boards) != len(histories):
        raise ValueError("boards and histories must have the same length")

    return _encode_flat_features_batch_parts(boards, histories, *_validate_feature_config(config))


def main() -> None:
    print("hello veloversi")
