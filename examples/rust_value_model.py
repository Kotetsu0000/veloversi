from pathlib import Path

import veloversi as vv


def main() -> None:
    try:
        import torch
    except ModuleNotFoundError as exc:
        raise RuntimeError("this example requires torch to be installed") from exc

    model = vv.model.NNUE()
    state_dict = torch.load("model_weights.pth", map_location="cpu")
    model.load_state_dict(state_dict)

    vv.export_model("model_weights.pth", "model_weights.vvm")
    rust_model = vv.load_model("model_weights.vvm")

    board = vv.initial_board()
    nnue_input = board.prepare_nnue_model_input()
    result = board.select_move_with_model(
        rust_model,
        depth=2,
        timeout_seconds=1.0,
        exact_from_empty_threshold=16,
    )

    print(f"nnue_input_shape={nnue_input.shape}")
    print(f"best_move={result['best_move']} source={result['source']}")
    print(f"exported={Path('model_weights.vvm').resolve()}")


if __name__ == "__main__":
    main()
