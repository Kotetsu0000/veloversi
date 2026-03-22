from veloversi import (
    Board,
    apply_move,
    apply_move_bits,
    apply_move_unchecked,
    apply_move_unchecked_bits,
    board_from_bits,
    generate_legal_moves,
    generate_legal_moves_bits,
    initial_board,
)


def test_board_round_trip_with_bits_api() -> None:
    board = initial_board()

    assert isinstance(board, Board)
    assert board.to_bits() == (board.black_bits, board.white_bits, board.side_to_move)
    assert board_from_bits(*board.to_bits()).to_bits() == board.to_bits()


def test_generate_legal_moves_bits_matches_object_api() -> None:
    board = initial_board()

    assert generate_legal_moves(board) == 0x0000_1020_0408_0000
    assert generate_legal_moves_bits(*board.to_bits()) == generate_legal_moves(board)


def test_apply_move_bits_matches_object_api() -> None:
    board = initial_board()
    square = 19

    next_board = apply_move(board, square)
    assert apply_move_bits(*board.to_bits(), square) == next_board.to_bits()


def test_apply_move_unchecked_bits_matches_object_api() -> None:
    board = initial_board()
    square = 19

    next_board = apply_move_unchecked(board, square)
    assert apply_move_unchecked_bits(*board.to_bits(), square) == next_board.to_bits()
