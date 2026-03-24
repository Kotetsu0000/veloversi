from __future__ import annotations

import time

from veloversi import (
    board_status,
    disc_count,
    apply_move,
    generate_legal_moves,
    initial_board,
    is_legal_move,
    legal_moves_list,
)


def bench_generate_legal_moves(iterations: int = 200_000) -> float:
    board = initial_board()
    start = time.perf_counter()
    bitmask = 0
    for _ in range(iterations):
        bitmask ^= generate_legal_moves(board)
    elapsed = time.perf_counter() - start
    print(f"python generate_legal_moves: {elapsed:.6f}s checksum={bitmask}")
    return elapsed


def bench_legal_moves_list(iterations: int = 200_000) -> float:
    board = initial_board()
    start = time.perf_counter()
    checksum = 0
    for _ in range(iterations):
        checksum ^= len(legal_moves_list(board))
    elapsed = time.perf_counter() - start
    print(f"python legal_moves_list: {elapsed:.6f}s checksum={checksum}")
    return elapsed


def bench_is_legal_move(iterations: int = 200_000) -> float:
    board = initial_board()
    square = 19
    start = time.perf_counter()
    checksum = 0
    for _ in range(iterations):
        checksum ^= int(is_legal_move(board, square))
    elapsed = time.perf_counter() - start
    print(f"python is_legal_move: {elapsed:.6f}s checksum={checksum}")
    return elapsed


def bench_disc_count(iterations: int = 200_000) -> float:
    board = initial_board()
    start = time.perf_counter()
    checksum = 0
    for _ in range(iterations):
        black, white, empty = disc_count(board)
        checksum ^= black ^ white ^ empty
    elapsed = time.perf_counter() - start
    print(f"python disc_count: {elapsed:.6f}s checksum={checksum}")
    return elapsed


def bench_apply_move(iterations: int = 200_000) -> float:
    board = initial_board()
    square = 19
    start = time.perf_counter()
    checksum = 0
    for _ in range(iterations):
        next_board = apply_move(board, square)
        checksum ^= next_board.white_bits
    elapsed = time.perf_counter() - start
    print(f"python apply_move: {elapsed:.6f}s checksum={checksum}")
    return elapsed


def bench_board_status(iterations: int = 200_000) -> float:
    board = initial_board()
    start = time.perf_counter()
    checksum = 0
    for _ in range(iterations):
        checksum ^= len(board_status(board))
    elapsed = time.perf_counter() - start
    print(f"python board_status: {elapsed:.6f}s checksum={checksum}")
    return elapsed


def main() -> None:
    print("python api bench")
    bench_generate_legal_moves()
    bench_legal_moves_list()
    bench_is_legal_move()
    bench_apply_move()
    bench_disc_count()
    bench_board_status()


if __name__ == "__main__":
    main()
