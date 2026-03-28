from typing import cast

import numpy as np

from ._core import (
    _encode_flat_features_batch_parts,
    _encode_flat_features_parts,
    _encode_planes_batch_parts,
    _encode_planes_parts,
    _append_game_record_parts,
    _finish_game_recording_parts,
    _load_game_records_parts,
    _packed_supervised_examples_from_trace_parts,
    _packed_supervised_examples_from_traces_parts,
    _prepare_flat_learning_batch_parts,
    _prepare_planes_learning_batch_parts,
    _play_random_game_parts,
    _random_start_board_parts,
    _record_move_parts,
    _record_pass_parts,
    _sample_reachable_positions_parts,
    _start_game_recording_parts,
    _supervised_examples_from_trace_parts,
    _supervised_examples_from_traces_parts,
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
    "random_start_board",
    "sample_reachable_positions",
    "start_game_recording",
    "record_move",
    "record_pass",
    "current_board",
    "finish_game_recording",
    "append_game_record",
    "load_game_records",
    "supervised_examples_from_trace",
    "supervised_examples_from_traces",
    "packed_supervised_examples_from_trace",
    "packed_supervised_examples_from_traces",
    "prepare_planes_learning_batch",
    "prepare_flat_learning_batch",
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


def random_start_board(plies: int, seed: int) -> Board:
    return Board(*_random_start_board_parts(seed, _validate_u16(plies, "plies")))


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


def start_game_recording(start_board: Board) -> dict[str, object]:
    return _recording_from_parts(_start_game_recording_parts(start_board))


def record_move(record: dict, square: int) -> dict[str, object]:
    return _recording_from_parts(
        _record_move_parts(*_recording_to_core_parts(record), _validate_u16(square, "square"))
    )


def record_pass(record: dict) -> dict[str, object]:
    return _recording_from_parts(_record_pass_parts(*_recording_to_core_parts(record)))


def current_board(record: dict) -> Board:
    return cast(Board, _validate_recording(record)["current_board"])


def finish_game_recording(record: dict) -> dict[str, object]:
    return _game_record_from_parts(_finish_game_recording_parts(*_recording_to_core_parts(record)))


def append_game_record(path: str, record: dict) -> None:
    if type(path) is not str:
        raise ValueError("path must be a str")
    _append_game_record_parts(path, *_game_record_to_core_parts(record))


def load_game_records(path: str) -> list[dict[str, object]]:
    if type(path) is not str:
        raise ValueError("path must be a str")
    return [_game_record_from_parts(parts) for parts in _load_game_records_parts(path)]


def _validate_random_game_trace(trace: object) -> dict[object, object]:
    if type(trace) is not dict:
        raise ValueError("trace must be a dict")
    return cast(dict[object, object], trace)


def _validate_recording(record: object) -> dict[object, object]:
    if type(record) is not dict:
        raise ValueError("record must be a dict")
    return cast(dict[object, object], record)


def _trace_to_core_parts(
    trace: object,
) -> tuple[list[tuple[int, int, str]], list[int | None], str, int, int, bool]:
    typed_trace = _validate_random_game_trace(trace)
    boards = typed_trace.get("boards")
    moves = typed_trace.get("moves")
    final_result = typed_trace.get("final_result")
    final_margin_from_black = typed_trace.get("final_margin_from_black")
    plies_played = typed_trace.get("plies_played")
    reached_terminal = typed_trace.get("reached_terminal")

    if type(boards) is not list or not all(isinstance(board, Board) for board in boards):
        raise ValueError("trace['boards'] must be a list[Board]")
    if type(moves) is not list:
        raise ValueError("trace['moves'] must be a list[int | None]")
    if type(final_result) is not str:
        raise ValueError("trace['final_result'] must be a str")
    if final_result not in {"black_win", "white_win", "draw"}:
        raise ValueError("trace['final_result'] must be 'black_win', 'white_win', or 'draw'")
    if type(final_margin_from_black) is not int:
        raise ValueError("trace['final_margin_from_black'] must be an int")
    if type(plies_played) is not int or not (0 <= plies_played <= 0xFFFF):
        raise ValueError("trace['plies_played'] must be an int in 0..65535")
    if type(reached_terminal) is not bool:
        raise ValueError("trace['reached_terminal'] must be a bool")

    validated_moves: list[int | None] = []
    for move in moves:
        if move is None:
            validated_moves.append(None)
        elif type(move) is int and 0 <= move <= 63:
            validated_moves.append(move)
        else:
            raise ValueError("trace['moves'] must contain int in 0..63 or None")

    if len(cast(list[Board], boards)) != len(validated_moves) + 1:
        raise ValueError("trace['boards'] must have len(trace['moves']) + 1")
    if plies_played != len(validated_moves):
        raise ValueError("trace['plies_played'] must equal len(trace['moves'])")

    return (
        [board.to_bits() for board in cast(list[Board], boards)],
        validated_moves,
        final_result,
        final_margin_from_black,
        plies_played,
        reached_terminal,
    )


def _recording_to_core_parts(
    record: object,
) -> tuple[tuple[int, int, str], tuple[int, int, str], list[int | None]]:
    typed_record = _validate_recording(record)
    start_board = typed_record.get("start_board")
    current = typed_record.get("current_board")
    moves = typed_record.get("moves")

    if not isinstance(start_board, Board):
        raise ValueError("record['start_board'] must be a Board")
    if not isinstance(current, Board):
        raise ValueError("record['current_board'] must be a Board")
    if type(moves) is not list:
        raise ValueError("record['moves'] must be a list[int | None]")

    validated_moves: list[int | None] = []
    for move in moves:
        if move is None:
            validated_moves.append(None)
        elif type(move) is int and 0 <= move <= 63:
            validated_moves.append(move)
        else:
            raise ValueError("record['moves'] must contain int in 0..63 or None")

    return start_board.to_bits(), current.to_bits(), validated_moves


def _recording_from_parts(
    parts: tuple[tuple[int, int, str], tuple[int, int, str], list[int | None]],
) -> dict[str, object]:
    start_board_bits, current_board_bits, moves = parts
    return {
        "start_board": Board(*start_board_bits),
        "current_board": Board(*current_board_bits),
        "moves": moves,
    }


def _game_record_from_parts(
    parts: tuple[tuple[int, int, str], list[int | None], str, int, int, int, int],
) -> dict[str, object]:
    (
        start_board,
        moves,
        final_result,
        final_black_discs,
        final_white_discs,
        final_empty_discs,
        final_margin_from_black,
    ) = parts
    return {
        "start_board": start_board,
        "moves": moves,
        "final_result": final_result,
        "final_black_discs": final_black_discs,
        "final_white_discs": final_white_discs,
        "final_empty_discs": final_empty_discs,
        "final_margin_from_black": final_margin_from_black,
    }


def _game_record_to_core_parts(
    record: object,
) -> tuple[tuple[int, int, str], list[int | None], str, int, int, int, int]:
    if type(record) is not dict:
        raise ValueError("record must be a dict")
    typed_record = cast(dict[object, object], record)
    start_board = typed_record.get("start_board")
    moves = typed_record.get("moves")
    final_result = typed_record.get("final_result")
    final_black_discs = typed_record.get("final_black_discs")
    final_white_discs = typed_record.get("final_white_discs")
    final_empty_discs = typed_record.get("final_empty_discs")
    final_margin_from_black = typed_record.get("final_margin_from_black")

    if type(start_board) is not tuple or len(start_board) != 3:
        raise ValueError("record['start_board'] must be tuple[int, int, str]")
    black_bits, white_bits, side_to_move = start_board
    if type(black_bits) is not int or type(white_bits) is not int or type(side_to_move) is not str:
        raise ValueError("record['start_board'] must be tuple[int, int, str]")
    if type(moves) is not list:
        raise ValueError("record['moves'] must be a list[int | None]")
    if type(final_result) is not str or final_result not in {"black", "white", "draw"}:
        raise ValueError("record['final_result'] must be 'black', 'white', or 'draw'")
    if type(final_black_discs) is not int or not (0 <= final_black_discs <= 64):
        raise ValueError("record['final_black_discs'] must be an int in 0..64")
    if type(final_white_discs) is not int or not (0 <= final_white_discs <= 64):
        raise ValueError("record['final_white_discs'] must be an int in 0..64")
    if type(final_empty_discs) is not int or not (0 <= final_empty_discs <= 64):
        raise ValueError("record['final_empty_discs'] must be an int in 0..64")
    if type(final_margin_from_black) is not int:
        raise ValueError("record['final_margin_from_black'] must be an int")

    validated_moves: list[int | None] = []
    for move in moves:
        if move is None:
            validated_moves.append(None)
        elif type(move) is int and 0 <= move <= 63:
            validated_moves.append(move)
        else:
            raise ValueError("record['moves'] must contain int in 0..63 or None")

    return (
        (black_bits, white_bits, side_to_move),
        validated_moves,
        final_result,
        final_black_discs,
        final_white_discs,
        final_empty_discs,
        final_margin_from_black,
    )


def _example_from_parts(
    parts: tuple[tuple[int, int, str], int, list[int | None], str, int],
) -> dict[str, object]:
    board_bits, ply, moves_until_here, final_result, final_margin_from_black = parts
    return {
        "board": Board(*board_bits),
        "ply": ply,
        "moves_until_here": moves_until_here,
        "final_result": final_result,
        "final_margin_from_black": final_margin_from_black,
    }


def _packed_example_from_parts(
    parts: tuple[tuple[int, int, str], int, list[int | None], str, int, int],
) -> dict[str, object]:
    (
        board_bits,
        ply,
        moves_until_here,
        final_result,
        final_margin_from_black,
        policy_target_index,
    ) = parts
    if policy_target_index == -1:
        policy_target_square: int | None = None
        policy_target_is_pass = False
        has_policy_target = False
    elif policy_target_index == 64:
        policy_target_square = None
        policy_target_is_pass = True
        has_policy_target = True
    else:
        policy_target_square = policy_target_index
        policy_target_is_pass = False
        has_policy_target = True
    return {
        "board": board_bits,
        "ply": ply,
        "moves_until_here": moves_until_here,
        "final_result": final_result,
        "final_margin_from_black": final_margin_from_black,
        "policy_target_index": policy_target_index,
        "policy_target_square": policy_target_square,
        "policy_target_is_pass": policy_target_is_pass,
        "has_policy_target": has_policy_target,
    }


def _validate_packed_supervised_example(example: object) -> dict[object, object]:
    if type(example) is not dict:
        raise ValueError("example must be a dict")
    return cast(dict[object, object], example)


def _packed_example_to_core_parts(
    example: object,
) -> tuple[tuple[int, int, str], int, list[int | None], str, int, int]:
    typed_example = _validate_packed_supervised_example(example)
    board = typed_example.get("board")
    ply = typed_example.get("ply")
    moves_until_here = typed_example.get("moves_until_here")
    final_result = typed_example.get("final_result")
    final_margin_from_black = typed_example.get("final_margin_from_black")
    policy_target_index = typed_example.get("policy_target_index")

    if type(board) is not tuple or len(board) != 3:
        raise ValueError("example['board'] must be tuple[int, int, str]")
    black_bits, white_bits, side_to_move = board
    if type(black_bits) is not int or type(white_bits) is not int or type(side_to_move) is not str:
        raise ValueError("example['board'] must be tuple[int, int, str]")
    if type(ply) is not int or not (0 <= ply <= 0xFFFF):
        raise ValueError("example['ply'] must be an int in 0..65535")
    if type(moves_until_here) is not list:
        raise ValueError("example['moves_until_here'] must be a list[int | None]")
    if type(final_result) is not str or final_result not in {"black_win", "white_win", "draw"}:
        raise ValueError("example['final_result'] must be 'black_win', 'white_win', or 'draw'")
    if type(final_margin_from_black) is not int:
        raise ValueError("example['final_margin_from_black'] must be an int")
    if type(policy_target_index) is not int or not (-1 <= policy_target_index <= 64):
        raise ValueError("example['policy_target_index'] must be an int in -1..64")

    validated_moves: list[int | None] = []
    for move in moves_until_here:
        if move is None:
            validated_moves.append(None)
        elif type(move) is int and 0 <= move <= 63:
            validated_moves.append(move)
        else:
            raise ValueError("example['moves_until_here'] must contain int in 0..63 or None")

    return (
        (black_bits, white_bits, side_to_move),
        ply,
        validated_moves,
        final_result,
        final_margin_from_black,
        policy_target_index,
    )


def supervised_examples_from_trace(trace: dict) -> list[dict[str, object]]:
    return [
        _example_from_parts(parts)
        for parts in _supervised_examples_from_trace_parts(*_trace_to_core_parts(trace))
    ]


def supervised_examples_from_traces(traces: list[dict]) -> list[dict[str, object]]:
    if type(traces) is not list:
        raise ValueError("traces must be a list[dict]")
    return [
        _example_from_parts(parts)
        for parts in _supervised_examples_from_traces_parts(
            [_trace_to_core_parts(trace) for trace in traces]
        )
    ]


def packed_supervised_examples_from_trace(trace: dict) -> list[dict[str, object]]:
    return [
        _packed_example_from_parts(parts)
        for parts in _packed_supervised_examples_from_trace_parts(*_trace_to_core_parts(trace))
    ]


def packed_supervised_examples_from_traces(traces: list[dict]) -> list[dict[str, object]]:
    if type(traces) is not list:
        raise ValueError("traces must be a list[dict]")
    return [
        _packed_example_from_parts(parts)
        for parts in _packed_supervised_examples_from_traces_parts(
            [_trace_to_core_parts(trace) for trace in traces]
        )
    ]


def prepare_planes_learning_batch(examples: list[dict], config: dict) -> dict[str, np.ndarray]:
    if type(examples) is not list:
        raise ValueError("examples must be a list[dict]")
    features, value_targets, policy_targets, legal_move_masks = (
        _prepare_planes_learning_batch_parts(
            [_packed_example_to_core_parts(example) for example in examples],
            *_validate_feature_config(config),
        )
    )
    return {
        "features": features,
        "value_targets": value_targets,
        "policy_targets": policy_targets,
        "legal_move_masks": legal_move_masks,
    }


def prepare_flat_learning_batch(examples: list[dict], config: dict) -> dict[str, np.ndarray]:
    if type(examples) is not list:
        raise ValueError("examples must be a list[dict]")
    features, value_targets, policy_targets, legal_move_masks = _prepare_flat_learning_batch_parts(
        [_packed_example_to_core_parts(example) for example in examples],
        *_validate_feature_config(config),
    )
    return {
        "features": features,
        "value_targets": value_targets,
        "policy_targets": policy_targets,
        "legal_move_masks": legal_move_masks,
    }


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
