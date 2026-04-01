from bisect import bisect_right
from pathlib import Path
from typing import cast, overload

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
    Board as _CoreBoard,
    all_symmetries,
    apply_move as _apply_move_core,
    apply_forced_pass as _apply_forced_pass_core,
    board_from_bits,
    board_status as _board_status_core,
    disc_count as _disc_count_core,
    final_margin_from_black as _final_margin_from_black_core,
    game_result as _game_result_core,
    generate_legal_moves as _generate_legal_moves_core,
    initial_board,
    is_legal_move as _is_legal_move_core,
    legal_moves_list as _legal_moves_list_core,
    pack_board as _pack_board_core,
    transform_board,
    transform_square,
    validate_board as _validate_board_core,
)

Board = _CoreBoard

__all__ = [
    "Board",
    "RecordedBoard",
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
    "RecordDataset",
    "open_game_record_dataset",
    "supervised_examples_from_trace",
    "supervised_examples_from_traces",
    "packed_supervised_examples_from_trace",
    "packed_supervised_examples_from_traces",
    "prepare_planes_learning_batch",
    "prepare_flat_learning_batch",
    "prepare_cnn_model_input",
    "prepare_cnn_model_input_batch",
    "prepare_flat_model_input",
    "prepare_flat_model_input_batch",
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


class RecordedBoard:
    """Immutable board wrapper that also records moves from `start_board`."""

    __slots__ = ("_start_board", "_current_board", "_moves")

    @staticmethod
    def new_initial() -> "RecordedBoard":
        return start_game_recording(initial_board())

    def __init__(self, start_board: Board, current_board: Board, moves: list[int | None]) -> None:
        self._start_board = start_board
        self._current_board = current_board
        self._moves = list(moves)

    @property
    def start_board(self) -> Board:
        return self._start_board

    @property
    def current_board(self) -> Board:
        return self._current_board

    @property
    def black_bits(self) -> int:
        return self.current_board.black_bits

    @property
    def white_bits(self) -> int:
        return self.current_board.white_bits

    @property
    def side_to_move(self) -> str:
        return self.current_board.side_to_move

    @property
    def moves(self) -> list[int | None]:
        return list(self._moves)

    def to_bits(self) -> tuple[int, int, str]:
        return self.current_board.to_bits()

    def apply_move(self, square: int) -> "RecordedBoard":
        return _recording_from_parts(
            _record_move_parts(
                self.start_board.to_bits(),
                self.current_board.to_bits(),
                self.moves,
                _validate_u16(square, "square"),
            )
        )

    def apply_forced_pass(self) -> "RecordedBoard":
        return _recording_from_parts(
            _record_pass_parts(
                self.start_board.to_bits(),
                self.current_board.to_bits(),
                self.moves,
            )
        )

    def generate_legal_moves(self) -> int:
        return generate_legal_moves(self)

    def legal_moves_list(self) -> list[int]:
        return legal_moves_list(self)

    def is_legal_move(self, square: int) -> bool:
        return is_legal_move(self, square)

    def board_status(self) -> str:
        return board_status(self)

    def disc_count(self) -> tuple[int, int, int]:
        return disc_count(self)

    def game_result(self) -> str:
        return game_result(self)

    def final_margin_from_black(self) -> int:
        return final_margin_from_black(self)

    def transform(self, sym: str) -> Board:
        return transform_board(self.current_board, sym)

    def encode_planes(
        self,
        history: list[Board],
        config: dict,
    ) -> np.ndarray:
        return encode_planes(self.current_board, history, config)

    def encode_flat_features(
        self,
        history: list[Board],
        config: dict,
    ) -> np.ndarray:
        return encode_flat_features(self.current_board, history, config)

    def prepare_cnn_model_input(self) -> np.ndarray:
        return prepare_cnn_model_input(self)

    def prepare_flat_model_input(self) -> np.ndarray:
        return prepare_flat_model_input(self)

    def to_dict(self) -> dict[str, object]:
        """Return the in-progress recording as a plain dict.

        This is not a finished game record. The result contains the current board state
        together with the original `start_board` and the moves recorded so far.
        """
        return {
            "start_board": self.start_board,
            "current_board": self.current_board,
            "moves": self.moves,
        }

    def finish(self) -> dict[str, object]:
        """Return a completed game record dict.

        The current position must be terminal. The returned dict is the serializable
        one-game record format used by `save_record()` and `append_game_record()`.
        """
        return finish_game_recording(self)

    def save_record(self, path: str) -> None:
        """Append the finished game record to `path` as one JSONL record."""
        append_game_record(path, self)


class RecordDataset:
    """Indexable view over append-only game record JSONL.

    `len(dataset)` counts only policy-enabled positions.
    Pass and policy-invalid plies are excluded from the index.
    """

    __slots__ = ("_records", "_cumulative_positions")

    def __init__(self, records: list[dict[str, object]]) -> None:
        validated_records = [_validate_game_record(record) for record in records]
        cumulative: list[int] = []
        total = 0
        for record in validated_records:
            total += sum(1 for move in cast(list[object], record["moves"]) if move is not None)
            cumulative.append(total)
        self._records = validated_records
        self._cumulative_positions = cumulative

    def __len__(self) -> int:
        return self._cumulative_positions[-1] if self._cumulative_positions else 0

    def len(self) -> int:
        return len(self)

    def __getitem__(self, global_index: int) -> dict[str, object]:
        return self.get(global_index)

    def _resolve_index(self, global_index: int) -> tuple[int, int]:
        if type(global_index) is not int:
            raise TypeError("global_index must be an int")
        if global_index < 0 or global_index >= len(self):
            raise IndexError("global_index out of range")
        record_index = bisect_right(self._cumulative_positions, global_index)
        previous_total = 0 if record_index == 0 else self._cumulative_positions[record_index - 1]
        within_record_index = global_index - previous_total
        moves = cast(list[object], self._records[record_index]["moves"])
        seen = -1
        for ply, move in enumerate(moves):
            if move is not None:
                seen += 1
                if seen == within_record_index:
                    return record_index, ply
        raise IndexError("resolved index does not map to a policy-enabled ply")

    def _board_at(self, record_index: int, ply: int) -> Board:
        record = self._records[record_index]
        board = unpack_board(cast(tuple[int, int, str], record["start_board"]))
        moves = cast(list[object], record["moves"])
        for move in moves[:ply]:
            if move is None:
                board = board.apply_forced_pass()
            else:
                board = board.apply_move(cast(int, move))
        return board

    def get(self, global_index: int) -> dict[str, object]:
        """Return one indexed position.

        The returned `board` is always the current `Board` at that ply.
        """
        record_index, ply = self._resolve_index(global_index)
        record = self._records[record_index]
        board = self._board_at(record_index, ply)
        return {
            "board": board,
            "record_index": record_index,
            "ply": ply,
            "global_index": global_index,
            "policy_target_index": cast(int, cast(list[object], record["moves"])[ply]),
            "final_result": cast(str, record["final_result"]),
            "final_margin_from_black": cast(int, record["final_margin_from_black"]),
        }

    def get_cnn_input(self, global_index: int) -> np.ndarray:
        """Return one `(3, 8, 8)` CNN input sample."""
        board = cast(Board, self.get(global_index)["board"])
        return board.prepare_cnn_model_input()[0]

    def get_flat_input(self, global_index: int) -> np.ndarray:
        """Return one `(192,)` flat input sample."""
        board = cast(Board, self.get(global_index)["board"])
        return board.prepare_flat_model_input()[0]

    def get_targets(self, global_index: int) -> dict[str, object]:
        """Return `value_target` and `(64,)` float32 one-hot `policy_target`."""
        item = self.get(global_index)
        board = cast(Board, item["board"])
        final_margin_from_black = cast(int, item["final_margin_from_black"])
        side_to_move_margin = (
            final_margin_from_black if board.side_to_move == "black" else -final_margin_from_black
        )
        policy_target = np.zeros((64,), dtype=np.float32)
        policy_target[cast(int, item["policy_target_index"])] = 1.0
        return {
            "value_target": np.float32(side_to_move_margin / 64.0),
            "policy_target": policy_target,
        }


def _recording_from_parts(
    parts: tuple[tuple[int, int, str], tuple[int, int, str], list[int | None]],
) -> RecordedBoard:
    start_board_bits, current_board_bits, moves = parts
    return RecordedBoard(Board(*start_board_bits), Board(*current_board_bits), moves)


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


def _validate_game_record(record: object) -> dict[str, object]:
    (
        start_board,
        moves,
        final_result,
        final_black_discs,
        final_white_discs,
        final_empty_discs,
        final_margin_from_black,
    ) = _game_record_to_core_parts(record)
    return {
        "start_board": start_board,
        "moves": moves,
        "final_result": final_result,
        "final_black_discs": final_black_discs,
        "final_white_discs": final_white_discs,
        "final_empty_discs": final_empty_discs,
        "final_margin_from_black": final_margin_from_black,
    }


def _normalize_record_dataset_paths(paths: object) -> list[str]:
    if isinstance(paths, (str, Path)):
        return [str(paths)]
    if type(paths) is list:
        normalized: list[str] = []
        for path in cast(list[object], paths):
            if isinstance(path, (str, Path)):
                normalized.append(str(path))
            else:
                raise TypeError("paths must be a path or a list of paths")
        return normalized
    raise TypeError("paths must be a path or a list of paths")


def _board_from_board_or_record(value: object) -> Board:
    if isinstance(value, Board):
        return value
    if isinstance(value, RecordedBoard):
        return value.current_board
    if type(value) is dict:
        typed_record = _validate_recording(value)
        current = typed_record.get("current_board")
        if isinstance(current, Board):
            return current
    raise TypeError("value must be a Board or RecordedBoard")


def generate_legal_moves(board_or_record: object) -> int:
    return _generate_legal_moves_core(_board_from_board_or_record(board_or_record))


def validate_board(board_or_record: object) -> None:
    _validate_board_core(_board_from_board_or_record(board_or_record))


def legal_moves_list(board_or_record: object) -> list[int]:
    return _legal_moves_list_core(_board_from_board_or_record(board_or_record))


def is_legal_move(board_or_record: object, square: int) -> bool:
    return _is_legal_move_core(
        _board_from_board_or_record(board_or_record),
        _validate_u16(square, "square"),
    )


@overload
def apply_move(board_or_record: Board, square: int) -> Board: ...


@overload
def apply_move(board_or_record: RecordedBoard, square: int) -> RecordedBoard: ...


def apply_move(board_or_record: object, square: int) -> Board | RecordedBoard:
    if isinstance(board_or_record, RecordedBoard):
        return board_or_record.apply_move(square)
    return _apply_move_core(
        _board_from_board_or_record(board_or_record),
        _validate_u16(square, "square"),
    )


@overload
def apply_forced_pass(board_or_record: Board) -> Board: ...


@overload
def apply_forced_pass(board_or_record: RecordedBoard) -> RecordedBoard: ...


def apply_forced_pass(board_or_record: object) -> Board | RecordedBoard:
    if isinstance(board_or_record, RecordedBoard):
        return board_or_record.apply_forced_pass()
    return _apply_forced_pass_core(_board_from_board_or_record(board_or_record))


def board_status(board_or_record: object) -> str:
    return _board_status_core(_board_from_board_or_record(board_or_record))


def disc_count(board_or_record: object) -> tuple[int, int, int]:
    return _disc_count_core(_board_from_board_or_record(board_or_record))


def game_result(board_or_record: object) -> str:
    return _game_result_core(_board_from_board_or_record(board_or_record))


def final_margin_from_black(board_or_record: object) -> int:
    return _final_margin_from_black_core(_board_from_board_or_record(board_or_record))


def pack_board(board_or_record: object) -> tuple[int, int, str]:
    return _pack_board_core(_board_from_board_or_record(board_or_record))


def _board_apply_move(self: Board, square: int) -> Board:
    result = apply_move(self, square)
    assert isinstance(result, Board)
    return result


def _board_apply_forced_pass(self: Board) -> Board:
    result = apply_forced_pass(self)
    assert isinstance(result, Board)
    return result


def _board_generate_legal_moves(self: Board) -> int:
    return generate_legal_moves(self)


def _board_legal_moves_list(self: Board) -> list[int]:
    return legal_moves_list(self)


def _board_is_legal_move(self: Board, square: int) -> bool:
    return is_legal_move(self, square)


def _board_board_status(self: Board) -> str:
    return board_status(self)


def _board_disc_count(self: Board) -> tuple[int, int, int]:
    return disc_count(self)


def _board_game_result(self: Board) -> str:
    return game_result(self)


def _board_final_margin_from_black(self: Board) -> int:
    return final_margin_from_black(self)


def _board_transform(self: Board, sym: str) -> Board:
    return transform_board(self, sym)


def _board_encode_planes(
    self: Board,
    history: list[Board],
    config: dict,
) -> np.ndarray:
    return encode_planes(self, history, config)


def _board_encode_flat_features(
    self: Board,
    history: list[Board],
    config: dict,
) -> np.ndarray:
    return encode_flat_features(self, history, config)


def _board_prepare_cnn_model_input(self: Board) -> np.ndarray:
    return prepare_cnn_model_input(self)


def _board_prepare_flat_model_input(self: Board) -> np.ndarray:
    return prepare_flat_model_input(self)


Board.apply_move = _board_apply_move  # type: ignore[attr-defined]
Board.apply_forced_pass = _board_apply_forced_pass  # type: ignore[attr-defined]
Board.generate_legal_moves = _board_generate_legal_moves  # type: ignore[attr-defined]
Board.legal_moves_list = _board_legal_moves_list  # type: ignore[attr-defined]
Board.is_legal_move = _board_is_legal_move  # type: ignore[attr-defined]
Board.board_status = _board_board_status  # type: ignore[attr-defined]
Board.disc_count = _board_disc_count  # type: ignore[attr-defined]
Board.game_result = _board_game_result  # type: ignore[attr-defined]
Board.final_margin_from_black = _board_final_margin_from_black  # type: ignore[attr-defined]
Board.transform = _board_transform  # type: ignore[attr-defined]
Board.encode_planes = _board_encode_planes  # type: ignore[attr-defined]
Board.encode_flat_features = _board_encode_flat_features  # type: ignore[attr-defined]
Board.prepare_cnn_model_input = _board_prepare_cnn_model_input  # type: ignore[attr-defined]
Board.prepare_flat_model_input = _board_prepare_flat_model_input  # type: ignore[attr-defined]


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


def start_game_recording(start_board: Board) -> RecordedBoard:
    return _recording_from_parts(_start_game_recording_parts(start_board))


def record_move(record: RecordedBoard, square: int) -> RecordedBoard:
    if not isinstance(record, RecordedBoard):
        raise ValueError("record must be a RecordedBoard")
    return record.apply_move(square)


def record_pass(record: RecordedBoard) -> RecordedBoard:
    if not isinstance(record, RecordedBoard):
        raise ValueError("record must be a RecordedBoard")
    return record.apply_forced_pass()


def current_board(record: RecordedBoard) -> Board:
    if not isinstance(record, RecordedBoard):
        raise ValueError("record must be a RecordedBoard")
    return record.current_board


def finish_game_recording(record: RecordedBoard) -> dict[str, object]:
    if not isinstance(record, RecordedBoard):
        raise ValueError("record must be a RecordedBoard")
    return _game_record_from_parts(_finish_game_recording_parts(*_recording_to_core_parts(record)))


def append_game_record(path: str, record: object) -> None:
    if type(path) is not str:
        raise ValueError("path must be a str")
    if isinstance(record, RecordedBoard):
        record = finish_game_recording(record)
    _append_game_record_parts(path, *_game_record_to_core_parts(record))


def load_game_records(path: str) -> list[dict[str, object]]:
    if type(path) is not str:
        raise ValueError("path must be a str")
    return [_game_record_from_parts(parts) for parts in _load_game_records_parts(path)]


def open_game_record_dataset(paths: object) -> RecordDataset:
    """Open one or more append-only game record JSONL files as one position dataset."""
    records: list[dict[str, object]] = []
    for path in _normalize_record_dataset_paths(paths):
        records.extend(load_game_records(path))
    return RecordDataset(records)


def _boards_from_board_or_record_batch(values: object) -> list[Board]:
    if type(values) is not list:
        raise ValueError("values must be a list[Board | RecordedBoard]")
    return [_board_from_board_or_record(value) for value in cast(list[object], values)]


def _cnn_planes_for_board(board: Board) -> np.ndarray:
    self_plane = np.zeros((8, 8), dtype=np.float32)
    opp_plane = np.zeros((8, 8), dtype=np.float32)
    legal_plane = np.zeros((8, 8), dtype=np.float32)

    if board.side_to_move == "black":
        self_bits = board.black_bits
        opp_bits = board.white_bits
    else:
        self_bits = board.white_bits
        opp_bits = board.black_bits
    legal_bits = generate_legal_moves(board)

    for plane, bits in (
        (self_plane, self_bits),
        (opp_plane, opp_bits),
        (legal_plane, legal_bits),
    ):
        current_bits = bits
        while current_bits != 0:
            square = (current_bits & -current_bits).bit_length() - 1
            plane[square // 8, square % 8] = 1.0
            current_bits &= current_bits - 1

    return np.stack((self_plane, opp_plane, legal_plane), axis=0)


def _flat_features_for_board(board: Board) -> np.ndarray:
    planes = _cnn_planes_for_board(board)
    return np.concatenate(
        (
            planes[0].reshape(64),
            planes[1].reshape(64),
            planes[2].reshape(64),
        )
    ).astype(np.float32, copy=False)


def prepare_cnn_model_input(board_or_record: object) -> np.ndarray:
    board = _board_from_board_or_record(board_or_record)
    return _cnn_planes_for_board(board)[np.newaxis, ...]


def prepare_cnn_model_input_batch(values: list[object]) -> np.ndarray:
    boards = _boards_from_board_or_record_batch(values)
    return np.stack([_cnn_planes_for_board(board) for board in boards], axis=0).astype(
        np.float32,
        copy=False,
    )


def prepare_flat_model_input(board_or_record: object) -> np.ndarray:
    board = _board_from_board_or_record(board_or_record)
    return _flat_features_for_board(board)[np.newaxis, ...]


def prepare_flat_model_input_batch(values: list[object]) -> np.ndarray:
    boards = _boards_from_board_or_record_batch(values)
    return np.stack([_flat_features_for_board(board) for board in boards], axis=0).astype(
        np.float32,
        copy=False,
    )


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
    record: RecordedBoard,
) -> tuple[tuple[int, int, str], tuple[int, int, str], list[int | None]]:
    if not isinstance(record, RecordedBoard):
        raise ValueError("record must be a RecordedBoard")

    validated_moves: list[int | None] = []
    for move in record.moves:
        if move is None:
            validated_moves.append(None)
        elif type(move) is int and 0 <= move <= 63:
            validated_moves.append(move)
        else:
            raise ValueError("record.moves must contain int in 0..63 or None")

    return record.start_board.to_bits(), record.current_board.to_bits(), validated_moves


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
