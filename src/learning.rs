use crate::engine::{Board, BoardError, generate_legal_moves};
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
    HistoryNotSupported,
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
    if config.history_len != 0 {
        return Err(LearningBatchError::HistoryNotSupported);
    }
    let boards = decode_boards(examples)?;
    let features = encode_planes_batch(&boards, &vec![Vec::new(); boards.len()], config);
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
    if config.history_len != 0 {
        return Err(LearningBatchError::HistoryNotSupported);
    }
    let boards = decode_boards(examples)?;
    let features = encode_flat_features_batch(&boards, &vec![Vec::new(); boards.len()], config);
    Ok(PreparedFlatBatch {
        features,
        value_targets: value_targets(examples),
        policy_targets: policy_targets(examples),
        legal_move_masks: legal_move_masks(&boards),
    })
}

#[cfg(test)]
mod tests {
    use super::{LearningBatchError, prepare_flat_learning_batch, prepare_planes_learning_batch};
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
    fn prepare_learning_batch_rejects_nonzero_history_len() {
        let trace = play_random_game(3, &RandomPlayConfig { max_plies: Some(2) });
        let examples = packed_supervised_examples_from_trace(&trace);
        let mut config = feature_config();
        config.history_len = 1;

        assert_eq!(
            prepare_planes_learning_batch(&examples, &config),
            Err(LearningBatchError::HistoryNotSupported)
        );
        assert_eq!(
            prepare_flat_learning_batch(&examples, &config),
            Err(LearningBatchError::HistoryNotSupported)
        );
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
}
