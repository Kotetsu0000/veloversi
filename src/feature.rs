use crate::engine::{Board, Color, generate_legal_moves};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeaturePerspective {
    AbsoluteColor,
    SideToMove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeatureConfig {
    pub history_len: usize,
    pub include_legal_mask: bool,
    pub include_phase_plane: bool,
    pub include_turn_plane: bool,
    pub perspective: FeaturePerspective,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EncodedPlanes {
    pub channels: usize,
    pub width: usize,
    pub height: usize,
    pub data_f32: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EncodedPlanesBatch {
    pub batch: usize,
    pub channels: usize,
    pub width: usize,
    pub height: usize,
    pub data_f32: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EncodedFlatFeatures {
    pub len: usize,
    pub data_f32: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EncodedFlatFeaturesBatch {
    pub batch: usize,
    pub len: usize,
    pub data_f32: Vec<f32>,
}

fn feature_anchor_color(current: &Board, perspective: FeaturePerspective) -> Color {
    match perspective {
        FeaturePerspective::AbsoluteColor => Color::Black,
        FeaturePerspective::SideToMove => current.side_to_move,
    }
}

fn feature_frame_board(current: &Board, history: &[Board], frame_idx: usize) -> Option<Board> {
    if frame_idx == 0 {
        Some(*current)
    } else {
        history.get(frame_idx - 1).copied()
    }
}

fn feature_plane_bits(board: &Board, anchor_color: Color) -> (u64, u64) {
    match anchor_color {
        Color::Black => (board.black_bits, board.white_bits),
        Color::White => (board.white_bits, board.black_bits),
    }
}

fn feature_plane_channels(config: &FeatureConfig) -> usize {
    let mut channels = 2 * (1 + config.history_len);
    if config.include_legal_mask {
        channels += 1;
    }
    if config.include_phase_plane {
        channels += 1;
    }
    if config.include_turn_plane {
        channels += 1;
    }
    channels
}

fn feature_flat_len(config: &FeatureConfig) -> usize {
    let mut len = 128 * (1 + config.history_len);
    if config.include_legal_mask {
        len += 64;
    }
    if config.include_phase_plane {
        len += 1;
    }
    if config.include_turn_plane {
        len += 1;
    }
    len
}

fn phase_value(board: &Board) -> f32 {
    let plies = board.occupied_bits().count_ones().saturating_sub(4) as f32;
    plies / 60.0
}

fn turn_value(board: &Board) -> f32 {
    match board.side_to_move {
        Color::Black => 1.0,
        Color::White => 0.0,
    }
}

fn write_bit_plane(dst: &mut [f32], bits: u64) {
    let mut src = bits;
    while src != 0 {
        let square = src.trailing_zeros() as usize;
        dst[square] = 1.0;
        src &= src - 1;
    }
}

fn write_scalar_plane(dst: &mut [f32], value: f32) {
    dst.fill(value);
}

fn encode_planes_into(
    dst: &mut [f32],
    current: &Board,
    history: &[Board],
    config: &FeatureConfig,
) -> EncodedPlanes {
    let channels = feature_plane_channels(config);
    let anchor_color = feature_anchor_color(current, config.perspective);
    let mut channel_idx = 0usize;

    for frame_idx in 0..=config.history_len {
        if let Some(board) = feature_frame_board(current, history, frame_idx) {
            let (first_bits, second_bits) = feature_plane_bits(&board, anchor_color);
            write_bit_plane(
                &mut dst[channel_idx * 64..(channel_idx + 1) * 64],
                first_bits,
            );
            write_bit_plane(
                &mut dst[(channel_idx + 1) * 64..(channel_idx + 2) * 64],
                second_bits,
            );
        }
        channel_idx += 2;
    }

    if config.include_legal_mask {
        write_bit_plane(
            &mut dst[channel_idx * 64..(channel_idx + 1) * 64],
            generate_legal_moves(current).bitmask,
        );
        channel_idx += 1;
    }

    if config.include_phase_plane {
        write_scalar_plane(
            &mut dst[channel_idx * 64..(channel_idx + 1) * 64],
            phase_value(current),
        );
        channel_idx += 1;
    }

    if config.include_turn_plane {
        write_scalar_plane(
            &mut dst[channel_idx * 64..(channel_idx + 1) * 64],
            turn_value(current),
        );
    }

    EncodedPlanes {
        channels,
        width: 8,
        height: 8,
        data_f32: dst.to_vec(),
    }
}

pub fn encode_planes(current: &Board, history: &[Board], config: &FeatureConfig) -> EncodedPlanes {
    let mut data = vec![0.0; feature_plane_channels(config) * 64];
    encode_planes_into(&mut data, current, history, config)
}

pub fn encode_planes_batch(
    boards: &[Board],
    histories: &[Vec<Board>],
    config: &FeatureConfig,
) -> EncodedPlanesBatch {
    let channels = feature_plane_channels(config);
    let mut data = vec![0.0; boards.len() * channels * 64];

    for (idx, board) in boards.iter().enumerate() {
        let offset = idx * channels * 64;
        let history = histories.get(idx).map(Vec::as_slice).unwrap_or(&[]);
        encode_planes_into(
            &mut data[offset..offset + channels * 64],
            board,
            history,
            config,
        );
    }

    EncodedPlanesBatch {
        batch: boards.len(),
        channels,
        width: 8,
        height: 8,
        data_f32: data,
    }
}

fn write_bit_vector(dst: &mut [f32], bits: u64) {
    let mut src = bits;
    while src != 0 {
        let square = src.trailing_zeros() as usize;
        dst[square] = 1.0;
        src &= src - 1;
    }
}

fn encode_flat_features_into(
    dst: &mut [f32],
    current: &Board,
    history: &[Board],
    config: &FeatureConfig,
) -> EncodedFlatFeatures {
    let anchor_color = feature_anchor_color(current, config.perspective);
    let mut offset = 0usize;

    for frame_idx in 0..=config.history_len {
        if let Some(board) = feature_frame_board(current, history, frame_idx) {
            let (first_bits, second_bits) = feature_plane_bits(&board, anchor_color);
            write_bit_vector(&mut dst[offset..offset + 64], first_bits);
            write_bit_vector(&mut dst[offset + 64..offset + 128], second_bits);
        }
        offset += 128;
    }

    if config.include_legal_mask {
        write_bit_vector(
            &mut dst[offset..offset + 64],
            generate_legal_moves(current).bitmask,
        );
        offset += 64;
    }

    if config.include_phase_plane {
        dst[offset] = phase_value(current);
        offset += 1;
    }

    if config.include_turn_plane {
        dst[offset] = turn_value(current);
    }

    EncodedFlatFeatures {
        len: dst.len(),
        data_f32: dst.to_vec(),
    }
}

pub fn encode_flat_features(
    current: &Board,
    history: &[Board],
    config: &FeatureConfig,
) -> EncodedFlatFeatures {
    let mut data = vec![0.0; feature_flat_len(config)];
    encode_flat_features_into(&mut data, current, history, config)
}

pub fn encode_flat_features_batch(
    boards: &[Board],
    histories: &[Vec<Board>],
    config: &FeatureConfig,
) -> EncodedFlatFeaturesBatch {
    let len = feature_flat_len(config);
    let mut data = vec![0.0; boards.len() * len];

    for (idx, board) in boards.iter().enumerate() {
        let offset = idx * len;
        let history = histories.get(idx).map(Vec::as_slice).unwrap_or(&[]);
        encode_flat_features_into(&mut data[offset..offset + len], board, history, config);
    }

    EncodedFlatFeaturesBatch {
        batch: boards.len(),
        len,
        data_f32: data,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EncodedFlatFeatures, EncodedPlanes, FeatureConfig, FeaturePerspective,
        encode_flat_features, encode_flat_features_batch, encode_planes, encode_planes_batch,
    };
    use crate::{Board, Color, apply_move};

    fn bit(square: u8) -> u64 {
        1u64 << square
    }

    fn default_feature_config() -> FeatureConfig {
        FeatureConfig {
            history_len: 0,
            include_legal_mask: false,
            include_phase_plane: false,
            include_turn_plane: false,
            perspective: FeaturePerspective::AbsoluteColor,
        }
    }

    #[test]
    fn encode_planes_reports_expected_shape() {
        let config = FeatureConfig {
            history_len: 2,
            include_legal_mask: true,
            include_phase_plane: true,
            include_turn_plane: true,
            perspective: FeaturePerspective::AbsoluteColor,
        };
        let encoded: EncodedPlanes = encode_planes(&Board::new_initial(), &[], &config);

        assert_eq!(encoded.channels, 9);
        assert_eq!(encoded.width, 8);
        assert_eq!(encoded.height, 8);
        assert_eq!(encoded.data_f32.len(), 9 * 64);
    }

    #[test]
    fn encode_flat_features_reports_expected_shape() {
        let config = FeatureConfig {
            history_len: 2,
            include_legal_mask: true,
            include_phase_plane: true,
            include_turn_plane: true,
            perspective: FeaturePerspective::AbsoluteColor,
        };
        let encoded: EncodedFlatFeatures =
            encode_flat_features(&Board::new_initial(), &[], &config);

        assert_eq!(encoded.len, 128 * 3 + 64 + 1 + 1);
        assert_eq!(encoded.data_f32.len(), encoded.len);
    }

    #[test]
    fn encode_feature_batches_match_single_position_encoders() {
        let board_a = Board::new_initial();
        let board_b = apply_move(&board_a, crate::Move { square: 19 }).expect("move must succeed");
        let config = FeatureConfig {
            history_len: 1,
            include_legal_mask: true,
            include_phase_plane: false,
            include_turn_plane: true,
            perspective: FeaturePerspective::AbsoluteColor,
        };
        let planes_a = encode_planes(&board_a, &[board_b], &config);
        let planes_b = encode_planes(&board_b, &[board_a], &config);
        let flat_a = encode_flat_features(&board_a, &[board_b], &config);
        let flat_b = encode_flat_features(&board_b, &[board_a], &config);

        let planes_batch = encode_planes_batch(
            &[board_a, board_b],
            &[vec![board_b], vec![board_a]],
            &config,
        );
        let flat_batch = encode_flat_features_batch(
            &[board_a, board_b],
            &[vec![board_b], vec![board_a]],
            &config,
        );

        let plane_stride = planes_a.data_f32.len();
        assert_eq!(
            &planes_batch.data_f32[0..plane_stride],
            planes_a.data_f32.as_slice()
        );
        assert_eq!(
            &planes_batch.data_f32[plane_stride..plane_stride * 2],
            planes_b.data_f32.as_slice()
        );

        let flat_stride = flat_a.data_f32.len();
        assert_eq!(
            &flat_batch.data_f32[0..flat_stride],
            flat_a.data_f32.as_slice()
        );
        assert_eq!(
            &flat_batch.data_f32[flat_stride..flat_stride * 2],
            flat_b.data_f32.as_slice()
        );
    }

    #[test]
    fn encode_planes_uses_newest_first_history_and_zero_fills_missing_slots() {
        let current = Board::new_initial();
        let history_newest = Board {
            black_bits: bit(0),
            white_bits: bit(1),
            side_to_move: Color::Black,
        };
        let history_older = Board {
            black_bits: bit(2),
            white_bits: bit(3),
            side_to_move: Color::White,
        };
        let config = FeatureConfig {
            history_len: 3,
            ..default_feature_config()
        };
        let encoded = encode_planes(&current, &[history_newest, history_older], &config);

        assert_eq!(encoded.data_f32[28], 1.0);
        assert_eq!(encoded.data_f32[64 + 27], 1.0);
        assert_eq!(encoded.data_f32[128], 1.0);
        assert_eq!(encoded.data_f32[193], 1.0);
        assert_eq!(encoded.data_f32[258], 1.0);
        assert_eq!(encoded.data_f32[323], 1.0);
        assert!(encoded.data_f32[384..512].iter().all(|&value| value == 0.0));
    }

    #[test]
    fn encode_features_reflect_side_to_move_perspective() {
        let board = Board {
            black_bits: bit(0),
            white_bits: bit(1),
            side_to_move: Color::White,
        };
        let absolute = encode_planes(
            &board,
            &[],
            &FeatureConfig {
                perspective: FeaturePerspective::AbsoluteColor,
                ..default_feature_config()
            },
        );
        let relative = encode_planes(
            &board,
            &[],
            &FeatureConfig {
                perspective: FeaturePerspective::SideToMove,
                ..default_feature_config()
            },
        );

        assert_eq!(absolute.data_f32[0], 1.0);
        assert_eq!(absolute.data_f32[64 + 1], 1.0);
        assert_eq!(relative.data_f32[1], 1.0);
        assert_eq!(relative.data_f32[64], 1.0);
    }
}
