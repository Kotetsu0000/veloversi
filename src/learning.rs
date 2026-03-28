use crate::engine::{Board, BoardError, Move, apply_forced_pass, apply_move, generate_legal_moves};
use crate::feature::{
    EncodedFlatFeaturesBatch, EncodedPlanesBatch, FeatureConfig, encode_flat_features_batch,
    encode_planes_batch,
};
use crate::random_play::PackedSupervisedExample;
use crate::serialize::unpack_board;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LearningBatchError {
    InvalidBoard(BoardError),
    InvalidPolicyTargetIndex(i8),
    InvalidHistoryMove(u8),
    InvalidHistoryPass,
    ReplayMismatch,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedPlanesBatch {
    pub features: EncodedPlanesBatch,
    pub value_targets: Vec<f32>,
    pub policy_targets: Vec<i16>,
    pub legal_move_masks: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedFlatBatch {
    pub features: EncodedFlatFeaturesBatch,
    pub value_targets: Vec<f32>,
    pub policy_targets: Vec<i16>,
    pub legal_move_masks: Vec<f32>,
}

fn validate_policy_target_index(policy_target_index: i8) -> Result<(), LearningBatchError> {
    if (-1..=64).contains(&policy_target_index) {
        Ok(())
    } else {
        Err(LearningBatchError::InvalidPolicyTargetIndex(
            policy_target_index,
        ))
    }
}

fn decode_boards(examples: &[PackedSupervisedExample]) -> Result<Vec<Board>, LearningBatchError> {
    let mut boards = Vec::with_capacity(examples.len());
    for example in examples {
        validate_policy_target_index(example.policy_target_index)?;
        boards.push(unpack_board(example.board).map_err(LearningBatchError::InvalidBoard)?);
    }
    Ok(boards)
}

fn replay_moves_to_history(
    current_board: &Board,
    moves_until_here: &[Option<u8>],
    history_len: usize,
) -> Result<Vec<Board>, LearningBatchError> {
    if history_len == 0 {
        return Ok(Vec::new());
    }

    let mut board = Board::new_initial();
    let mut boards = vec![board];

    for mv in moves_until_here {
        board = match mv {
            Some(square) => apply_move(&board, Move { square: *square })
                .map_err(|_| LearningBatchError::InvalidHistoryMove(*square))?,
            None => {
                apply_forced_pass(&board).map_err(|_| LearningBatchError::InvalidHistoryPass)?
            }
        };
        boards.push(board);
    }

    if &board != current_board {
        return Err(LearningBatchError::ReplayMismatch);
    }

    let mut history = Vec::with_capacity(history_len);
    for idx in 1..=history_len {
        if boards.len() > idx {
            history.push(boards[boards.len() - 1 - idx]);
        } else {
            break;
        }
    }
    Ok(history)
}

fn decode_histories(
    examples: &[PackedSupervisedExample],
    boards: &[Board],
    history_len: usize,
) -> Result<Vec<Vec<Board>>, LearningBatchError> {
    let mut histories = Vec::with_capacity(examples.len());
    for (example, board) in examples.iter().zip(boards.iter()) {
        histories.push(replay_moves_to_history(
            board,
            &example.moves_until_here,
            history_len,
        )?);
    }
    Ok(histories)
}

fn value_targets(examples: &[PackedSupervisedExample]) -> Vec<f32> {
    examples
        .iter()
        .map(|example| example.final_margin_from_black as f32)
        .collect()
}

fn policy_targets(examples: &[PackedSupervisedExample]) -> Vec<i16> {
    examples
        .iter()
        .map(|example| example.policy_target_index as i16)
        .collect()
}

fn legal_move_masks(boards: &[Board]) -> Vec<f32> {
    let mut masks = vec![0.0; boards.len() * 64];
    for (idx, board) in boards.iter().enumerate() {
        let mut bits = generate_legal_moves(board).bitmask;
        let offset = idx * 64;
        while bits != 0 {
            let square = bits.trailing_zeros() as usize;
            masks[offset + square] = 1.0;
            bits &= bits - 1;
        }
    }
    masks
}

pub fn prepare_planes_learning_batch(
    examples: &[PackedSupervisedExample],
    config: &FeatureConfig,
) -> Result<PreparedPlanesBatch, LearningBatchError> {
    let boards = decode_boards(examples)?;
    let histories = decode_histories(examples, &boards, config.history_len)?;
    let features = encode_planes_batch(&boards, &histories, config);
    Ok(PreparedPlanesBatch {
        features,
        value_targets: value_targets(examples),
        policy_targets: policy_targets(examples),
        legal_move_masks: legal_move_masks(&boards),
    })
}

pub fn prepare_flat_learning_batch(
    examples: &[PackedSupervisedExample],
    config: &FeatureConfig,
) -> Result<PreparedFlatBatch, LearningBatchError> {
    let boards = decode_boards(examples)?;
    let histories = decode_histories(examples, &boards, config.history_len)?;
    let features = encode_flat_features_batch(&boards, &histories, config);
    Ok(PreparedFlatBatch {
        features,
        value_targets: value_targets(examples),
        policy_targets: policy_targets(examples),
        legal_move_masks: legal_move_masks(&boards),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        LearningBatchError, prepare_flat_learning_batch, prepare_planes_learning_batch,
        replay_moves_to_history,
    };
    use crate::feature::{FeatureConfig, FeaturePerspective};
    use crate::random_play::{
        RandomPlayConfig, packed_supervised_examples_from_trace, play_random_game,
    };

    fn feature_config() -> FeatureConfig {
        FeatureConfig {
            history_len: 0,
            include_legal_mask: false,
            include_phase_plane: true,
            include_turn_plane: true,
            perspective: FeaturePerspective::SideToMove,
        }
    }

    #[test]
    fn prepare_planes_learning_batch_returns_expected_shapes_and_targets() {
        let trace = play_random_game(7, &RandomPlayConfig { max_plies: Some(4) });
        let examples = packed_supervised_examples_from_trace(&trace);
        let batch = prepare_planes_learning_batch(&examples, &feature_config()).expect("valid");

        assert_eq!(batch.features.batch, examples.len());
        assert_eq!(batch.features.height, 8);
        assert_eq!(batch.features.width, 8);
        assert_eq!(batch.value_targets.len(), examples.len());
        assert_eq!(batch.policy_targets.len(), examples.len());
        assert_eq!(batch.legal_move_masks.len(), examples.len() * 64);
        assert_eq!(
            batch.value_targets[0],
            examples[0].final_margin_from_black as f32
        );
        assert_eq!(
            batch.policy_targets[0],
            examples[0].policy_target_index as i16
        );
    }

    #[test]
    fn prepare_flat_learning_batch_returns_expected_shapes_and_targets() {
        let trace = play_random_game(11, &RandomPlayConfig { max_plies: Some(3) });
        let examples = packed_supervised_examples_from_trace(&trace);
        let batch = prepare_flat_learning_batch(&examples, &feature_config()).expect("valid");

        assert_eq!(batch.features.batch, examples.len());
        assert_eq!(batch.value_targets.len(), examples.len());
        assert_eq!(batch.policy_targets.len(), examples.len());
        assert_eq!(batch.legal_move_masks.len(), examples.len() * 64);
    }

    #[test]
    fn prepare_learning_batch_rejects_invalid_policy_target_index() {
        let trace = play_random_game(5, &RandomPlayConfig { max_plies: Some(1) });
        let mut examples = packed_supervised_examples_from_trace(&trace);
        examples[0].policy_target_index = 65;

        assert_eq!(
            prepare_planes_learning_batch(&examples, &feature_config()),
            Err(LearningBatchError::InvalidPolicyTargetIndex(65))
        );
    }

    #[test]
    fn prepare_learning_batch_ignores_moves_until_here_when_history_is_disabled() {
        let trace = play_random_game(9, &RandomPlayConfig { max_plies: Some(2) });
        let examples = packed_supervised_examples_from_trace(&trace);
        let batch = prepare_flat_learning_batch(&examples, &feature_config()).expect("valid");

        assert_eq!(batch.features.batch, examples.len());
    }

    #[test]
    fn legal_move_mask_matches_current_board_only() {
        let trace = play_random_game(13, &RandomPlayConfig { max_plies: Some(1) });
        let examples = packed_supervised_examples_from_trace(&trace);
        let batch =
            prepare_planes_learning_batch(&examples[..1], &feature_config()).expect("valid");

        let mask = &batch.legal_move_masks[..64];
        let active: Vec<usize> = mask
            .iter()
            .enumerate()
            .filter_map(|(idx, &value)| if value == 1.0 { Some(idx) } else { None })
            .collect();
        assert_eq!(active, vec![19, 26, 37, 44]);
    }

    #[test]
    fn batch_api_supports_b_equals_one() {
        let trace = play_random_game(17, &RandomPlayConfig { max_plies: Some(1) });
        let examples = packed_supervised_examples_from_trace(&trace);
        let batch = prepare_flat_learning_batch(&examples[..1], &feature_config()).expect("valid");

        assert_eq!(batch.features.batch, 1);
        assert_eq!(batch.value_targets.len(), 1);
        assert_eq!(batch.policy_targets.len(), 1);
        assert_eq!(batch.legal_move_masks.len(), 64);
    }

    #[test]
    fn replay_moves_to_history_reconstructs_newest_first_history() {
        let trace = play_random_game(21, &RandomPlayConfig { max_plies: Some(4) });
        let current = trace.boards[4];
        let history = replay_moves_to_history(
            &current,
            &trace.moves[..4]
                .iter()
                .map(|mv| mv.map(|mv| mv.square))
                .collect::<Vec<_>>(),
            3,
        )
        .expect("valid replay");

        assert_eq!(
            history,
            vec![trace.boards[3], trace.boards[2], trace.boards[1]]
        );
    }

    #[test]
    fn replay_moves_to_history_rejects_mismatched_current_board() {
        let trace = play_random_game(22, &RandomPlayConfig { max_plies: Some(3) });
        let err = replay_moves_to_history(
            &trace.boards[2],
            &trace.moves[..3]
                .iter()
                .map(|mv| mv.map(|mv| mv.square))
                .collect::<Vec<_>>(),
            2,
        );

        assert_eq!(err, Err(LearningBatchError::ReplayMismatch));
    }

    #[test]
    fn prepare_learning_batch_supports_nonzero_history_len() {
        let trace = play_random_game(31, &RandomPlayConfig { max_plies: Some(4) });
        let examples = packed_supervised_examples_from_trace(&trace);
        let mut config = feature_config();
        config.history_len = 2;

        let planes = prepare_planes_learning_batch(&examples, &config).expect("valid");
        let flat = prepare_flat_learning_batch(&examples, &config).expect("valid");

        assert_eq!(planes.features.batch, examples.len());
        assert_eq!(flat.features.batch, examples.len());
        assert!(planes.features.channels > feature_config().history_len);
        assert!(
            flat.features.len
                > prepare_flat_learning_batch(&examples, &feature_config())
                    .expect("base")
                    .features
                    .len
        );
    }

    #[test]
    fn prepare_learning_batch_rejects_invalid_history_pass() {
        let trace = play_random_game(33, &RandomPlayConfig { max_plies: Some(1) });
        let mut examples = packed_supervised_examples_from_trace(&trace);
        examples[0].moves_until_here = vec![None];
        let mut config = feature_config();
        config.history_len = 1;

        assert_eq!(
            prepare_planes_learning_batch(&examples, &config),
            Err(LearningBatchError::InvalidHistoryPass)
        );
    }
}
