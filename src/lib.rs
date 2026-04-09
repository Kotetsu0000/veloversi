pub mod engine;
mod feature;
mod flip_tables;
mod learning;
mod nnue;
mod python;
mod random_play;
mod recording;
mod search;
mod serialize;
mod symmetry;

pub use engine::{
    Board, BoardError, BoardStatus, Color, DiscCount, GameResult, LegalMoves, Move, MoveError,
    PerftError, apply_forced_pass, apply_move, board_status, disc_count, final_margin_from_black,
    final_margin_from_side_to_move, game_result, generate_legal_moves, is_legal_move,
    legal_moves_to_vec, perft,
};
pub use feature::{
    EncodedFlatFeatures, EncodedFlatFeaturesBatch, EncodedPlanes, EncodedPlanesBatch,
    FeatureConfig, FeaturePerspective, encode_flat_features, encode_flat_features_batch,
    encode_planes, encode_planes_batch,
};
pub use learning::{
    LearningBatchError, PreparedFlatBatch, PreparedPlanesBatch, prepare_flat_learning_batch,
    prepare_planes_learning_batch,
};
pub use nnue::{
    EncodedNnueInput, NNUE_ACCUMULATOR_DIM, NNUE_ARCHITECTURE, NNUE_FORMAT, NNUE_HIDDEN_DIM,
    NNUE_INPUT_LEN, NNUE_PATTERN_FAMILIES, NNUE_PATTERN_SLOTS, NNUE_SCALAR_SLOTS, NnueModelError,
    NnueValueModel, load_rust_value_model, prepare_nnue_model_input,
};
pub use random_play::{
    PackedSupervisedExample, PositionSamplingConfig, RandomGameTrace, RandomPlayConfig,
    SupervisedExample, packed_supervised_examples_from_trace,
    packed_supervised_examples_from_traces, play_random_game, sample_reachable_positions,
    supervised_examples_from_trace, supervised_examples_from_traces,
};
pub use recording::{
    GameRecord, GameRecording, RecordingError, append_game_record, current_board,
    finish_game_recording, load_game_records, random_start_board, record_move, record_pass,
    start_game_recording,
};
pub use search::{
    ExactSearchConfig, ExactSearchFailure, ExactSearchFailureReason, ScoreKind, SearchConfig,
    SearchResult, SolveConfig, SolveError, SolveResult, can_solve_exact, search_best_move,
    search_best_move_exact, search_best_move_exact_with_config, solve_exact,
};
pub use serialize::{PackedBoard, pack_board, unpack_board};
pub use symmetry::{Symmetry, all_symmetries, transform_board, transform_square};
