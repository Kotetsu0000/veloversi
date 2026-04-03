from bisect import bisect_right
import concurrent.futures
import importlib
from pathlib import Path
import time
from typing import Any, cast, overload

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
    _search_best_move_exact_parts,
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
    "search_best_move_exact",
    "select_move_with_model",
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


def _validate_optional_positive_int(value: object, name: str) -> int | None:
    if value is None:
        return None
    if type(value) is not int or value <= 0:
        raise ValueError(f"{name} must be a positive int or None")
    return value


def _validate_u8(value: object, name: str) -> int:
    if type(value) is not int or not (0 <= value <= 0xFF):
        raise ValueError(f"{name} must be an int in 0..255")
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
        """標準初期局面から recording を開始します。

        Returns:
            開始局面と現在局面がともに初期局面の `RecordedBoard`。

        Example:
            >>> import veloversi as vv
            >>> record = vv.RecordedBoard.new_initial()
            >>> record.legal_moves_list()
            [19, 26, 37, 44]
        """
        return start_game_recording(initial_board())

    def __init__(self, start_board: Board, current_board: Board, moves: list[int | None]) -> None:
        self._start_board = start_board
        self._current_board = current_board
        self._moves = list(moves)

    @property
    def start_board(self) -> Board:
        """recording の開始局面を返します。"""
        return self._start_board

    @property
    def current_board(self) -> Board:
        """recording が現在保持している局面を返します。"""
        return self._current_board

    @property
    def black_bits(self) -> int:
        """現在局面の黒石 bitboard を返します。"""
        return self.current_board.black_bits

    @property
    def white_bits(self) -> int:
        """現在局面の白石 bitboard を返します。"""
        return self.current_board.white_bits

    @property
    def side_to_move(self) -> str:
        """現在局面の手番を `"black"` または `"white"` で返します。"""
        return self.current_board.side_to_move

    @property
    def moves(self) -> list[int | None]:
        """開始局面から現在局面までの手順を返します。

        `None` は forced pass を表します。
        """
        return list(self._moves)

    def to_bits(self) -> tuple[int, int, str]:
        """現在局面を `(black_bits, white_bits, side_to_move)` で返します。"""
        return self.current_board.to_bits()

    def apply_move(self, square: int) -> "RecordedBoard":
        """現在局面に着手し、履歴も更新した新しい recording を返します。

        Args:
            square: 0..63 のマス番号。

        Example:
            >>> import veloversi as vv
            >>> record = vv.start_game_recording(vv.initial_board())
            >>> next_record = record.apply_move(19)
        """
        return _recording_from_parts(
            _record_move_parts(
                self.start_board.to_bits(),
                self.current_board.to_bits(),
                self.moves,
                _validate_u16(square, "square"),
            )
        )

    def apply_forced_pass(self) -> "RecordedBoard":
        """強制パスを適用し、履歴も更新した新しい recording を返します。"""
        return _recording_from_parts(
            _record_pass_parts(
                self.start_board.to_bits(),
                self.current_board.to_bits(),
                self.moves,
            )
        )

    def generate_legal_moves(self) -> int:
        """現在局面の合法手 bitmask を返します。"""
        return generate_legal_moves(self)

    def legal_moves_list(self) -> list[int]:
        """現在局面の合法手を昇順のマス番号 list で返します。"""
        return legal_moves_list(self)

    def is_legal_move(self, square: int) -> bool:
        """現在局面で `square` が合法手なら `True` を返します。"""
        return is_legal_move(self, square)

    def board_status(self) -> str:
        """現在局面の状態を返します。"""
        return board_status(self)

    def disc_count(self) -> tuple[int, int, int]:
        """現在局面の `(black, white, empty)` の石数を返します。"""
        return disc_count(self)

    def game_result(self) -> str:
        """現在局面の石数比較に基づく結果を返します。"""
        return game_result(self)

    def final_margin_from_black(self) -> int:
        """現在局面の `black - white` を返します。"""
        return final_margin_from_black(self)

    def transform(self, sym: str) -> Board:
        """現在局面に対称変換を適用した `Board` を返します。"""
        return transform_board(self.current_board, sym)

    def encode_planes(
        self,
        history: list[Board],
        config: dict,
    ) -> np.ndarray:
        """現在局面を planes feature に変換します。"""
        return encode_planes(self.current_board, history, config)

    def encode_flat_features(
        self,
        history: list[Board],
        config: dict,
    ) -> np.ndarray:
        """現在局面を flat feature に変換します。"""
        return encode_flat_features(self.current_board, history, config)

    def prepare_cnn_model_input(self) -> np.ndarray:
        """現在局面を CNN 向け `(1, 3, 8, 8)` 入力に変換します。"""
        return prepare_cnn_model_input(self)

    def prepare_flat_model_input(self) -> np.ndarray:
        """現在局面を flat/NNUE 風 `(1, 192)` 入力に変換します。"""
        return prepare_flat_model_input(self)

    def search_best_move_exact(
        self,
        timeout_seconds: float = 1.0,
        *,
        worker_count: int | None = None,
        serial_fallback_empty_threshold: int = 18,
        shared_tt_empty_threshold: int = 20,
    ) -> dict[str, object]:
        """現在局面を全探索し、最善手を返します。

        Args:
            timeout_seconds: 探索の制限時間。既定値は `1.0` 秒です。
            worker_count: 並列 worker 数。`None` の場合は自動設定です。
            serial_fallback_empty_threshold: この空き数未満では serial fallback を使います。
            shared_tt_empty_threshold: この空き数以上で shared TT を使います。

        Notes:
            `RecordedBoard` では `current_board` を探索対象にします。
            timeout を超えた場合は partial result ではなく失敗結果を返します。
        """
        return search_best_move_exact(
            self,
            timeout_seconds,
            worker_count=worker_count,
            serial_fallback_empty_threshold=serial_fallback_empty_threshold,
            shared_tt_empty_threshold=shared_tt_empty_threshold,
        )

    def select_move_with_model(
        self,
        model: object,
        depth: int = 1,
        timeout_seconds: float = 1.0,
        *,
        policy_mode: str = "best",
        device: str = "cpu",
        exact_from_empty_threshold: int | None = 16,
        exact_timeout_seconds: float | None = None,
    ) -> dict[str, object]:
        """PyTorch model を使って現在局面の着手を選びます。

        Args:
            model: PyTorch `nn.Module`。
            depth: value 出力時の探索深さ。既定値は `1`。
            timeout_seconds: 探索全体の制限時間。
            policy_mode: `"best"` または `"sample"`。
            device: 推論に使う device。既定値は `"cpu"`。
            exact_from_empty_threshold:
                空き数がこの値以下なら exact 探索を優先します。
                `None` の場合は exact へ切り替えません。
            exact_timeout_seconds:
                exact 探索に使う制限時間。`None` の場合は `timeout_seconds` を使います。

        Notes:
            `RecordedBoard` では常に `current_board` を対象にします。
        """
        return select_move_with_model(
            self,
            model,
            depth,
            timeout_seconds,
            policy_mode=policy_mode,
            device=device,
            exact_from_empty_threshold=exact_from_empty_threshold,
            exact_timeout_seconds=exact_timeout_seconds,
        )

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
        """policy target を持つ局面数を返します。"""
        return self._cumulative_positions[-1] if self._cumulative_positions else 0

    def len(self) -> int:
        """`len(dataset)` と同じ値を返します。"""
        return len(self)

    def __getitem__(self, global_index: int) -> dict[str, object]:
        """`get(global_index)` の別名です。"""
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
        """通し番号で 1 局面の情報を返します。

        Args:
            global_index: dataset 内の 0 始まりの通し番号。

        Returns:
            少なくとも次のキーを含む dict。

            - `board`:
              常に現在局面の `Board`。
            - `record_index`:
              入力ファイル群を連結した後の game record 番号。
            - `ply`:
              `start_board` から数えた 0 始まりの手数。
            - `global_index`:
              dataset 全体での 0 始まり通し番号。
            - `policy_target_index`:
              次手の着手位置。常に `0..63`。
              pass や policy 無効局面は dataset index から除外されます。
            - `final_result`:
              `"black"`, `"white"`, `"draw"` のいずれか。
            - `final_margin_from_black`:
              終局時の `black - white`。
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
        """通し番号で 1 局面の `(3, 8, 8)` CNN 入力を返します。"""
        board = cast(Board, self.get(global_index)["board"])
        return board.prepare_cnn_model_input()[0]

    def get_flat_input(self, global_index: int) -> np.ndarray:
        """通し番号で 1 局面の `(192,)` flat 入力を返します。"""
        board = cast(Board, self.get(global_index)["board"])
        return board.prepare_flat_model_input()[0]

    def get_targets(self, global_index: int) -> dict[str, object]:
        """通し番号で 1 局面の教師データを返します。

        Returns:
            `value_target`:
                現在手番視点の最終石差を `[-1, 1]` に正規化した `np.float32`。
                計算式は `final_margin_from_side_to_move / 64.0` です。
            `policy_target`:
                shape `(64,)` の one-hot `numpy.ndarray(float32)`。
                `policy_target_index` に対応する要素だけが `1.0` です。
        """
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
    """`(black_bits, white_bits, side_to_move)` から `Board` を復元します。

    Example:
        >>> import veloversi as vv
        >>> board = vv.unpack_board((34628173824, 68853694464, "black"))
    """
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
    """合法手 bitmask を返します。

    Args:
        board_or_record: `Board` または `RecordedBoard`。
    """
    return _generate_legal_moves_core(_board_from_board_or_record(board_or_record))


def validate_board(board_or_record: object) -> None:
    """基本的な整合性を検証し、異常なら例外を送出します。"""
    _validate_board_core(_board_from_board_or_record(board_or_record))


def legal_moves_list(board_or_record: object) -> list[int]:
    """合法手を昇順のマス番号 list で返します。"""
    return _legal_moves_list_core(_board_from_board_or_record(board_or_record))


def is_legal_move(board_or_record: object, square: int) -> bool:
    """`square` が合法手なら `True` を返します。"""
    return _is_legal_move_core(
        _board_from_board_or_record(board_or_record),
        _validate_u16(square, "square"),
    )


@overload
def apply_move(board_or_record: Board, square: int) -> Board: ...


@overload
def apply_move(board_or_record: RecordedBoard, square: int) -> RecordedBoard: ...


def apply_move(board_or_record: object, square: int) -> Board | RecordedBoard:
    """着手後の新しい `Board` または `RecordedBoard` を返します。

    `RecordedBoard` を渡した場合は、現在局面の更新に加えて履歴も更新します。
    """
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
    """強制パス後の新しい `Board` または `RecordedBoard` を返します。"""
    if isinstance(board_or_record, RecordedBoard):
        return board_or_record.apply_forced_pass()
    return _apply_forced_pass_core(_board_from_board_or_record(board_or_record))


def board_status(board_or_record: object) -> str:
    """局面状態を返します。"""
    return _board_status_core(_board_from_board_or_record(board_or_record))


def disc_count(board_or_record: object) -> tuple[int, int, int]:
    """`(black, white, empty)` を返します。"""
    return _disc_count_core(_board_from_board_or_record(board_or_record))


def game_result(board_or_record: object) -> str:
    """現在局面の石数比較に基づく結果を返します。"""
    return _game_result_core(_board_from_board_or_record(board_or_record))


def final_margin_from_black(board_or_record: object) -> int:
    """`black - white` の石差を返します。"""
    return _final_margin_from_black_core(_board_from_board_or_record(board_or_record))


def pack_board(board_or_record: object) -> tuple[int, int, str]:
    """局面を `(black_bits, white_bits, side_to_move)` に変換します。"""
    return _pack_board_core(_board_from_board_or_record(board_or_record))


def _board_apply_move(self: Board, square: int) -> Board:
    """着手後の新しい盤面を返します。

    Example:
        >>> import veloversi as vv
        >>> board = vv.initial_board()
        >>> next_board = board.apply_move(19)
    """
    result = apply_move(self, square)
    assert isinstance(result, Board)
    return result


def _board_apply_forced_pass(self: Board) -> Board:
    """強制パス後の新しい盤面を返します。"""
    result = apply_forced_pass(self)
    assert isinstance(result, Board)
    return result


def _board_generate_legal_moves(self: Board) -> int:
    """合法手 bitmask を返します。"""
    return generate_legal_moves(self)


def _board_legal_moves_list(self: Board) -> list[int]:
    """合法手を昇順のマス番号 list で返します。"""
    return legal_moves_list(self)


def _board_is_legal_move(self: Board, square: int) -> bool:
    """`square` が合法手なら `True` を返します。"""
    return is_legal_move(self, square)


def _board_board_status(self: Board) -> str:
    """局面状態を返します。"""
    return board_status(self)


def _board_disc_count(self: Board) -> tuple[int, int, int]:
    """`(black, white, empty)` を返します。"""
    return disc_count(self)


def _board_game_result(self: Board) -> str:
    """現在局面の石数比較に基づく結果を返します。"""
    return game_result(self)


def _board_final_margin_from_black(self: Board) -> int:
    """`black - white` の石差を返します。"""
    return final_margin_from_black(self)


def _board_transform(self: Board, sym: str) -> Board:
    """対称変換後の新しい盤面を返します。"""
    return transform_board(self, sym)


def _board_encode_planes(
    self: Board,
    history: list[Board],
    config: dict,
) -> np.ndarray:
    """盤面を planes feature に変換します。"""
    return encode_planes(self, history, config)


def _board_encode_flat_features(
    self: Board,
    history: list[Board],
    config: dict,
) -> np.ndarray:
    """盤面を flat feature に変換します。"""
    return encode_flat_features(self, history, config)


def _board_prepare_cnn_model_input(self: Board) -> np.ndarray:
    """盤面を CNN 向け `(1, 3, 8, 8)` 入力に変換します。"""
    return prepare_cnn_model_input(self)


def _board_prepare_flat_model_input(self: Board) -> np.ndarray:
    """盤面を flat/NNUE 風 `(1, 192)` 入力に変換します。"""
    return prepare_flat_model_input(self)


def _board_search_best_move_exact(
    self: Board,
    timeout_seconds: float = 1.0,
    *,
    worker_count: int | None = None,
    serial_fallback_empty_threshold: int = 18,
    shared_tt_empty_threshold: int = 20,
) -> dict[str, object]:
    """盤面を全探索し、最善手を返します。

    timeout を超えた場合は partial result ではなく失敗結果を返します。
    """
    return search_best_move_exact(
        self,
        timeout_seconds,
        worker_count=worker_count,
        serial_fallback_empty_threshold=serial_fallback_empty_threshold,
        shared_tt_empty_threshold=shared_tt_empty_threshold,
    )


def _board_select_move_with_model(
    self: Board,
    model: object,
    depth: int = 1,
    timeout_seconds: float = 1.0,
    *,
    policy_mode: str = "best",
    device: str = "cpu",
    exact_from_empty_threshold: int | None = 16,
    exact_timeout_seconds: float | None = None,
) -> dict[str, object]:
    """PyTorch model を使って盤面の着手を選びます。"""
    return select_move_with_model(
        self,
        model,
        depth,
        timeout_seconds,
        policy_mode=policy_mode,
        device=device,
        exact_from_empty_threshold=exact_from_empty_threshold,
        exact_timeout_seconds=exact_timeout_seconds,
    )


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
Board.search_best_move_exact = _board_search_best_move_exact  # type: ignore[attr-defined]
Board.select_move_with_model = _board_select_move_with_model  # type: ignore[attr-defined]


def play_random_game(seed: int, config: dict) -> dict:
    """初期局面からランダムに対局を進め、trace を返します。

    Args:
        seed: 乱数 seed。
        config: `{\"max_plies\": int | None}` を含む設定 dict。
    """
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
    """初期局面から合法手で `plies` 手進めた開始局面を返します。"""
    return Board(*_random_start_board_parts(seed, _validate_u16(plies, "plies")))


def sample_reachable_positions(seed: int, config: dict) -> list[Board]:
    """到達可能局面をランダムサンプリングします。"""
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
    """任意局面を開始点として recording を開始します。

    Example:
        >>> import veloversi as vv
        >>> start = vv.random_start_board(plies=6, seed=123)
        >>> record = vv.start_game_recording(start)
    """
    return _recording_from_parts(_start_game_recording_parts(start_board))


def record_move(record: RecordedBoard, square: int) -> RecordedBoard:
    """`RecordedBoard.apply_move()` の関数版です。"""
    if not isinstance(record, RecordedBoard):
        raise ValueError("record must be a RecordedBoard")
    return record.apply_move(square)


def record_pass(record: RecordedBoard) -> RecordedBoard:
    """`RecordedBoard.apply_forced_pass()` の関数版です。"""
    if not isinstance(record, RecordedBoard):
        raise ValueError("record must be a RecordedBoard")
    return record.apply_forced_pass()


def current_board(record: RecordedBoard) -> Board:
    """recording が保持している現在局面を返します。"""
    if not isinstance(record, RecordedBoard):
        raise ValueError("record must be a RecordedBoard")
    return record.current_board


def finish_game_recording(record: RecordedBoard) -> dict[str, object]:
    """終局済み recording を完成 game record に変換します。"""
    if not isinstance(record, RecordedBoard):
        raise ValueError("record must be a RecordedBoard")
    return _game_record_from_parts(_finish_game_recording_parts(*_recording_to_core_parts(record)))


def append_game_record(path: str, record: object) -> None:
    """game record を JSONL に 1 行追記します。

    `record` に `RecordedBoard` を渡した場合は、内部で `finish()` 相当を行います。
    """
    if type(path) is not str:
        raise ValueError("path must be a str")
    if isinstance(record, RecordedBoard):
        record = finish_game_recording(record)
    _append_game_record_parts(path, *_game_record_to_core_parts(record))


def load_game_records(path: str) -> list[dict[str, object]]:
    """JSONL から game record を全件読み込みます。"""
    if type(path) is not str:
        raise ValueError("path must be a str")
    return [_game_record_from_parts(parts) for parts in _load_game_records_parts(path)]


def open_game_record_dataset(paths: object) -> RecordDataset:
    """JSONL の game record 群を局面 index 付き dataset として開きます。

    Args:
        paths: 単一 path、または path の list。

    Notes:
        - index 対象は policy target を持つ局面のみです。
        - pass 局面は dataset index から除外されます。
        - JSONL は append-only 前提です。
    """
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
    """現在局面から CNN 向け `(1, 3, 8, 8)` 入力を作ります。

    `Board` と `RecordedBoard` の両方を受け取ります。
    """
    board = _board_from_board_or_record(board_or_record)
    return _cnn_planes_for_board(board)[np.newaxis, ...]


def prepare_cnn_model_input_batch(values: list[object]) -> np.ndarray:
    """複数の盤面/recording から `(B, 3, 8, 8)` 入力を作ります。"""
    boards = _boards_from_board_or_record_batch(values)
    return np.stack([_cnn_planes_for_board(board) for board in boards], axis=0).astype(
        np.float32,
        copy=False,
    )


def prepare_flat_model_input(board_or_record: object) -> np.ndarray:
    """現在局面から flat/NNUE 風 `(1, 192)` 入力を作ります。"""
    board = _board_from_board_or_record(board_or_record)
    return _flat_features_for_board(board)[np.newaxis, ...]


def prepare_flat_model_input_batch(values: list[object]) -> np.ndarray:
    """複数の盤面/recording から `(B, 192)` 入力を作ります。"""
    boards = _boards_from_board_or_record_batch(values)
    return np.stack([_flat_features_for_board(board) for board in boards], axis=0).astype(
        np.float32,
        copy=False,
    )


def search_best_move_exact(
    board_or_record: object,
    timeout_seconds: float = 1.0,
    *,
    worker_count: int | None = None,
    serial_fallback_empty_threshold: int = 18,
    shared_tt_empty_threshold: int = 20,
) -> dict[str, object]:
    """全探索で最善手を探します。

    Args:
        board_or_record: `Board` または `RecordedBoard`。
        timeout_seconds: 探索の制限時間。既定値は `1.0` 秒です。
        worker_count: 並列 worker 数。`None` の場合は自動設定です。
        serial_fallback_empty_threshold: この空き数未満では serial fallback を使います。
        shared_tt_empty_threshold: この空き数以上で shared TT を使います。

    Returns:
        次のキーを持つ dict。

        - `success`: 探索成功なら `True`
        - `best_move`: 最善手。失敗時は `None`
        - `exact_margin`: 現在手番視点の厳密石差。失敗時は `None`
        - `pv`: 読み筋。失敗時は空 list
        - `searched_nodes`: 探索ノード数
        - `elapsed_seconds`: 実行時間
        - `failure_reason`: 失敗理由。成功時は `None`
    """
    if not isinstance(timeout_seconds, (int, float)) or not np.isfinite(timeout_seconds):
        raise ValueError("timeout_seconds must be a finite float >= 0.0")
    timeout_value = float(timeout_seconds)
    if timeout_value < 0.0:
        raise ValueError("timeout_seconds must be a finite float >= 0.0")
    worker_count_value = _validate_optional_positive_int(worker_count, "worker_count")
    serial_threshold = _validate_u8(
        serial_fallback_empty_threshold, "serial_fallback_empty_threshold"
    )
    shared_threshold = _validate_u8(shared_tt_empty_threshold, "shared_tt_empty_threshold")
    if shared_threshold < serial_threshold:
        raise ValueError("shared_tt_empty_threshold must be >= serial_fallback_empty_threshold")

    board = _board_from_board_or_record(board_or_record)
    start = time.perf_counter()
    success, best_move, exact_margin, pv, searched_nodes, failure_reason = (
        _search_best_move_exact_parts(
            board,
            timeout_value,
            worker_count_value,
            serial_threshold,
            shared_threshold,
        )
    )
    elapsed_seconds = time.perf_counter() - start
    return {
        "success": success,
        "best_move": best_move,
        "exact_margin": exact_margin,
        "pv": list(pv),
        "searched_nodes": searched_nodes,
        "elapsed_seconds": elapsed_seconds,
        "failure_reason": failure_reason,
    }


def _import_torch() -> object:
    try:
        return importlib.import_module("torch")
    except ModuleNotFoundError as exc:
        raise RuntimeError(
            "select_move_with_model を使うには PyTorch (`torch`) の導入が必要です"
        ) from exc


def _validate_timeout_seconds(value: object, name: str) -> float:
    if not isinstance(value, (int, float)) or not np.isfinite(value):
        raise ValueError(f"{name} must be a finite float >= 0.0")
    timeout_seconds = float(value)
    if timeout_seconds < 0.0:
        raise ValueError(f"{name} must be a finite float >= 0.0")
    return timeout_seconds


def _validate_positive_depth(value: object) -> int:
    if type(value) is not int or value <= 0:
        raise ValueError("depth must be a positive int")
    return value


def _validate_policy_mode(value: object) -> str:
    if type(value) is not str or value not in {"best", "sample"}:
        raise ValueError("policy_mode must be 'best' or 'sample'")
    return value


def _validate_device(value: object) -> str:
    if type(value) is not str or value == "":
        raise ValueError("device must be a non-empty str")
    return value


def _validate_optional_exact_threshold(value: object) -> int | None:
    return _validate_optional_positive_int(value, "exact_from_empty_threshold")


def _validate_torch_model(model: object, torch_module: object) -> None:
    nn_module = getattr(getattr(torch_module, "nn", None), "Module", None)
    if nn_module is None or not isinstance(model, nn_module):
        raise TypeError("model must be a torch.nn.Module")


def _torch_tensor_from_numpy(torch_module: object, array: np.ndarray, device: str) -> object:
    tensor = getattr(torch_module, "from_numpy")(np.ascontiguousarray(array, dtype=np.float32))
    if hasattr(tensor, "to"):
        tensor = tensor.to(device)
    return tensor


def _torch_output_to_numpy(output: object) -> np.ndarray:
    if hasattr(output, "detach") and hasattr(output, "cpu") and hasattr(output, "numpy"):
        detached = cast(Any, output).detach()
        cpu_value = detached.cpu() if hasattr(detached, "cpu") else detached
        return np.asarray(cpu_value.numpy(), dtype=np.float32)
    return np.asarray(output, dtype=np.float32)


def _classify_model_output(output: object) -> tuple[str, np.ndarray | np.float32]:
    array = _torch_output_to_numpy(output)
    if array.ndim == 0:
        return "value", np.float32(array)
    if array.ndim == 1:
        if array.shape == (64,):
            return "policy", array.astype(np.float32, copy=False)
        if array.shape == (1,):
            return "value", np.float32(array[0])
    if array.ndim == 2:
        if array.shape == (1, 64):
            return "policy", array[0].astype(np.float32, copy=False)
        if array.shape == (1, 1):
            return "value", np.float32(array[0, 0])
    raise ValueError("model output shape must be scalar, (1,), (1, 1), (64,), or (1, 64)")


def _run_model_once(
    model: object,
    board: Board,
    input_format: str,
    torch_module: object,
    device: str,
) -> tuple[str, np.ndarray | np.float32]:
    if input_format == "cnn":
        input_array = prepare_cnn_model_input(board)
    elif input_format == "flat":
        input_array = prepare_flat_model_input(board)
    else:
        raise ValueError("input_format must be 'cnn' or 'flat'")

    tensor = _torch_tensor_from_numpy(torch_module, input_array, device)
    output = cast(Any, model)(tensor)
    return _classify_model_output(output)


def _detect_model_io(
    model: object,
    board: Board,
    torch_module: object,
    device: str,
) -> tuple[str, str, np.ndarray | np.float32]:
    successes: list[tuple[str, str, np.ndarray | np.float32]] = []
    errors: list[str] = []
    for input_format in ("cnn", "flat"):
        try:
            output_format, value = _run_model_once(model, board, input_format, torch_module, device)
            successes.append((input_format, output_format, value))
        except Exception as exc:
            errors.append(f"{input_format}: {exc}")

    if not successes:
        joined = "; ".join(errors)
        raise ValueError(f"model does not accept cnn or flat input: {joined}")
    if len(successes) > 1:
        raise ValueError("model accepts both cnn and flat input; input format is ambiguous")
    input_format, output_format, root_output = successes[0]
    return input_format, output_format, root_output


def _is_probability_distribution(values: np.ndarray) -> bool:
    if values.size == 0:
        return False
    if np.any(values < -1e-5):
        return False
    total = float(values.sum())
    return abs(total - 1.0) <= 1e-4


def _softmax(values: np.ndarray) -> np.ndarray:
    shifted = values - np.max(values)
    exp_values = np.exp(shifted).astype(np.float32, copy=False)
    total = exp_values.sum(dtype=np.float32)
    if not np.isfinite(total) or total <= 0.0:
        raise ValueError("policy logits produced an invalid softmax distribution")
    return (exp_values / total).astype(np.float32, copy=False)


def _policy_distribution_for_board(
    raw_policy: np.ndarray,
    legal_moves: list[int],
) -> np.ndarray:
    legal_values = raw_policy[np.asarray(legal_moves, dtype=np.intp)].astype(np.float32, copy=False)
    if _is_probability_distribution(legal_values):
        legal_probs = legal_values.astype(np.float32, copy=False)
    else:
        legal_probs = _softmax(legal_values)
    distribution = np.zeros((64,), dtype=np.float32)
    distribution[np.asarray(legal_moves, dtype=np.intp)] = legal_probs
    return distribution


def _terminal_value_from_side_to_move(board: Board) -> float:
    margin = float(final_margin_from_black(board))
    if board.side_to_move == "white":
        margin = -margin
    return margin / 64.0


class _ModelSearchTimeout(Exception):
    pass


def _value_search_negamax(
    board: Board,
    depth: int,
    deadline: float,
    evaluate: object,
    searched_nodes: list[int],
    alpha: float,
    beta: float,
) -> tuple[float, list[int]]:
    if time.perf_counter() >= deadline:
        raise _ModelSearchTimeout

    status = board.board_status()
    if status == "terminal":
        return _terminal_value_from_side_to_move(board), []
    if status == "forced_pass":
        child_value, child_pv = _value_search_negamax(
            board.apply_forced_pass(), depth, deadline, evaluate, searched_nodes, -beta, -alpha
        )
        return -child_value, child_pv
    if depth == 0:
        searched_nodes[0] += 1
        evaluator = cast(Any, evaluate)
        return float(evaluator(board)), []

    best_value = -float("inf")
    best_pv: list[int] = []
    for move in board.legal_moves_list():
        if time.perf_counter() >= deadline:
            raise _ModelSearchTimeout
        child_value, child_pv = _value_search_negamax(
            board.apply_move(move),
            depth - 1,
            deadline,
            evaluate,
            searched_nodes,
            -beta,
            -alpha,
        )
        score = -child_value
        if score > best_value:
            best_value = score
            best_pv = [move, *child_pv]
        if score > alpha:
            alpha = score
        if alpha >= beta:
            break
    return best_value, best_pv


def _success_result(
    *,
    best_move: int | None,
    elapsed_seconds: float,
    input_format: str | None,
    output_format: str,
    source: str,
    forced_pass: bool = False,
    pv: list[int] | None = None,
    searched_nodes: int = 0,
    value: float | None = None,
    policy: np.ndarray | None = None,
    selected_probability: float | None = None,
    exact_margin: int | None = None,
    timeout_reached: bool = False,
) -> dict[str, object]:
    return {
        "success": True,
        "best_move": best_move,
        "value": value,
        "policy": policy,
        "pv": [] if pv is None else list(pv),
        "searched_nodes": searched_nodes,
        "elapsed_seconds": elapsed_seconds,
        "failure_reason": None,
        "input_format": input_format,
        "output_format": output_format,
        "source": source,
        "forced_pass": forced_pass,
        "selected_probability": selected_probability,
        "exact_margin": exact_margin,
        "timeout_reached": timeout_reached,
    }


def _failure_result(
    *,
    elapsed_seconds: float,
    failure_reason: str,
    input_format: str | None = None,
    output_format: str | None = None,
    source: str | None = None,
    searched_nodes: int = 0,
    timeout_reached: bool = False,
) -> dict[str, object]:
    return {
        "success": False,
        "best_move": None,
        "value": None,
        "policy": None,
        "pv": [],
        "searched_nodes": searched_nodes,
        "elapsed_seconds": elapsed_seconds,
        "failure_reason": failure_reason,
        "input_format": input_format,
        "output_format": output_format,
        "source": source,
        "forced_pass": False,
        "selected_probability": None,
        "exact_margin": None,
        "timeout_reached": timeout_reached,
    }


def _exact_result_to_selection_result(
    exact_result: dict[str, object],
    *,
    elapsed_seconds: float,
) -> dict[str, object]:
    exact_margin = cast(int, exact_result["exact_margin"])
    return _success_result(
        best_move=cast(int | None, exact_result["best_move"]),
        elapsed_seconds=elapsed_seconds,
        input_format=None,
        output_format="exact",
        source="exact",
        pv=cast(list[int], exact_result["pv"]),
        searched_nodes=cast(int, exact_result["searched_nodes"]),
        value=float(exact_margin) / 64.0,
        exact_margin=exact_margin,
        timeout_reached=False,
    )


def _is_model_fallback_candidate(result: dict[str, object]) -> bool:
    return bool(result["success"]) and result["best_move"] is not None


def _with_elapsed(
    result: dict[str, object],
    *,
    start_time: float,
    timeout_reached: bool | None = None,
) -> dict[str, object]:
    updated = dict(result)
    updated["elapsed_seconds"] = time.perf_counter() - start_time
    if timeout_reached is not None:
        updated["timeout_reached"] = timeout_reached
    return updated


def _run_model_selection_path(
    *,
    board: Board,
    model: object,
    depth: int,
    deadline: float,
    policy_mode: str,
    device: str,
    torch_module: object,
    start_time: float,
) -> dict[str, object]:
    with getattr(torch_module, "no_grad")():
        input_format, output_format, root_output = _detect_model_io(
            model, board, torch_module, device
        )

        if output_format == "policy":
            raw_policy = cast(np.ndarray, root_output)
            legal_moves = board.legal_moves_list()
            distribution = _policy_distribution_for_board(raw_policy, legal_moves)
            legal_probs = distribution[np.asarray(legal_moves, dtype=np.intp)]
            if policy_mode == "best":
                chosen_idx = int(np.argmax(legal_probs))
            else:
                rng = np.random.default_rng()
                chosen_idx = int(rng.choice(len(legal_moves), p=legal_probs))
            selected_move = legal_moves[chosen_idx]
            return _success_result(
                best_move=selected_move,
                elapsed_seconds=time.perf_counter() - start_time,
                input_format=input_format,
                output_format="policy",
                source="policy",
                pv=[selected_move],
                policy=distribution,
                selected_probability=float(legal_probs[chosen_idx]),
                timeout_reached=time.perf_counter() >= deadline,
            )

        def evaluate_position(current_board: Board) -> float:
            _, value_output = _run_model_once(
                model, current_board, input_format, torch_module, device
            )
            return float(cast(np.float32, value_output))

        searched_nodes = [0]
        legal_moves = board.legal_moves_list()
        best_move: int | None = None
        best_value = -float("inf")
        best_pv: list[int] = []
        timeout_reached = False

        for move in legal_moves:
            if time.perf_counter() >= deadline:
                timeout_reached = True
                break
            try:
                child_value, child_pv = _value_search_negamax(
                    board.apply_move(move),
                    depth - 1,
                    deadline,
                    evaluate_position,
                    searched_nodes,
                    -float("inf"),
                    float("inf"),
                )
            except _ModelSearchTimeout:
                timeout_reached = True
                break
            score = -child_value
            if score > best_value:
                best_value = score
                best_move = move
                best_pv = [move, *child_pv]

        elapsed = time.perf_counter() - start_time
        if best_move is None:
            return _failure_result(
                elapsed_seconds=elapsed,
                failure_reason="timeout" if timeout_reached else "no_legal_moves",
                input_format=input_format,
                output_format="value",
                source="value_search",
                searched_nodes=searched_nodes[0],
                timeout_reached=timeout_reached,
            )
        return _success_result(
            best_move=best_move,
            elapsed_seconds=elapsed,
            input_format=input_format,
            output_format="value",
            source="value_search",
            pv=best_pv,
            searched_nodes=searched_nodes[0],
            value=float(best_value),
            timeout_reached=timeout_reached,
        )


def select_move_with_model(
    board_or_record: object,
    model: object,
    depth: int = 1,
    timeout_seconds: float = 1.0,
    *,
    policy_mode: str = "best",
    device: str = "cpu",
    exact_from_empty_threshold: int | None = 16,
    exact_timeout_seconds: float | None = None,
) -> dict[str, object]:
    """PyTorch model を使って着手を選びます。

    Args:
        board_or_record: `Board` または `RecordedBoard`。
        model: PyTorch `nn.Module`。
        depth: value 出力時の探索深さ。既定値は `1`。
        timeout_seconds: 探索全体の制限時間。
        policy_mode: `\"best\"` または `\"sample\"`。
        device: 推論に使う device。既定値は `\"cpu\"`。
        exact_from_empty_threshold:
            空き数がこの値以下なら exact 探索を優先します。
            `None` の場合は exact へ切り替えません。
        exact_timeout_seconds:
            exact 探索に使う制限時間。`None` の場合は `timeout_seconds` を使います。
            `exact_from_empty_threshold` 以下では exact と model を並列に開始し、
            exact がこの制限内で成功すれば exact を返します。

    Returns:
        次のキーを持つ dict。

        - `success`: 着手を決定できたか
        - `best_move`: 選択された手。強制パス時は `None`
        - `value`: value 系評価。policy 出力時は `None`
        - `policy`: shape `(64,)` の確率分布。value 出力時は `None`
        - `pv`: value / exact 経路で得られた読み筋
        - `searched_nodes`: value / exact 経路の探索ノード数
        - `elapsed_seconds`: 実行時間
        - `failure_reason`: 失敗理由。成功時は `None`
        - `input_format`: `\"cnn\"` / `\"flat\"` / `None`
        - `output_format`: `\"policy\"` / `\"value\"` / `\"exact\"` / `None`
        - `source`: `\"policy\"` / `\"value_search\"` / `\"exact\"` / `\"forced_pass\"`
        - `forced_pass`: 強制パス局面なら `True`
        - `selected_probability`: policy 経路で選ばれた手の確率
        - `exact_margin`: exact 経路の現在手番視点石差
        - `timeout_reached`: value 探索の部分結果、または exact timeout/failure 後の model fallback なら `True`
    """
    board = _board_from_board_or_record(board_or_record)
    validated_depth = _validate_positive_depth(depth)
    timeout_value = _validate_timeout_seconds(timeout_seconds, "timeout_seconds")
    validated_policy_mode = _validate_policy_mode(policy_mode)
    validated_device = _validate_device(device)
    exact_threshold = _validate_optional_exact_threshold(exact_from_empty_threshold)
    exact_timeout_value = (
        timeout_value
        if exact_timeout_seconds is None
        else _validate_timeout_seconds(exact_timeout_seconds, "exact_timeout_seconds")
    )

    start = time.perf_counter()
    torch_module = _import_torch()
    _validate_torch_model(model, torch_module)

    status = board.board_status()
    if status == "terminal":
        return _failure_result(
            elapsed_seconds=time.perf_counter() - start,
            failure_reason="terminal",
        )
    if status == "forced_pass":
        return _success_result(
            best_move=None,
            elapsed_seconds=time.perf_counter() - start,
            input_format=None,
            output_format="policy",
            source="forced_pass",
            forced_pass=True,
        )
    if timeout_value == 0.0:
        return _failure_result(
            elapsed_seconds=time.perf_counter() - start,
            failure_reason="timeout",
            timeout_reached=True,
        )

    overall_deadline = start + timeout_value
    exact_deadline = start + min(timeout_value, exact_timeout_value)
    was_training = bool(getattr(model, "training", False))
    if hasattr(model, "eval"):
        cast(Any, model).eval()
    try:
        empty_count = disc_count(board)[2]
        if exact_threshold is not None and empty_count <= exact_threshold:
            model_result: dict[str, object] | None = None
            model_exception: BaseException | None = None
            exact_result: dict[str, object] | None = None
            exact_consumed = False
            executor = concurrent.futures.ThreadPoolExecutor(max_workers=2)
            exact_future: concurrent.futures.Future[dict[str, object]] | None = None
            model_future: concurrent.futures.Future[dict[str, object]] | None = None
            try:
                remaining_exact = max(0.0, exact_deadline - time.perf_counter())
                if remaining_exact > 0.0:
                    exact_future = executor.submit(search_best_move_exact, board, remaining_exact)
                else:
                    exact_consumed = True

                model_future = executor.submit(
                    _run_model_selection_path,
                    board=board,
                    model=model,
                    depth=validated_depth,
                    deadline=overall_deadline,
                    policy_mode=validated_policy_mode,
                    device=validated_device,
                    torch_module=torch_module,
                    start_time=start,
                )

                while True:
                    now = time.perf_counter()
                    if exact_future is not None and exact_future.done():
                        exact_result = exact_future.result()
                        exact_future = None
                        exact_consumed = True
                        if cast(bool, exact_result["success"]):
                            return _exact_result_to_selection_result(
                                exact_result,
                                elapsed_seconds=time.perf_counter() - start,
                            )
                        if model_result is not None and _is_model_fallback_candidate(model_result):
                            return _with_elapsed(
                                model_result,
                                start_time=start,
                                timeout_reached=True,
                            )
                        if model_exception is not None:
                            raise model_exception

                    if model_future is not None and model_future.done():
                        try:
                            model_result = model_future.result()
                        except BaseException as exc:
                            model_exception = exc
                        model_future = None
                        if exact_consumed:
                            if model_result is not None and _is_model_fallback_candidate(
                                model_result
                            ):
                                return _with_elapsed(
                                    model_result,
                                    start_time=start,
                                    timeout_reached=True,
                                )
                            if model_exception is not None:
                                raise model_exception

                    now = time.perf_counter()
                    if not exact_consumed and now >= exact_deadline:
                        exact_consumed = True
                        exact_future = None
                        if model_result is not None and _is_model_fallback_candidate(model_result):
                            return _with_elapsed(
                                model_result,
                                start_time=start,
                                timeout_reached=True,
                            )
                        if model_exception is not None:
                            raise model_exception

                    if now >= overall_deadline:
                        if model_result is not None and _is_model_fallback_candidate(model_result):
                            return _with_elapsed(
                                model_result,
                                start_time=start,
                                timeout_reached=True,
                            )
                        if model_exception is not None and exact_consumed:
                            raise model_exception
                        return _failure_result(
                            elapsed_seconds=time.perf_counter() - start,
                            failure_reason="timeout",
                            timeout_reached=True,
                        )

                    pending: list[concurrent.futures.Future[dict[str, object]]] = []
                    if exact_future is not None:
                        pending.append(exact_future)
                    if model_future is not None:
                        pending.append(model_future)
                    if not pending:
                        if model_result is not None and _is_model_fallback_candidate(model_result):
                            return _with_elapsed(
                                model_result,
                                start_time=start,
                                timeout_reached=True,
                            )
                        if model_exception is not None:
                            raise model_exception
                        return _failure_result(
                            elapsed_seconds=time.perf_counter() - start,
                            failure_reason="timeout",
                            timeout_reached=True,
                        )

                    next_deadline = overall_deadline
                    if not exact_consumed:
                        next_deadline = min(next_deadline, exact_deadline)
                    wait_timeout = max(0.0, next_deadline - now)
                    concurrent.futures.wait(
                        pending,
                        timeout=wait_timeout,
                        return_when=concurrent.futures.FIRST_COMPLETED,
                    )
            finally:
                executor.shutdown(wait=False, cancel_futures=True)

        return _run_model_selection_path(
            board=board,
            model=model,
            depth=validated_depth,
            deadline=overall_deadline,
            policy_mode=validated_policy_mode,
            device=validated_device,
            torch_module=torch_module,
            start_time=start,
        )
    finally:
        if hasattr(model, "train"):
            cast(Any, model).train(was_training)


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
    """1 つの trace から supervised example 列を生成します。"""
    return [
        _example_from_parts(parts)
        for parts in _supervised_examples_from_trace_parts(*_trace_to_core_parts(trace))
    ]


def supervised_examples_from_traces(traces: list[dict]) -> list[dict[str, object]]:
    """複数 trace から supervised example 列を連結して生成します。"""
    if type(traces) is not list:
        raise ValueError("traces must be a list[dict]")
    return [
        _example_from_parts(parts)
        for parts in _supervised_examples_from_traces_parts(
            [_trace_to_core_parts(trace) for trace in traces]
        )
    ]


def packed_supervised_examples_from_trace(trace: dict) -> list[dict[str, object]]:
    """1 つの trace から packed supervised example 列を生成します。"""
    return [
        _packed_example_from_parts(parts)
        for parts in _packed_supervised_examples_from_trace_parts(*_trace_to_core_parts(trace))
    ]


def packed_supervised_examples_from_traces(traces: list[dict]) -> list[dict[str, object]]:
    """複数 trace から packed supervised example 列を連結して生成します。"""
    if type(traces) is not list:
        raise ValueError("traces must be a list[dict]")
    return [
        _packed_example_from_parts(parts)
        for parts in _packed_supervised_examples_from_traces_parts(
            [_trace_to_core_parts(trace) for trace in traces]
        )
    ]


def prepare_planes_learning_batch(examples: list[dict], config: dict) -> dict[str, np.ndarray]:
    """packed supervised example 列から planes 学習 batch を作ります。"""
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
    """packed supervised example 列から flat 学習 batch を作ります。"""
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
    """盤面と history から planes feature を生成します。"""
    if type(history) is not list:
        raise ValueError("history must be a list[Board]")

    return _encode_planes_parts(board, history, *_validate_feature_config(config))


def encode_planes_batch(
    boards: list[Board], histories: list[list[Board]], config: dict
) -> np.ndarray:
    """複数盤面と history 列から planes feature batch を生成します。"""
    if type(boards) is not list:
        raise ValueError("boards must be a list[Board]")
    if type(histories) is not list:
        raise ValueError("histories must be a list[list[Board]]")
    if len(boards) != len(histories):
        raise ValueError("boards and histories must have the same length")

    return _encode_planes_batch_parts(boards, histories, *_validate_feature_config(config))


def encode_flat_features(board: Board, history: list[Board], config: dict) -> np.ndarray:
    """盤面と history から flat feature を生成します。"""
    if type(history) is not list:
        raise ValueError("history must be a list[Board]")

    return _encode_flat_features_parts(board, history, *_validate_feature_config(config))


def encode_flat_features_batch(
    boards: list[Board], histories: list[list[Board]], config: dict
) -> np.ndarray:
    """複数盤面と history 列から flat feature batch を生成します。"""
    if type(boards) is not list:
        raise ValueError("boards must be a list[Board]")
    if type(histories) is not list:
        raise ValueError("histories must be a list[list[Board]]")
    if len(boards) != len(histories):
        raise ValueError("boards and histories must have the same length")

    return _encode_flat_features_batch_parts(boards, histories, *_validate_feature_config(config))


def main() -> None:
    print("hello veloversi")
