from __future__ import annotations

from pathlib import Path

import veloversi as vv


def main() -> None:
    output_path = Path("examples/generated_records.jsonl")

    board = vv.random_start_board(plies=6, seed=123)
    record = vv.start_game_recording(board)

    while True:
        board = vv.current_board(record)
        status = vv.board_status(board)
        if status == "terminal":
            break
        if status == "forced_pass":
            record = vv.record_pass(record)
            continue

        move = vv.legal_moves_list(board)[0]
        record = vv.record_move(record, move)

    game_record = vv.finish_game_recording(record)
    vv.append_game_record(str(output_path), game_record)
    loaded = vv.load_game_records(str(output_path))

    print(f"saved_records={len(loaded)}")
    print(f"start_board={game_record['start_board']}")
    print(f"final_result={game_record['final_result']}")
    print(f"final_black_discs={game_record['final_black_discs']}")
    print(f"final_white_discs={game_record['final_white_discs']}")


if __name__ == "__main__":
    main()
