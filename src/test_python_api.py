from pathlib import Path
from typing import cast

import numpy as np
import pytest
import veloversi
import veloversi._core as core

from veloversi import (
    Board,
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
    pack_board,
    packed_supervised_examples_from_trace,
    packed_supervised_examples_from_traces,
    play_random_game,
    prepare_cnn_model_input,
    prepare_cnn_model_input_batch,
    random_start_board,
    record_move,
    record_pass,
    start_game_recording,
    finish_game_recording,
    append_game_record,
    current_board,
    prepare_flat_learning_batch,
    prepare_flat_model_input,
    prepare_flat_model_input_batch,
    prepare_planes_learning_batch,
    sample_reachable_positions,
    supervised_examples_from_trace,
    supervised_examples_from_traces,
    transform_board,
    transform_square,
    unpack_board,
    validate_board,
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
        assert isinstance(example["board"], Board)
        assert example["board"].to_bits() == trace["boards"][ply].to_bits()
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


def test_prepare_learning_batch_rejects_nonzero_history_and_invalid_examples() -> None:
    trace = play_random_game(5, {"max_plies": 2})
    examples = packed_supervised_examples_from_trace(trace)

    with pytest.raises(ValueError, match="HistoryNotSupported"):
        prepare_planes_learning_batch(
            examples,
            {
                "history_len": 1,
                "include_legal_mask": False,
                "include_phase_plane": True,
                "include_turn_plane": True,
                "perspective": "side_to_move",
            },
        )

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


def test_random_start_board_is_reproducible() -> None:
    lhs = random_start_board(5, 123)
    rhs = random_start_board(5, 123)
    assert lhs.to_bits() == rhs.to_bits()


def test_start_game_recording_and_record_move_update_current_board() -> None:
    record = start_game_recording(initial_board())
    next_record = record_move(record, 19)

    assert current_board(record).to_bits() == initial_board().to_bits()
    assert current_board(next_record).to_bits() == apply_move(initial_board(), 19).to_bits()
    assert next_record["moves"] == [19]


def test_record_pass_updates_recording() -> None:
    board = board_from_bits(0xFFFF_FFFF_FFFF_FF7E, 0x0000_0000_0000_0080, "black")
    record = start_game_recording(board)
    next_record = record_pass(record)

    assert next_record["moves"] == [None]
    assert current_board(next_record).to_bits() == apply_forced_pass(board).to_bits()


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


def test_prepare_model_input_accepts_recording_dict() -> None:
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
        prepare_cnn_model_input({"bad": "record"})


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
