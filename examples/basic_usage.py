from __future__ import annotations

import veloversi as vv


def main() -> None:
    board = vv.initial_board()
    print("initial:", board.to_bits())
    print("legal moves:", board.legal_moves_list())

    next_board = board.apply_move(19)
    print("after move 19:", next_board.to_bits())
    print("status:", next_board.board_status())
    print("disc_count:", next_board.disc_count())
    print("margin_from_black:", next_board.final_margin_from_black())

    packed = vv.pack_board(next_board)
    restored = vv.unpack_board(packed)
    print("packed:", packed)
    print("restored:", restored.to_bits())
    print("rot90:", next_board.transform("rot90").to_bits())

    trace = vv.play_random_game(7, {"max_plies": 8})
    print("trace plies:", trace["plies_played"])
    print("trace final_result:", trace["final_result"])
    print("trace final_margin_from_black:", trace["final_margin_from_black"])

    examples = vv.supervised_examples_from_trace(trace)
    print("supervised examples:", len(examples))
    first_example = examples[0]
    print(
        "first example:",
        {
            "board": first_example["board"].to_bits(),
            "ply": first_example["ply"],
            "moves_until_here": first_example["moves_until_here"],
            "final_result": first_example["final_result"],
            "final_margin_from_black": first_example["final_margin_from_black"],
        },
    )

    history = trace["boards"][:3]
    feature_config = {
        "history_len": 2,
        "include_legal_mask": True,
        "include_phase_plane": True,
        "include_turn_plane": True,
        "perspective": "side_to_move",
    }
    feature_board = trace["boards"][-1]
    planes = feature_board.encode_planes(history, feature_config)
    flat = feature_board.encode_flat_features(history, feature_config)
    print("planes shape:", planes.shape)
    print("flat shape:", flat.shape)
    print("cnn input shape:", feature_board.prepare_cnn_model_input().shape)
    print("flat input shape:", feature_board.prepare_flat_model_input().shape)

    sampled = vv.sample_reachable_positions(
        11, {"num_positions": 3, "min_plies": 4, "max_plies": 8}
    )
    print("sampled positions:", [position.to_bits() for position in sampled])


if __name__ == "__main__":
    main()
