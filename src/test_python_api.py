from typing import cast

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
    final_margin_from_black,
    game_result,
    generate_legal_moves,
    initial_board,
    is_legal_move,
    legal_moves_list,
    pack_board,
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
