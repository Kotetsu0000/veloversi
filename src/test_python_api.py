import pytest
import veloversi

from veloversi import (
    Board,
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
    validate_board,
)


def test_board_round_trip_and_validate() -> None:
    board = initial_board()

    assert isinstance(board, Board)
    assert board.to_bits() == (board.black_bits, board.white_bits, board.side_to_move)
    assert board_from_bits(*board.to_bits()).to_bits() == board.to_bits()
    validate_board(board)


def test_generate_legal_moves_helpers_match_initial_position() -> None:
    board = initial_board()

    assert generate_legal_moves(board) == 0x0000_1020_0408_0000
    assert legal_moves_list(board) == [19, 26, 37, 44]
    assert is_legal_move(board, 19)
    assert not is_legal_move(board, 0)


def test_game_helpers_match_expected_values() -> None:
    board = initial_board()

    assert board_status(board) == "ongoing"
    assert disc_count(board) == (2, 2, 60)
    assert game_result(board) == "draw"
    assert final_margin_from_black(board) == 0


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


def test_step10_python_public_surface_matches_policy() -> None:
    assert not hasattr(veloversi, "apply_move_unchecked")
    assert not hasattr(veloversi, "generate_legal_moves_bits")
    assert not hasattr(veloversi, "apply_move_bits")
    assert not hasattr(veloversi._core, "apply_move_unchecked")
    assert not hasattr(veloversi._core, "generate_legal_moves_bits")
    assert not hasattr(veloversi._core, "apply_move_bits")

    with pytest.raises(ValueError):
        board_from_bits(1, 1, "black")
