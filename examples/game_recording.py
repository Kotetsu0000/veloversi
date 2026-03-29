from __future__ import annotations

from pathlib import Path

import veloversi as vv


def main() -> None:
    output_path = Path("examples/generated_records.jsonl")

    board = vv.random_start_board(plies=6, seed=123)
    record = vv.start_game_recording(board)

    while True:
        board = record.current_board
        status = vv.board_status(board)
        if status == "terminal":
            break
        if status == "forced_pass":
            record = record.apply_forced_pass()
            continue

        move = board.legal_moves_list()[0]
        record = record.apply_move(move)

    game_record = vv.finish_game_recording(record)
    record.save_record(str(output_path))
    loaded = vv.load_game_records(str(output_path))

    print(f"saved_records={len(loaded)}")
    print(f"start_board={game_record['start_board']}")
    print(f"final_result={game_record['final_result']}")
    print(f"final_black_discs={game_record['final_black_discs']}")
    print(f"final_white_discs={game_record['final_white_discs']}")


if __name__ == "__main__":
    main()
