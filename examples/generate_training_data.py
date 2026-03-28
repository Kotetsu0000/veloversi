from __future__ import annotations

import argparse
import json
import random
from pathlib import Path
from typing import Any

import veloversi as vv


def play_random_game_manual(seed: int) -> dict[str, Any]:
    rng = random.Random(seed)
    board = vv.initial_board()
    boards = [board]
    moves: list[int | None] = []

    while True:
        status = vv.board_status(board)
        if status == "terminal":
            break
        if status == "forced_pass":
            board = vv.apply_forced_pass(board)
            moves.append(None)
            boards.append(board)
            continue

        legal_moves = vv.legal_moves_list(board)
        move = rng.choice(legal_moves)
        board = vv.apply_move(board, move)
        moves.append(move)
        boards.append(board)

    return {
        "boards": boards,
        "moves": moves,
        "final_result": vv.game_result(board),
        "final_margin_from_black": vv.final_margin_from_black(board),
        "plies_played": len(moves),
        "reached_terminal": True,
    }


def board_tuple_to_dict(board_tuple: tuple[int, int, str]) -> dict[str, int | str]:
    black_bits, white_bits, side_to_move = board_tuple
    return {
        "black_bits": black_bits,
        "white_bits": white_bits,
        "side_to_move": side_to_move,
    }


def save_examples(output_dir: Path, num_games: int, seed: int) -> Path:
    output_dir.mkdir(parents=True, exist_ok=True)
    output_path = output_dir / "examples.jsonl"

    with output_path.open("w", encoding="utf-8") as f:
        for game_index in range(num_games):
            trace = play_random_game_manual(seed + game_index)
            for example_index, example in enumerate(vv.packed_supervised_examples_from_trace(trace)):
                record = {
                    "game_index": game_index,
                    "example_index": example_index,
                    "board": board_tuple_to_dict(example["board"]),
                    "ply": example["ply"],
                    "moves_until_here": example["moves_until_here"],
                    "final_result": example["final_result"],
                    "final_margin_from_black": example["final_margin_from_black"],
                    "policy_target_index": example["policy_target_index"],
                    "policy_target_square": example["policy_target_square"],
                    "policy_target_is_pass": example["policy_target_is_pass"],
                    "has_policy_target": example["has_policy_target"],
                }
                f.write(json.dumps(record, ensure_ascii=True) + "\n")

    return output_path


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate policy/value training data.")
    parser.add_argument("--output-dir", type=Path, default=Path("examples/generated_data"))
    parser.add_argument("--num-games", type=int, default=2)
    parser.add_argument("--seed", type=int, default=123)
    args = parser.parse_args()

    output_path = save_examples(args.output_dir, args.num_games, args.seed)
    print(f"saved: {output_path}")

    with output_path.open("r", encoding="utf-8") as f:
        first_record = json.loads(f.readline())
    print(
        "first record:",
        {
            "board": first_record["board"],
            "ply": first_record["ply"],
            "policy_target_index": first_record["policy_target_index"],
            "final_result": first_record["final_result"],
            "final_margin_from_black": first_record["final_margin_from_black"],
        },
    )


if __name__ == "__main__":
    main()
