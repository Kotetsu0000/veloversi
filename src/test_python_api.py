from pathlib import Path
from types import SimpleNamespace
import time
from typing import cast

import numpy as np
import pytest
import veloversi
import veloversi._core as core

from veloversi import (
    Board,
    RecordedBoard,
    all_symmetries,
    apply_forced_pass,
    apply_move,
    board_from_bits,
    board_status,
    disc_count,
    encode_flat_features,
    encode_flat_features_batch,
    encode_planes,
    encode_planes_batch,
    final_margin_from_black,
    game_result,
    generate_legal_moves,
    initial_board,
    is_legal_move,
    legal_moves_list,
    load_game_records,
    open_game_record_dataset,
    pack_board,
    packed_supervised_examples_from_trace,
    packed_supervised_examples_from_traces,
    play_random_game,
    prepare_cnn_model_input,
    prepare_cnn_model_input_batch,
    random_start_board,
    record_move,
    record_pass,
    search_best_move_exact,
    start_game_recording,
    finish_game_recording,
    append_game_record,
    current_board,
    export_model,
    prepare_flat_learning_batch,
    prepare_flat_model_input,
    prepare_flat_model_input_batch,
    prepare_nnue_model_input,
    prepare_planes_learning_batch,
    sample_reachable_positions,
    select_move_with_model,
    load_model,
    supervised_examples_from_trace,
    supervised_examples_from_traces,
    transform_board,
    transform_square,
    unpack_board,
    validate_board,
)


class _FakeTensor:
    def __init__(self, array: object) -> None:
        self._array = np.asarray(array, dtype=np.float32)
        self.device = "cpu"

    def to(self, device: str) -> "_FakeTensor":
        self.device = device
        return self

    def detach(self) -> "_FakeTensor":
        return self

    def cpu(self) -> "_FakeTensor":
        return self

    def numpy(self) -> np.ndarray:
        return np.asarray(self._array, dtype=np.float32)


class _FakeNoGrad:
    def __enter__(self) -> None:
        return None

    def __exit__(self, exc_type: object, exc: object, tb: object) -> bool:
        return False


class _FakeModule:
    def __init__(self) -> None:
        self.training = True

    def eval(self) -> "_FakeModule":
        self.training = False
        return self

    def train(self, mode: bool = True) -> "_FakeModule":
        self.training = mode
        return self

    def __call__(self, tensor: _FakeTensor) -> _FakeTensor:
        return self.forward(tensor)

    def forward(self, tensor: _FakeTensor) -> _FakeTensor:
        raise NotImplementedError


class _CnnPolicyModel(_FakeModule):
    def __init__(self) -> None:
        super().__init__()
        self.calls = 0

    def forward(self, tensor: _FakeTensor) -> _FakeTensor:
        self.calls += 1
        array = tensor.numpy()
        if array.shape != (1, 3, 8, 8):
            raise ValueError(f"unexpected cnn shape: {array.shape}")
        legal = array[0, 2].reshape(64)
        weights = np.arange(64, dtype=np.float32)
        logits = legal * weights
        return _FakeTensor(logits.reshape(1, 64))


class _FlatValueModel(_FakeModule):
    def __init__(self) -> None:
        super().__init__()
        self.calls = 0

    def forward(self, tensor: _FakeTensor) -> _FakeTensor:
        self.calls += 1
        array = tensor.numpy()
        if array.shape != (1, 192):
            raise ValueError(f"unexpected flat shape: {array.shape}")
        opp = array[0, 64:128]
        weights = np.arange(64, dtype=np.float32)
        score = -float(np.dot(opp, weights) / max(np.sum(opp), 1.0) / 63.0)
        return _FakeTensor(np.asarray([score], dtype=np.float32))


class _AmbiguousModel(_FakeModule):
    def forward(self, tensor: _FakeTensor) -> _FakeTensor:
        return _FakeTensor(np.asarray([0.0], dtype=np.float32))


class _FakeRustModel:
    def __init__(self) -> None:
        self.calls = 0
        self.accumulator_dim = 32
        self.hidden_dim = 16

    def evaluate_board(self, board: Board) -> float:
        self.calls += 1
        features = board.prepare_flat_model_input()[0]
        opp = features[64:128]
        weights = np.arange(64, dtype=np.float32)
        return -float(np.dot(opp, weights) / max(np.sum(opp), 1.0) / 63.0)


def _fake_torch_module() -> object:
    return SimpleNamespace(
        nn=SimpleNamespace(Module=_FakeModule),
        from_numpy=lambda array: _FakeTensor(array),
        no_grad=lambda: _FakeNoGrad(),
    )


def test_board_round_trip_and_validate() -> None:
    board = initial_board()

    assert isinstance(board, Board)
    assert board.to_bits() == (board.black_bits, board.white_bits, board.side_to_move)
    assert board_from_bits(*board.to_bits()).to_bits() == board.to_bits()
    validate_board(board)

    core_board = core.Board(*board.to_bits())
    assert core_board.black_bits == board.black_bits
    assert core_board.white_bits == board.white_bits
    assert core_board.side_to_move == board.side_to_move
    assert core_board.to_bits() == board.to_bits()

    packed = pack_board(board)
    assert packed == board.to_bits()
    assert unpack_board(packed).to_bits() == board.to_bits()
    assert core.pack_board(board) == board.to_bits()
    assert core._unpack_board_parts(*packed).to_bits() == board.to_bits()


def test_generate_legal_moves_helpers_match_initial_position() -> None:
    board = initial_board()

    assert generate_legal_moves(board) == 0x0000_1020_0408_0000
    assert legal_moves_list(board) == [19, 26, 37, 44]
    assert is_legal_move(board, 19)
    assert not is_legal_move(board, 0)
    assert core.generate_legal_moves(board) == generate_legal_moves(board)
    assert core.legal_moves_list(board) == legal_moves_list(board)
    assert core.is_legal_move(board, 19)
    assert not core.is_legal_move(board, 0)


def test_game_helpers_match_expected_values() -> None:
    board = initial_board()

    assert board_status(board) == "ongoing"
    assert disc_count(board) == (2, 2, 60)
    assert game_result(board) == "draw"
    assert final_margin_from_black(board) == 0
    assert core.board_status(board) == "ongoing"
    assert core.disc_count(board) == (2, 2, 60)
    assert core.game_result(board) == "draw"
    assert core.final_margin_from_black(board) == 0


def test_apply_move_and_forced_pass_behave_as_expected() -> None:
    board = initial_board()
    next_board = apply_move(board, 19)

    assert next_board.to_bits() == (0x0000_0008_1808_0000, 0x0000_0010_0000_0000, "white")
    assert board_status(next_board) == "ongoing"

    forced_pass_board = board_from_bits(0xFFFF_FFFF_FFFF_FF7E, 0x0000_0000_0000_0080, "black")
    assert board_status(forced_pass_board) == "forced_pass"
    assert apply_forced_pass(forced_pass_board).to_bits() == (
        0xFFFF_FFFF_FFFF_FF7E,
        0x0000_0000_0000_0080,
        "white",
    )
    assert core.apply_move(board, 19).to_bits() == next_board.to_bits()
    assert core.apply_forced_pass(forced_pass_board).to_bits() == (
        0xFFFF_FFFF_FFFF_FF7E,
        0x0000_0000_0000_0080,
        "white",
    )


def test_board_method_style_api_matches_module_level_helpers() -> None:
    board = initial_board()

    moved = board.apply_move(19)

    assert moved.to_bits() == apply_move(board, 19).to_bits()
    assert board.legal_moves_list() == legal_moves_list(board)
    assert board.is_legal_move(19) is True
    assert board.board_status() == board_status(board)
    assert board.disc_count() == disc_count(board)
    assert board.game_result() == game_result(board)
    assert board.final_margin_from_black() == final_margin_from_black(board)


def test_board_extended_method_style_api_matches_module_level_helpers() -> None:
    board = initial_board()
    history = [board.apply_move(19)]
    config = {
        "history_len": 1,
        "include_legal_mask": True,
        "include_phase_plane": False,
        "include_turn_plane": False,
        "perspective": "side_to_move",
    }

    assert board.transform("rot90").to_bits() == transform_board(board, "rot90").to_bits()
    assert np.array_equal(
        board.encode_planes(history, config), encode_planes(board, history, config)
    )
    assert np.array_equal(
        board.encode_flat_features(history, config),
        encode_flat_features(board, history, config),
    )
    assert np.array_equal(board.prepare_cnn_model_input(), prepare_cnn_model_input(board))
    assert np.array_equal(board.prepare_flat_model_input(), prepare_flat_model_input(board))


def test_prepare_nnue_model_input_matches_board_record_and_dataset(tmp_path: Path) -> None:
    board = board_from_bits(0xFFFF_FFFF_FFFF_FF7E, 0x0000_0000_0000_0080, "white")
    record = start_game_recording(board)
    path = tmp_path / "nnue-games.jsonl"

    finished = finish_game_recording(record.apply_move(0))
    append_game_record(str(path), finished)
    dataset = open_game_record_dataset(path)

    expected = prepare_nnue_model_input(board)
    assert expected.shape == (1, 67)
    assert expected.dtype == np.int32
    assert np.array_equal(board.prepare_nnue_model_input(), expected)
    assert np.array_equal(record.prepare_nnue_model_input(), expected)
    assert np.array_equal(dataset.get_nnue_input(0), expected[0])


def test_step10_python_public_surface_matches_policy() -> None:
    assert not hasattr(veloversi, "apply_move_unchecked")
    assert not hasattr(veloversi, "generate_legal_moves_bits")
    assert not hasattr(veloversi, "apply_move_bits")
    assert not hasattr(veloversi._core, "apply_move_unchecked")
    assert not hasattr(veloversi._core, "generate_legal_moves_bits")
    assert not hasattr(veloversi._core, "apply_move_bits")

    with pytest.raises(ValueError):
        board_from_bits(1, 1, "black")

    with pytest.raises(ValueError):
        core.Board(0, 0, "bad_color")


def test_symmetry_api_fixed_order_and_square_mapping() -> None:
    expected = [
        "identity",
        "rot90",
        "rot180",
        "rot270",
        "flip_horizontal",
        "flip_vertical",
        "flip_diag",
        "flip_anti_diag",
    ]
    assert all_symmetries() == expected
    assert core.all_symmetries() == expected

    assert transform_square(19, "identity") == 19
    assert transform_square(19, "rot90") == 29
    assert transform_square(19, "rot180") == 44
    assert transform_square(19, "rot270") == 34
    assert transform_square(19, "flip_horizontal") == 20
    assert transform_square(19, "flip_vertical") == 43
    assert transform_square(19, "flip_diag") == 26
    assert transform_square(19, "flip_anti_diag") == 37
    for sym in expected:
        assert core.transform_square(19, sym) == transform_square(19, sym)


def test_transform_board_preserves_counts_and_transforms_legal_moves() -> None:
    board = initial_board()
    transformed = transform_board(board, "rot90")
    core_transformed = core.transform_board(board, "rot90")

    assert transformed.side_to_move == board.side_to_move
    assert disc_count(transformed) == disc_count(board)
    assert final_margin_from_black(transformed) == final_margin_from_black(board)
    assert core_transformed.to_bits() == transformed.to_bits()

    expected = sorted(transform_square(square, "rot90") for square in legal_moves_list(board))
    assert legal_moves_list(transformed) == expected


def test_unknown_symmetry_name_raises_value_error() -> None:
    with pytest.raises(ValueError):
        transform_square(19, "bad_symmetry")

    with pytest.raises(ValueError):
        transform_board(initial_board(), "bad_symmetry")

    with pytest.raises(ValueError):
        core.transform_square(19, "bad_symmetry")

    with pytest.raises(ValueError):
        core.transform_board(initial_board(), "bad_symmetry")


def test_transform_square_rejects_out_of_range_square() -> None:
    with pytest.raises(ValueError):
        core.transform_square(64, "identity")


def test_pack_unpack_round_trip_and_fixed_initial_tuple() -> None:
    board = initial_board()

    assert pack_board(board) == (0x0000_0008_1000_0000, 0x0000_0010_0800_0000, "black")
    assert unpack_board(pack_board(board)).to_bits() == board.to_bits()

    moved = apply_move(board, 19)
    packed = pack_board(moved)
    assert unpack_board(packed).to_bits() == moved.to_bits()
    assert core.pack_board(moved) == packed
    assert core._unpack_board_parts(*packed).to_bits() == moved.to_bits()


def test_play_random_game_is_reproducible_and_returns_trace_dict() -> None:
    lhs = play_random_game(123, {"max_plies": 10})
    rhs = play_random_game(123, {"max_plies": 10})

    assert lhs["moves"] == rhs["moves"]
    assert lhs["final_result"] == rhs["final_result"]
    assert lhs["final_margin_from_black"] == rhs["final_margin_from_black"]
    assert lhs["plies_played"] == 10
    assert lhs["reached_terminal"] is False
    assert len(lhs["boards"]) == 11
    assert all(isinstance(board, Board) for board in lhs["boards"])


def test_play_random_game_trace_contains_only_legal_transitions() -> None:
    trace = play_random_game(9, {"max_plies": 16})

    assert len(trace["boards"]) == len(trace["moves"]) + 1
    for idx, mv in enumerate(trace["moves"]):
        board = trace["boards"][idx]
        next_board = trace["boards"][idx + 1]
        if mv is None:
            assert board_status(board) == "forced_pass"
            assert apply_forced_pass(board).to_bits() == next_board.to_bits()
        else:
            assert is_legal_move(board, mv)
            assert apply_move(board, mv).to_bits() == next_board.to_bits()


def test_play_random_game_final_label_matches_unbounded_rollout() -> None:
    full = play_random_game(77, {"max_plies": None})
    truncated = play_random_game(77, {"max_plies": 8})

    assert full["final_result"] == truncated["final_result"]
    assert full["final_margin_from_black"] == truncated["final_margin_from_black"]


def test_sample_reachable_positions_returns_boards() -> None:
    positions = sample_reachable_positions(5, {"num_positions": 6, "min_plies": 4, "max_plies": 8})

    assert len(positions) == 6
    assert all(isinstance(board, Board) for board in positions)
    for board in positions:
        black, white, _empty = disc_count(board)
        plies = black + white - 4
        assert 4 <= plies <= 8


def test_random_play_public_api_rejects_invalid_config() -> None:
    with pytest.raises(ValueError, match="config"):
        play_random_game(1, [])  # type: ignore[arg-type]

    with pytest.raises(ValueError, match="max_plies"):
        play_random_game(1, {"max_plies": -1})

    with pytest.raises(ValueError, match="config"):
        sample_reachable_positions(1, [])  # type: ignore[arg-type]

    with pytest.raises(ValueError, match="num_positions"):
        sample_reachable_positions(1, {"num_positions": -1, "min_plies": 0, "max_plies": 1})

    with pytest.raises(ValueError, match="min_plies"):
        sample_reachable_positions(1, {"num_positions": 1, "min_plies": -1, "max_plies": 1})

    with pytest.raises(ValueError, match="min_plies must be less than or equal to max_plies"):
        sample_reachable_positions(1, {"num_positions": 1, "min_plies": 3, "max_plies": 1})


def test_supervised_examples_from_trace_returns_prefix_examples() -> None:
    trace = play_random_game(11, {"max_plies": 6})
    examples = supervised_examples_from_trace(trace)

    assert len(examples) == len(trace["boards"])
    for ply, example in enumerate(examples):
        board = cast(Board, example["board"])
        assert isinstance(board, Board)
        assert board.to_bits() == trace["boards"][ply].to_bits()
        assert example["ply"] == ply
        assert example["moves_until_here"] == trace["moves"][:ply]
        assert example["final_result"] == trace["final_result"]
        assert example["final_margin_from_black"] == trace["final_margin_from_black"]


def test_supervised_examples_from_traces_concatenates_examples() -> None:
    first = play_random_game(1, {"max_plies": 3})
    second = play_random_game(2, {"max_plies": 2})

    merged = supervised_examples_from_traces([first, second])
    separate = supervised_examples_from_trace(first) + supervised_examples_from_trace(second)

    normalized_merged = [
        {
            **example,
            "board": cast(Board, example["board"]).to_bits(),
        }
        for example in merged
    ]
    normalized_separate = [
        {
            **example,
            "board": cast(Board, example["board"]).to_bits(),
        }
        for example in separate
    ]

    assert normalized_merged == normalized_separate


def test_supervised_examples_public_api_rejects_invalid_trace() -> None:
    with pytest.raises(ValueError, match="trace"):
        supervised_examples_from_trace([])  # type: ignore[arg-type]

    with pytest.raises(ValueError, match="boards"):
        supervised_examples_from_trace(
            {
                "boards": [],
                "moves": [],
                "final_result": "draw",
                "final_margin_from_black": 0,
                "plies_played": 0,
                "reached_terminal": True,
            }
        )

    with pytest.raises(ValueError, match="traces"):
        supervised_examples_from_traces({})  # type: ignore[arg-type]


def test_packed_supervised_examples_from_trace_returns_policy_and_value_labels() -> None:
    trace = play_random_game(11, {"max_plies": 6})
    packed_examples = packed_supervised_examples_from_trace(trace)

    assert len(packed_examples) == len(trace["boards"])
    for ply, example in enumerate(packed_examples):
        assert example["board"] == pack_board(trace["boards"][ply])
        assert example["ply"] == ply
        assert example["moves_until_here"] == trace["moves"][:ply]
        assert example["final_result"] == trace["final_result"]
        assert example["final_margin_from_black"] == trace["final_margin_from_black"]

        if ply == len(trace["moves"]):
            assert example["policy_target_index"] == -1
            assert example["policy_target_square"] is None
            assert example["policy_target_is_pass"] is False
            assert example["has_policy_target"] is False
        elif trace["moves"][ply] is None:
            assert example["policy_target_index"] == 64
            assert example["policy_target_square"] is None
            assert example["policy_target_is_pass"] is True
            assert example["has_policy_target"] is True
        else:
            assert example["policy_target_index"] == trace["moves"][ply]
            assert example["policy_target_square"] == trace["moves"][ply]
            assert example["policy_target_is_pass"] is False
            assert example["has_policy_target"] is True


def test_packed_supervised_examples_from_traces_concatenates_examples() -> None:
    first = play_random_game(1, {"max_plies": 3})
    second = play_random_game(2, {"max_plies": 2})

    merged = packed_supervised_examples_from_traces([first, second])
    separate = packed_supervised_examples_from_trace(first) + packed_supervised_examples_from_trace(
        second
    )

    assert merged == separate


def test_prepare_planes_learning_batch_returns_expected_shapes() -> None:
    trace = play_random_game(7, {"max_plies": 4})
    examples = packed_supervised_examples_from_trace(trace)
    batch = prepare_planes_learning_batch(
        examples,
        {
            "history_len": 0,
            "include_legal_mask": False,
            "include_phase_plane": True,
            "include_turn_plane": True,
            "perspective": "side_to_move",
        },
    )

    assert batch["features"].shape[0] == len(examples)
    assert batch["features"].shape[2:] == (8, 8)
    assert batch["value_targets"].shape == (len(examples),)
    assert batch["policy_targets"].shape == (len(examples),)
    assert batch["legal_move_masks"].shape == (len(examples), 64)
    assert batch["features"].dtype == np.float32
    assert batch["value_targets"].dtype == np.float32
    assert batch["policy_targets"].dtype == np.int16
    assert batch["legal_move_masks"].dtype == np.float32


def test_prepare_flat_learning_batch_returns_expected_shapes_and_b_equals_one() -> None:
    trace = play_random_game(9, {"max_plies": 2})
    examples = packed_supervised_examples_from_trace(trace)[:1]
    batch = prepare_flat_learning_batch(
        examples,
        {
            "history_len": 0,
            "include_legal_mask": False,
            "include_phase_plane": True,
            "include_turn_plane": True,
            "perspective": "side_to_move",
        },
    )

    assert batch["features"].shape[0] == 1
    assert batch["value_targets"].shape == (1,)
    assert batch["policy_targets"].shape == (1,)
    assert batch["legal_move_masks"].shape == (1, 64)


def test_prepare_learning_batch_supports_nonzero_history_and_rejects_invalid_examples() -> None:
    trace = play_random_game(5, {"max_plies": 2})
    examples = packed_supervised_examples_from_trace(trace)

    batch = prepare_planes_learning_batch(
        examples,
        {
            "history_len": 1,
            "include_legal_mask": False,
            "include_phase_plane": True,
            "include_turn_plane": True,
            "perspective": "side_to_move",
        },
    )
    assert batch["features"].shape[0] == len(examples)

    bad_examples = [dict(examples[0], policy_target_index=65)]
    with pytest.raises(ValueError, match="policy_target_index"):
        prepare_flat_learning_batch(
            bad_examples,
            {
                "history_len": 0,
                "include_legal_mask": False,
                "include_phase_plane": True,
                "include_turn_plane": True,
                "perspective": "side_to_move",
            },
        )


def test_prepare_learning_batch_rejects_invalid_history_pass() -> None:
    trace = play_random_game(6, {"max_plies": 1})
    examples = packed_supervised_examples_from_trace(trace)
    bad_examples = [dict(examples[0], moves_until_here=[None])]

    with pytest.raises(ValueError, match="InvalidHistoryPass"):
        prepare_planes_learning_batch(
            bad_examples,
            {
                "history_len": 1,
                "include_legal_mask": False,
                "include_phase_plane": True,
                "include_turn_plane": True,
                "perspective": "side_to_move",
            },
        )


def test_random_start_board_is_reproducible() -> None:
    lhs = random_start_board(5, 123)
    rhs = random_start_board(5, 123)
    assert lhs.to_bits() == rhs.to_bits()


def test_start_game_recording_and_record_move_update_current_board() -> None:
    record = start_game_recording(initial_board())
    next_record = record_move(record, 19)

    assert isinstance(record, RecordedBoard)
    assert isinstance(next_record, RecordedBoard)
    assert current_board(record).to_bits() == initial_board().to_bits()
    assert current_board(next_record).to_bits() == apply_move(initial_board(), 19).to_bits()
    assert next_record.moves == [19]


def test_record_pass_updates_recording() -> None:
    board = board_from_bits(0xFFFF_FFFF_FFFF_FF7E, 0x0000_0000_0000_0080, "black")
    record = start_game_recording(board)
    next_record = record_pass(record)

    assert next_record.moves == [None]
    assert current_board(next_record).to_bits() == apply_forced_pass(board).to_bits()


def test_recorded_board_method_style_api_matches_module_level_helpers() -> None:
    record = start_game_recording(initial_board())
    moved = record.apply_move(19)

    assert isinstance(moved, RecordedBoard)
    assert moved.current_board.to_bits() == apply_move(initial_board(), 19).to_bits()
    assert moved.moves == [19]
    assert moved.legal_moves_list() == legal_moves_list(moved)
    assert moved.is_legal_move(moved.legal_moves_list()[0]) is True
    assert moved.board_status() == board_status(moved)
    assert moved.disc_count() == disc_count(moved)
    assert moved.game_result() == game_result(moved)
    assert moved.final_margin_from_black() == final_margin_from_black(moved)


def test_recorded_board_extended_methods_forward_to_current_board() -> None:
    record = start_game_recording(initial_board()).apply_move(19)
    current = record.current_board
    history = [initial_board()]
    config = {
        "history_len": 1,
        "include_legal_mask": True,
        "include_phase_plane": False,
        "include_turn_plane": False,
        "perspective": "side_to_move",
    }

    assert record.transform("rot180").to_bits() == current.transform("rot180").to_bits()
    assert np.array_equal(
        record.encode_planes(history, config), current.encode_planes(history, config)
    )
    assert np.array_equal(
        record.encode_flat_features(history, config),
        current.encode_flat_features(history, config),
    )
    assert np.array_equal(record.prepare_cnn_model_input(), current.prepare_cnn_model_input())
    assert np.array_equal(record.prepare_flat_model_input(), current.prepare_flat_model_input())


def test_search_best_move_exact_succeeds_on_small_endgame_for_board_and_record() -> None:
    board = board_from_bits(0xFFFF_FFFF_FFFF_FF7E, 0x0000_0000_0000_0080, "white")
    record = start_game_recording(board)

    result = search_best_move_exact(board, 1.0)
    board_method = board.search_best_move_exact(1.0)
    record_method = record.search_best_move_exact(1.0)
    configured = search_best_move_exact(
        board,
        1.0,
        worker_count=1,
        serial_fallback_empty_threshold=1,
        shared_tt_empty_threshold=1,
    )

    for candidate in (result, board_method, record_method, configured):
        assert candidate["success"] is True
        assert candidate["best_move"] == 0
        assert candidate["exact_margin"] == -48
        assert candidate["pv"] == [0]
        assert cast(int, candidate["searched_nodes"]) >= 1
        assert candidate["failure_reason"] is None
        assert cast(float, candidate["elapsed_seconds"]) >= 0.0


def test_search_best_move_exact_returns_timeout_failure() -> None:
    result = search_best_move_exact(initial_board(), 0.0)

    assert result["success"] is False
    assert result["best_move"] is None
    assert result["exact_margin"] is None
    assert result["pv"] == []
    assert result["failure_reason"] == "timeout"
    assert cast(float, result["elapsed_seconds"]) >= 0.0


def test_search_best_move_exact_rejects_invalid_timeout() -> None:
    with pytest.raises(ValueError, match="timeout_seconds"):
        search_best_move_exact(initial_board(), -0.1)

    with pytest.raises(ValueError, match="timeout_seconds"):
        search_best_move_exact(initial_board(), float("nan"))


def test_search_best_move_exact_rejects_invalid_parallel_settings() -> None:
    with pytest.raises(ValueError, match="worker_count"):
        search_best_move_exact(initial_board(), 1.0, worker_count=0)

    with pytest.raises(ValueError, match="serial_fallback_empty_threshold"):
        search_best_move_exact(initial_board(), 1.0, serial_fallback_empty_threshold=-1)

    with pytest.raises(ValueError, match="shared_tt_empty_threshold"):
        search_best_move_exact(initial_board(), 1.0, shared_tt_empty_threshold=256)

    with pytest.raises(ValueError, match="shared_tt_empty_threshold must be >="):
        search_best_move_exact(
            initial_board(),
            1.0,
            serial_fallback_empty_threshold=20,
            shared_tt_empty_threshold=18,
        )


def test_select_move_with_model_requires_torch(monkeypatch: pytest.MonkeyPatch) -> None:
    def _raise_missing_torch() -> object:
        raise RuntimeError("select_move_with_model を使うには PyTorch (`torch`) の導入が必要です")

    monkeypatch.setattr(veloversi, "_import_torch", _raise_missing_torch)

    with pytest.raises(RuntimeError, match="PyTorch"):
        select_move_with_model(initial_board(), object())


def test_export_model_requires_torch(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    def _raise_missing_torch() -> object:
        raise RuntimeError("select_move_with_model を使うには PyTorch (`torch`) の導入が必要です")

    monkeypatch.setattr(veloversi, "_import_torch", _raise_missing_torch)

    with pytest.raises(RuntimeError, match="export_model"):
        export_model(tmp_path / "weights.pth", tmp_path / "weights.vvm")


def test_veloversi_model_nnue_requires_torch() -> None:
    with pytest.raises(RuntimeError, match="veloversi.model"):
        veloversi.model.NNUE()


def test_load_model_uses_core_loader(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    sentinel = object()
    calls: list[str] = []

    def _fake_loader(path: str) -> object:
        calls.append(path)
        return sentinel

    monkeypatch.setattr(veloversi, "_load_rust_value_model", _fake_loader)

    result = load_model(tmp_path / "weights.vvm")

    assert result is sentinel
    assert calls == [str(tmp_path / "weights.vvm")]


def test_select_move_with_rust_value_model_does_not_require_torch(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def _unexpected_torch() -> object:
        raise AssertionError("torch import should not run for RustValueModel")

    monkeypatch.setattr(veloversi, "_import_torch", _unexpected_torch)
    monkeypatch.setattr(veloversi, "RustValueModel", _FakeRustModel)
    model = _FakeRustModel()

    result = select_move_with_model(initial_board(), model, depth=1)

    assert result["success"] is True
    assert result["input_format"] == "nnue"
    assert result["output_format"] == "value"
    assert result["source"] == "value_search"
    assert result["best_move"] == 44
    assert model.calls >= 4


def test_select_move_with_model_policy_cnn_supports_board_and_record(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(veloversi, "_import_torch", _fake_torch_module)
    model = _CnnPolicyModel()
    board = initial_board()
    record = start_game_recording(board)

    board_result = board.select_move_with_model(model, policy_mode="best")
    record_result = record.select_move_with_model(model, policy_mode="best")

    for candidate in (board_result, record_result):
        assert candidate["success"] is True
        assert candidate["best_move"] == 44
        assert candidate["input_format"] == "cnn"
        assert candidate["output_format"] == "policy"
        assert candidate["source"] == "policy"
        assert candidate["forced_pass"] is False
        assert candidate["timeout_reached"] is False
        assert cast(np.ndarray, candidate["policy"]).shape == (64,)
        assert cast(float, candidate["selected_probability"]) > 0.0


def test_select_move_with_model_policy_sample_uses_probability_distribution(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(veloversi, "_import_torch", _fake_torch_module)
    monkeypatch.setattr(
        np.random,
        "default_rng",
        lambda: SimpleNamespace(choice=lambda length, p: 0),
    )
    model = _CnnPolicyModel()

    result = select_move_with_model(initial_board(), model, policy_mode="sample")

    assert result["success"] is True
    assert result["best_move"] == 19
    assert result["output_format"] == "policy"
    assert result["source"] == "policy"


def test_select_move_with_model_value_flat_search_and_partial_timeout(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(veloversi, "_import_torch", _fake_torch_module)
    model = _FlatValueModel()

    result = select_move_with_model(initial_board(), model, depth=1)

    assert result["success"] is True
    assert result["best_move"] == 44
    assert result["input_format"] == "flat"
    assert result["output_format"] == "value"
    assert result["source"] == "value_search"
    assert result["timeout_reached"] is False
    assert cast(float, result["value"]) > 0.0
    assert cast(int, result["searched_nodes"]) >= 4
    assert model.training is True

    ticks = iter(np.arange(0.0, 4.0, 0.2).tolist())
    monkeypatch.setattr("veloversi.time.perf_counter", lambda: float(next(ticks)))
    timeout_result = select_move_with_model(initial_board(), model, depth=1, timeout_seconds=1.0)

    assert timeout_result["success"] is True
    assert timeout_result["timeout_reached"] is True
    assert timeout_result["best_move"] in {19, 26}
    assert timeout_result["source"] == "value_search"


def test_select_move_with_model_does_not_try_exact_above_threshold_by_default(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(veloversi, "_import_torch", _fake_torch_module)

    def _unexpected_exact(*args: object, **kwargs: object) -> dict[str, object]:
        raise AssertionError("exact should not run above threshold by default")

    monkeypatch.setattr(veloversi, "search_best_move_exact", _unexpected_exact)
    model = _CnnPolicyModel()

    result = select_move_with_model(
        initial_board(),
        model,
        policy_mode="best",
        exact_from_empty_threshold=16,
    )

    assert result["success"] is True
    assert result["source"] == "policy"
    assert result["best_move"] == 44


def test_select_move_with_model_uses_exact_fallback_and_prefers_exact_result(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(veloversi, "_import_torch", _fake_torch_module)
    board = board_from_bits(0xFFFF_FFFF_FFFF_FF7E, 0x0000_0000_0000_0080, "white")
    model = _CnnPolicyModel()

    result = select_move_with_model(
        board,
        model,
        exact_from_empty_threshold=10,
    )

    assert result["success"] is True
    assert result["best_move"] == 0
    assert result["source"] == "exact"
    assert result["exact_margin"] == -48
    assert result["value"] == pytest.approx(-48.0 / 64.0)
    assert model.calls >= 0


def test_select_move_with_model_handles_forced_pass_without_model_call(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(veloversi, "_import_torch", _fake_torch_module)
    board = board_from_bits(0xFFFF_FFFF_FFFF_FF7E, 0x0000_0000_0000_0080, "black")
    model = _CnnPolicyModel()

    result = select_move_with_model(board, model)

    assert result["success"] is True
    assert result["best_move"] is None
    assert result["forced_pass"] is True
    assert result["source"] == "forced_pass"
    assert model.calls == 0


def test_select_move_with_model_rejects_ambiguous_input_format(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(veloversi, "_import_torch", _fake_torch_module)

    with pytest.raises(ValueError, match="ambiguous"):
        select_move_with_model(initial_board(), _AmbiguousModel())


def test_select_move_with_model_prefers_exact_result_when_concurrent(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(veloversi, "_import_torch", _fake_torch_module)

    def _exact_success(
        board_or_record: object,
        timeout_seconds: float = 1.0,
        *,
        worker_count: int | None = None,
        serial_fallback_empty_threshold: int = 18,
        shared_tt_empty_threshold: int = 20,
    ) -> dict[str, object]:
        time.sleep(0.02)
        return {
            "success": True,
            "best_move": 19,
            "exact_margin": 12,
            "pv": [19, 26],
            "searched_nodes": 321,
            "elapsed_seconds": timeout_seconds,
            "failure_reason": None,
        }

    monkeypatch.setattr(veloversi, "search_best_move_exact", _exact_success)
    model = _CnnPolicyModel()

    result = select_move_with_model(
        initial_board(),
        model,
        policy_mode="best",
        exact_from_empty_threshold=64,
        timeout_seconds=0.5,
    )

    assert result["success"] is True
    assert result["best_move"] == 19
    assert result["source"] == "exact"
    assert result["output_format"] == "exact"
    assert result["timeout_reached"] is False
    assert result["exact_margin"] == 12


def test_select_move_with_model_returns_exact_failure_below_threshold_without_model_fallback(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(veloversi, "_import_torch", _fake_torch_module)

    def _slow_exact_failure(
        board_or_record: object,
        timeout_seconds: float = 1.0,
        *,
        worker_count: int | None = None,
        serial_fallback_empty_threshold: int = 18,
        shared_tt_empty_threshold: int = 20,
    ) -> dict[str, object]:
        time.sleep(0.2)
        return {
            "success": False,
            "best_move": None,
            "exact_margin": None,
            "pv": [],
            "searched_nodes": 999,
            "elapsed_seconds": timeout_seconds,
            "failure_reason": "synthetic_failure",
        }

    monkeypatch.setattr(veloversi, "search_best_move_exact", _slow_exact_failure)
    model = _CnnPolicyModel()
    started = time.perf_counter()

    result = select_move_with_model(
        initial_board(),
        model,
        policy_mode="best",
        exact_from_empty_threshold=64,
        timeout_seconds=0.5,
    )
    elapsed = time.perf_counter() - started

    assert result["success"] is False
    assert result["source"] == "exact"
    assert result["best_move"] is None
    assert result["failure_reason"] == "synthetic_failure"
    assert result["timeout_reached"] is False
    assert model.calls == 0
    assert 0.18 <= elapsed < 0.5


def test_select_move_with_model_always_try_exact_returns_model_when_model_finishes_first(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(veloversi, "_import_torch", _fake_torch_module)

    def _slow_exact_success(
        board_or_record: object,
        timeout_seconds: float = 1.0,
        *,
        worker_count: int | None = None,
        serial_fallback_empty_threshold: int = 18,
        shared_tt_empty_threshold: int = 20,
    ) -> dict[str, object]:
        time.sleep(0.2)
        return {
            "success": True,
            "best_move": 19,
            "exact_margin": 12,
            "pv": [19, 26],
            "searched_nodes": 321,
            "elapsed_seconds": timeout_seconds,
            "failure_reason": None,
        }

    def _fast_model_result(**kwargs: object) -> dict[str, object]:
        time.sleep(0.02)
        return {
            "success": True,
            "best_move": 44,
            "value": None,
            "policy": np.zeros(64, dtype=np.float32),
            "pv": [44],
            "searched_nodes": 0,
            "elapsed_seconds": 0.02,
            "failure_reason": None,
            "input_format": "cnn",
            "output_format": "policy",
            "source": "policy",
            "forced_pass": False,
            "selected_probability": 1.0,
            "exact_margin": None,
            "timeout_reached": False,
        }

    monkeypatch.setattr(veloversi, "search_best_move_exact", _slow_exact_success)
    monkeypatch.setattr(veloversi, "_run_model_selection_path", _fast_model_result)
    model = _CnnPolicyModel()
    started = time.perf_counter()

    result = select_move_with_model(
        initial_board(),
        model,
        policy_mode="best",
        exact_from_empty_threshold=16,
        always_try_exact=True,
        timeout_seconds=0.5,
    )
    elapsed = time.perf_counter() - started

    assert result["success"] is True
    assert result["source"] == "policy"
    assert result["best_move"] == 44
    assert result["timeout_reached"] is False
    assert elapsed < 0.2


def test_select_move_with_model_always_try_exact_uses_exact_only_below_threshold(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(veloversi, "_import_torch", _fake_torch_module)

    def _exact_success(
        board_or_record: object,
        timeout_seconds: float = 1.0,
        *,
        worker_count: int | None = None,
        serial_fallback_empty_threshold: int = 18,
        shared_tt_empty_threshold: int = 20,
    ) -> dict[str, object]:
        return {
            "success": True,
            "best_move": 19,
            "exact_margin": 12,
            "pv": [19, 26],
            "searched_nodes": 321,
            "elapsed_seconds": timeout_seconds,
            "failure_reason": None,
        }

    def _unexpected_model(**kwargs: object) -> dict[str, object]:
        raise AssertionError("model path should not run below threshold with always_try_exact")

    monkeypatch.setattr(veloversi, "search_best_move_exact", _exact_success)
    monkeypatch.setattr(veloversi, "_run_model_selection_path", _unexpected_model)
    model = _CnnPolicyModel()

    result = select_move_with_model(
        initial_board(),
        model,
        policy_mode="best",
        exact_from_empty_threshold=64,
        always_try_exact=True,
        timeout_seconds=0.5,
    )

    assert result["success"] is True
    assert result["source"] == "exact"
    assert result["best_move"] == 19
    assert result["exact_margin"] == 12


def test_finish_game_recording_requires_terminal_board() -> None:
    with pytest.raises(ValueError, match="not terminal"):
        finish_game_recording(start_game_recording(initial_board()))


def test_append_and_load_game_records_round_trip(tmp_path: Path) -> None:
    path = tmp_path / "games.jsonl"
    terminal = board_from_bits(0xFFFF_FFFF_FFFF_FFFF, 0, "black")
    record = finish_game_recording(start_game_recording(terminal))

    append_game_record(str(path), record)
    append_game_record(str(path), record)
    loaded = load_game_records(str(path))

    assert loaded == [record, record]


def test_append_game_record_rejects_invalid_format_file(tmp_path: Path) -> None:
    path = tmp_path / "invalid-games.jsonl"
    path.write_text("{bad json}\n", encoding="utf-8")
    terminal = board_from_bits(0xFFFF_FFFF_FFFF_FFFF, 0, "black")
    record = finish_game_recording(start_game_recording(terminal))

    with pytest.raises(ValueError, match="invalid JSONL"):
        append_game_record(str(path), record)


def test_open_game_record_dataset_indexes_only_policy_enabled_positions(tmp_path: Path) -> None:
    path = tmp_path / "dataset.jsonl"
    first = start_game_recording(initial_board())
    while True:
        status = first.board_status()
        if status == "terminal":
            break
        if status == "forced_pass":
            first = first.apply_forced_pass()
            continue
        first = first.apply_move(first.legal_moves_list()[0])
    terminal = board_from_bits(0xFFFF_FFFF_FFFF_FFFF, 0, "black")
    append_game_record(str(path), first.finish())
    append_game_record(str(path), start_game_recording(terminal).finish())

    dataset = open_game_record_dataset(str(path))

    expected = sum(
        1 for move in cast(list[int | None], first.finish()["moves"]) if move is not None
    )
    assert len(dataset) == expected
    assert dataset.len() == expected


def test_record_dataset_get_returns_expected_position_and_targets(tmp_path: Path) -> None:
    path = tmp_path / "dataset-targets.jsonl"
    record = start_game_recording(initial_board())
    record = record.apply_move(19)
    record = record.apply_move(record.legal_moves_list()[0])
    while True:
        status = record.board_status()
        if status == "terminal":
            break
        if status == "forced_pass":
            record = record.apply_forced_pass()
            continue
        record = record.apply_move(record.legal_moves_list()[0])
    append_game_record(str(path), record.finish())

    dataset = open_game_record_dataset(str(path))
    item = dataset.get(0)
    cnn = dataset.get_cnn_input(0)
    flat = dataset.get_flat_input(0)
    targets = dataset.get_targets(0)

    assert item["record_index"] == 0
    assert item["ply"] == 0
    assert item["global_index"] == 0
    assert item["policy_target_index"] == 19
    assert cast(Board, item["board"]).to_bits() == initial_board().to_bits()
    assert cnn.shape == (3, 8, 8)
    assert flat.shape == (192,)
    policy_target = cast(np.ndarray, targets["policy_target"])
    assert policy_target.shape == (64,)
    assert policy_target.dtype == np.float32
    assert policy_target[19] == np.float32(1.0)
    expected_margin = cast(int, item["final_margin_from_black"])
    expected_value = expected_margin / 64.0
    if cast(Board, item["board"]).side_to_move == "white":
        expected_value = -expected_value
    assert targets["value_target"] == np.float32(expected_value)


def test_record_dataset_random_start_board_replay_matches_saved_games(tmp_path: Path) -> None:
    path = tmp_path / "random-start-dataset.jsonl"
    saved_records: list[dict[str, object]] = []
    for seed in range(32):
        record = start_game_recording(random_start_board((seed % 8) + 1, seed + 100))
        while True:
            status = record.board_status()
            if status == "terminal":
                break
            if status == "forced_pass":
                record = record.apply_forced_pass()
                continue
            record = record.apply_move(record.legal_moves_list()[0])
        finished = record.finish()
        saved_records.append(finished)
        append_game_record(str(path), finished)

    dataset = open_game_record_dataset(str(path))
    running_index = 0
    for record_index, saved in enumerate(saved_records):
        start_board = unpack_board(cast(tuple[int, int, str], saved["start_board"]))
        board = start_board
        for ply, move in enumerate(cast(list[int | None], saved["moves"])):
            if move is None:
                board = board.apply_forced_pass()
                continue
            item = dataset.get(running_index)
            assert item["record_index"] == record_index
            assert item["ply"] == ply
            assert cast(Board, item["board"]).to_bits() == board.to_bits()
            running_index += 1
            board = board.apply_move(move)

    assert running_index == len(dataset)


def test_open_game_record_dataset_accepts_single_path_and_multiple_paths(tmp_path: Path) -> None:
    path_a = tmp_path / "a.jsonl"
    path_b = tmp_path / "b.jsonl"

    record_a = start_game_recording(initial_board())
    while True:
        status = record_a.board_status()
        if status == "terminal":
            break
        if status == "forced_pass":
            record_a = record_a.apply_forced_pass()
            continue
        record_a = record_a.apply_move(record_a.legal_moves_list()[0])

    record_b = start_game_recording(random_start_board(3, 123))
    while True:
        status = record_b.board_status()
        if status == "terminal":
            break
        if status == "forced_pass":
            record_b = record_b.apply_forced_pass()
            continue
        record_b = record_b.apply_move(record_b.legal_moves_list()[0])

    append_game_record(str(path_a), record_a.finish())
    append_game_record(str(path_b), record_b.finish())

    dataset_a = open_game_record_dataset(path_a)
    dataset_both = open_game_record_dataset([path_a, path_b])

    assert len(dataset_a) > 0
    assert len(dataset_both) > len(dataset_a)


def test_recorded_board_exposes_all_public_board_methods() -> None:
    board_methods = {
        name for name, value in vars(Board).items() if callable(value) and not name.startswith("_")
    }
    recorded_methods = {
        name
        for name, value in vars(RecordedBoard).items()
        if callable(value) and not name.startswith("_")
    }

    assert board_methods <= recorded_methods


def test_recorded_board_new_initial_matches_board_initial_state() -> None:
    board = initial_board()
    record = RecordedBoard.new_initial()

    assert record.start_board.to_bits() == board.to_bits()
    assert record.current_board.to_bits() == board.to_bits()
    assert record.moves == []


def test_encode_planes_and_flat_features_return_float32_arrays() -> None:
    board = initial_board()
    config = {
        "history_len": 2,
        "include_legal_mask": True,
        "include_phase_plane": True,
        "include_turn_plane": True,
        "perspective": "absolute_color",
    }

    planes = encode_planes(board, [], config)
    flat = encode_flat_features(board, [], config)

    assert isinstance(planes, np.ndarray)
    assert isinstance(flat, np.ndarray)
    assert planes.dtype == np.float32
    assert flat.dtype == np.float32
    assert planes.shape == (9, 8, 8)
    assert flat.shape == (128 * 3 + 64 + 1 + 1,)


def test_prepare_cnn_model_input_returns_batch_first_three_planes() -> None:
    batch = prepare_cnn_model_input(initial_board())

    assert batch.shape == (1, 3, 8, 8)
    assert batch.dtype == np.float32
    assert float(batch[0, 0].sum()) == 2.0
    assert float(batch[0, 1].sum()) == 2.0

    legal_squares = {
        idx for idx, value in enumerate(batch[0, 2].reshape(64).tolist()) if value == 1.0
    }
    assert legal_squares == {19, 26, 37, 44}


def test_prepare_model_input_accepts_recorded_board() -> None:
    record = start_game_recording(initial_board())
    record = record_move(record, 19)

    cnn = prepare_cnn_model_input(record)
    flat = prepare_flat_model_input(record)

    assert cnn.shape == (1, 3, 8, 8)
    assert flat.shape == (1, 192)
    assert np.array_equal(cnn, prepare_cnn_model_input(current_board(record)))
    assert np.array_equal(flat, prepare_flat_model_input(current_board(record)))


def test_prepare_model_input_batch_is_batch_first() -> None:
    board = initial_board()
    moved = apply_move(board, 19)
    record = start_game_recording(moved)

    cnn = prepare_cnn_model_input_batch([board, record])
    flat = prepare_flat_model_input_batch([board, record])

    assert cnn.shape == (2, 3, 8, 8)
    assert flat.shape == (2, 192)


def test_prepare_model_input_rejects_invalid_value() -> None:
    with pytest.raises(TypeError):
        prepare_cnn_model_input({"bad": "record"})  # pyright: ignore[reportArgumentType]


def test_encode_feature_batches_match_single_position_results() -> None:
    board_a = initial_board()
    board_b = apply_move(board_a, 19)
    config = {
        "history_len": 1,
        "include_legal_mask": True,
        "include_phase_plane": False,
        "include_turn_plane": True,
        "perspective": "absolute_color",
    }

    single_planes_a = encode_planes(board_a, [board_b], config)
    single_planes_b = encode_planes(board_b, [board_a], config)
    single_flat_a = encode_flat_features(board_a, [board_b], config)
    single_flat_b = encode_flat_features(board_b, [board_a], config)

    batch_planes = encode_planes_batch([board_a, board_b], [[board_b], [board_a]], config)
    batch_flat = encode_flat_features_batch([board_a, board_b], [[board_b], [board_a]], config)

    assert batch_planes.shape == (2,) + single_planes_a.shape
    assert batch_flat.shape == (2,) + single_flat_a.shape
    assert np.array_equal(batch_planes[0], single_planes_a)
    assert np.array_equal(batch_planes[1], single_planes_b)
    assert np.array_equal(batch_flat[0], single_flat_a)
    assert np.array_equal(batch_flat[1], single_flat_b)


def test_encode_planes_uses_newest_first_history_and_zero_fills() -> None:
    board = initial_board()
    history_newest = board_from_bits(1, 2, "black")
    history_older = board_from_bits(4, 8, "white")
    planes = encode_planes(
        board,
        [history_newest, history_older],
        {"history_len": 3, "perspective": "absolute_color"},
    )

    assert planes[0, 3, 4] == np.float32(1.0)
    assert planes[1, 3, 3] == np.float32(1.0)
    assert planes[2, 0, 0] == np.float32(1.0)
    assert planes[3, 0, 1] == np.float32(1.0)
    assert planes[4, 0, 2] == np.float32(1.0)
    assert planes[5, 0, 3] == np.float32(1.0)
    assert np.all(planes[6:8] == np.float32(0.0))


def test_encode_planes_reflects_side_to_move_perspective() -> None:
    board = board_from_bits(1, 2, "white")

    absolute = encode_planes(board, [], {"perspective": "absolute_color"})
    relative = encode_planes(board, [], {"perspective": "side_to_move"})

    assert absolute[0, 0, 0] == np.float32(1.0)
    assert absolute[1, 0, 1] == np.float32(1.0)
    assert relative[0, 0, 1] == np.float32(1.0)
    assert relative[1, 0, 0] == np.float32(1.0)


def test_feature_public_api_rejects_invalid_inputs() -> None:
    board = initial_board()

    with pytest.raises(ValueError, match="config"):
        encode_planes(board, [], [])  # type: ignore[arg-type]

    with pytest.raises(ValueError, match="history"):
        encode_planes(board, (), {"history_len": 0})  # type: ignore[arg-type]

    with pytest.raises(ValueError, match="history_len"):
        encode_planes(board, [], {"history_len": -1})

    with pytest.raises(ValueError, match="include_legal_mask"):
        encode_planes(board, [], {"include_legal_mask": 1})

    with pytest.raises(ValueError, match="perspective"):
        encode_planes(board, [], {"perspective": "bad"})

    with pytest.raises(ValueError, match="same length"):
        encode_planes_batch([board], [[], []], {"history_len": 0})


@pytest.mark.parametrize(
    ("packed", "message"),
    [
        (123, "tuple"),
        ((1, 2), "tuple"),
        ((1, 2, 3, 4), "tuple"),
        (("x", 2, "black"), r"packed\[0\]"),
        ((1, "x", "black"), r"packed\[1\]"),
        ((1, 2, 3), r"packed\[2\]"),
        ((1, 2, "bad"), "side_to_move"),
        ((1, 1, "black"), "invalid board bits"),
    ],
)
def test_unpack_board_rejects_invalid_python_inputs(packed: object, message: str) -> None:
    with pytest.raises(ValueError, match=message):
        unpack_board(packed)  # type: ignore[arg-type]

    if packed in ((1, 2, "bad"), (1, 1, "black")):
        packed_tuple = cast(tuple[int, int, str], packed)
        with pytest.raises(ValueError, match=message):
            core._unpack_board_parts(*packed_tuple)
