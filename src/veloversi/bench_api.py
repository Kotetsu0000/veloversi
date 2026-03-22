from __future__ import annotations

import time

from veloversi import (
    apply_move,
    apply_move_bits,
    apply_move_unchecked,
    apply_move_unchecked_bits,
    generate_legal_moves,
    generate_legal_moves_bits,
    initial_board,
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


def bench_generate_legal_moves_bits(iterations: int = 200_000) -> float:
    black_bits, white_bits, side_to_move = initial_board().to_bits()
    start = time.perf_counter()
    bitmask = 0
    for _ in range(iterations):
        bitmask ^= generate_legal_moves_bits(black_bits, white_bits, side_to_move)
    elapsed = time.perf_counter() - start
    print(f"python generate_legal_moves_bits: {elapsed:.6f}s checksum={bitmask}")
    return elapsed


def bench_apply_move_unchecked(iterations: int = 200_000) -> float:
    board = initial_board()
    square = 19
    start = time.perf_counter()
    checksum = 0
    for _ in range(iterations):
        next_board = apply_move_unchecked(board, square)
        checksum ^= next_board.black_bits
    elapsed = time.perf_counter() - start
    print(f"python apply_move_unchecked: {elapsed:.6f}s checksum={checksum}")
    return elapsed


def bench_apply_move_unchecked_bits(iterations: int = 200_000) -> float:
    black_bits, white_bits, side_to_move = initial_board().to_bits()
    square = 19
    start = time.perf_counter()
    checksum = 0
    for _ in range(iterations):
        next_black_bits, _, _ = apply_move_unchecked_bits(
            black_bits, white_bits, side_to_move, square
        )
        checksum ^= next_black_bits
    elapsed = time.perf_counter() - start
    print(f"python apply_move_unchecked_bits: {elapsed:.6f}s checksum={checksum}")
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


def bench_apply_move_bits(iterations: int = 200_000) -> float:
    black_bits, white_bits, side_to_move = initial_board().to_bits()
    square = 19
    start = time.perf_counter()
    checksum = 0
    for _ in range(iterations):
        _, next_white_bits, _ = apply_move_bits(black_bits, white_bits, side_to_move, square)
        checksum ^= next_white_bits
    elapsed = time.perf_counter() - start
    print(f"python apply_move_bits: {elapsed:.6f}s checksum={checksum}")
    return elapsed


def main() -> None:
    print("python api bench")
    bench_generate_legal_moves()
    bench_generate_legal_moves_bits()
    bench_apply_move_unchecked()
    bench_apply_move_unchecked_bits()
    bench_apply_move()
    bench_apply_move_bits()


if __name__ == "__main__":
    main()
