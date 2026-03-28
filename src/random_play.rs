use crate::engine::{
    Board, BoardStatus, GameResult, Move, apply_forced_pass, apply_move, board_status,
    final_margin_from_black, game_result, generate_legal_moves, legal_moves_to_vec,
};
use crate::serialize::{PackedBoard, pack_board};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RandomPlayConfig {
    pub max_plies: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RandomGameTrace {
    pub boards: Vec<Board>,
    pub moves: Vec<Option<Move>>,
    pub final_result: GameResult,
    pub final_margin_from_black: i8,
    pub plies_played: u16,
    pub reached_terminal: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupervisedExample {
    pub board: Board,
    pub ply: u16,
    pub moves_until_here: Vec<Option<Move>>,
    pub final_result: GameResult,
    pub final_margin_from_black: i8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackedSupervisedExample {
    pub board: PackedBoard,
    pub ply: u16,
    pub moves_until_here: Vec<Option<u8>>,
    pub final_result: GameResult,
    pub final_margin_from_black: i8,
    pub policy_target_index: i8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PositionSamplingConfig {
    pub num_positions: u32,
    pub min_plies: u16,
    pub max_plies: u16,
}

#[derive(Clone, Copy, Debug)]
struct XorShift64Star {
    state: u64,
}

impl XorShift64Star {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn choose_index(&mut self, len: usize) -> usize {
        debug_assert!(len > 0);
        (self.next_u64() % len as u64) as usize
    }
}

fn play_random_game_from_board_with_rng(
    start_board: Board,
    rng: &mut XorShift64Star,
    config: &RandomPlayConfig,
) -> RandomGameTrace {
    let mut board = start_board;
    let mut boards = vec![board];
    let mut moves = Vec::new();
    let max_plies = config.max_plies.map(usize::from);

    loop {
        let status = board_status(&board);
        let should_record = max_plies.is_none_or(|limit| moves.len() < limit);

        match status {
            BoardStatus::Terminal => {
                let reached_terminal = boards
                    .last()
                    .is_some_and(|last_board| board_status(last_board) == BoardStatus::Terminal);
                let plies_played = moves.len() as u16;
                return RandomGameTrace {
                    boards,
                    moves,
                    final_result: game_result(&board),
                    final_margin_from_black: final_margin_from_black(&board),
                    plies_played,
                    reached_terminal,
                };
            }
            BoardStatus::ForcedPass => {
                board = apply_forced_pass(&board).expect("forced pass must succeed");
                if should_record {
                    moves.push(None);
                    boards.push(board);
                }
            }
            BoardStatus::Ongoing => {
                let legal_moves = legal_moves_to_vec(generate_legal_moves(&board));
                let mv = legal_moves[rng.choose_index(legal_moves.len())];
                board = apply_move(&board, mv).expect("chosen random move must be legal");
                if should_record {
                    moves.push(Some(mv));
                    boards.push(board);
                }
            }
        }
    }
}

pub fn play_random_game(seed: u64, config: &RandomPlayConfig) -> RandomGameTrace {
    let mut rng = XorShift64Star::new(seed);
    play_random_game_from_board_with_rng(Board::new_initial(), &mut rng, config)
}

pub fn sample_reachable_positions(seed: u64, config: &PositionSamplingConfig) -> Vec<Board> {
    if config.num_positions == 0 || config.min_plies > config.max_plies || config.min_plies > 60 {
        return Vec::new();
    }

    let mut rng = XorShift64Star::new(seed);
    let mut positions = Vec::with_capacity(config.num_positions as usize);
    let attempt_limit = usize::max(config.num_positions as usize * 128, 128);

    for _ in 0..attempt_limit {
        if positions.len() >= config.num_positions as usize {
            break;
        }

        let trace = play_random_game_from_board_with_rng(
            Board::new_initial(),
            &mut rng,
            &RandomPlayConfig {
                max_plies: Some(config.max_plies),
            },
        );

        let candidates: Vec<Board> = trace
            .boards
            .iter()
            .enumerate()
            .filter(|(ply, _)| {
                let ply = *ply as u16;
                config.min_plies <= ply && ply <= config.max_plies
            })
            .map(|(_, board)| *board)
            .collect();

        if !candidates.is_empty() {
            positions.push(candidates[rng.choose_index(candidates.len())]);
        }
    }

    positions
}

pub fn supervised_examples_from_trace(trace: &RandomGameTrace) -> Vec<SupervisedExample> {
    trace
        .boards
        .iter()
        .enumerate()
        .map(|(ply, board)| SupervisedExample {
            board: *board,
            ply: ply as u16,
            moves_until_here: trace.moves[..ply].to_vec(),
            final_result: trace.final_result,
            final_margin_from_black: trace.final_margin_from_black,
        })
        .collect()
}

pub fn supervised_examples_from_traces(traces: &[RandomGameTrace]) -> Vec<SupervisedExample> {
    let mut examples = Vec::new();
    for trace in traces {
        examples.extend(supervised_examples_from_trace(trace));
    }
    examples
}

fn policy_target_index(next_move: Option<Option<Move>>) -> i8 {
    match next_move {
        Some(Some(mv)) => mv.square as i8,
        Some(None) => 64,
        None => -1,
    }
}

pub fn packed_supervised_examples_from_trace(
    trace: &RandomGameTrace,
) -> Vec<PackedSupervisedExample> {
    trace
        .boards
        .iter()
        .enumerate()
        .map(|(ply, board)| PackedSupervisedExample {
            board: pack_board(board),
            ply: ply as u16,
            moves_until_here: trace.moves[..ply]
                .iter()
                .map(|mv| mv.map(|mv| mv.square))
                .collect(),
            final_result: trace.final_result,
            final_margin_from_black: trace.final_margin_from_black,
            policy_target_index: policy_target_index(trace.moves.get(ply).copied()),
        })
        .collect()
}

pub fn packed_supervised_examples_from_traces(
    traces: &[RandomGameTrace],
) -> Vec<PackedSupervisedExample> {
    let mut examples = Vec::new();
    for trace in traces {
        examples.extend(packed_supervised_examples_from_trace(trace));
    }
    examples
}

#[cfg(test)]
mod tests {
    use super::{
        PackedSupervisedExample, PositionSamplingConfig, RandomPlayConfig, SupervisedExample,
        XorShift64Star, packed_supervised_examples_from_trace,
        packed_supervised_examples_from_traces, play_random_game,
        play_random_game_from_board_with_rng, sample_reachable_positions,
        supervised_examples_from_trace, supervised_examples_from_traces,
    };
    use crate::{
        Board, BoardStatus, Color, PackedBoard, apply_forced_pass, apply_move, board_status,
        disc_count, is_legal_move,
    };

    #[test]
    fn random_generator_is_reproducible_for_same_seed() {
        let mut lhs = XorShift64Star::new(123);
        let mut rhs = XorShift64Star::new(123);

        for _ in 0..8 {
            assert_eq!(lhs.next_u64(), rhs.next_u64());
        }
    }

    #[test]
    fn play_random_game_is_reproducible_for_same_seed() {
        let config = RandomPlayConfig {
            max_plies: Some(12),
        };
        assert_eq!(
            play_random_game(123, &config),
            play_random_game(123, &config)
        );
    }

    #[test]
    fn play_random_game_trace_contains_only_legal_transitions() {
        let trace = play_random_game(
            7,
            &RandomPlayConfig {
                max_plies: Some(24),
            },
        );

        assert_eq!(trace.boards.len(), trace.moves.len() + 1);
        for (idx, mv) in trace.moves.iter().enumerate() {
            let board = trace.boards[idx];
            let next = trace.boards[idx + 1];
            match mv {
                Some(mv) => {
                    assert!(is_legal_move(&board, *mv));
                    assert_eq!(apply_move(&board, *mv), Ok(next));
                }
                None => {
                    assert_eq!(board_status(&board), BoardStatus::ForcedPass);
                    assert_eq!(apply_forced_pass(&board), Ok(next));
                }
            }
        }
    }

    #[test]
    fn play_random_game_records_forced_pass_as_none() {
        let board = Board::from_bits(0xFFFF_FFFF_FFFF_FF7E, 0x0000_0000_0000_0080, Color::Black)
            .expect("board must be valid");
        let mut rng = XorShift64Star::new(1);
        let trace = play_random_game_from_board_with_rng(
            board,
            &mut rng,
            &RandomPlayConfig { max_plies: Some(1) },
        );

        assert_eq!(trace.moves, vec![None]);
        assert_eq!(trace.boards.len(), 2);
        assert_eq!(trace.boards[0], board);
        assert_eq!(trace.boards[1].side_to_move, Color::White);
    }

    #[test]
    fn play_random_game_respects_max_plies_and_keeps_terminal_label() {
        let full = play_random_game(99, &RandomPlayConfig { max_plies: None });
        let truncated = play_random_game(99, &RandomPlayConfig { max_plies: Some(8) });

        assert_eq!(truncated.plies_played, 8);
        assert_eq!(truncated.moves.len(), 8);
        assert_eq!(truncated.boards.len(), 9);
        assert!(!truncated.reached_terminal);
        assert_eq!(truncated.final_result, full.final_result);
        assert_eq!(
            truncated.final_margin_from_black,
            full.final_margin_from_black
        );
    }

    #[test]
    fn sample_reachable_positions_returns_valid_boards_in_requested_range() {
        let config = PositionSamplingConfig {
            num_positions: 8,
            min_plies: 6,
            max_plies: 12,
        };
        let positions = sample_reachable_positions(5, &config);

        assert_eq!(positions.len(), 8);
        for board in positions {
            assert!(board.validate().is_ok());
            let counts = disc_count(&board);
            let plies = u16::from(counts.black + counts.white) - 4;
            assert!(config.min_plies <= plies && plies <= config.max_plies);
        }
    }

    #[test]
    fn supervised_examples_from_trace_keeps_prefix_moves_and_labels() {
        let trace = play_random_game(17, &RandomPlayConfig { max_plies: Some(6) });
        let examples = supervised_examples_from_trace(&trace);

        assert_eq!(examples.len(), trace.boards.len());
        for (ply, example) in examples.iter().enumerate() {
            assert_eq!(
                example,
                &SupervisedExample {
                    board: trace.boards[ply],
                    ply: ply as u16,
                    moves_until_here: trace.moves[..ply].to_vec(),
                    final_result: trace.final_result,
                    final_margin_from_black: trace.final_margin_from_black,
                }
            );
        }
    }

    #[test]
    fn supervised_examples_from_traces_concatenates_trace_examples() {
        let first = play_random_game(1, &RandomPlayConfig { max_plies: Some(3) });
        let second = play_random_game(2, &RandomPlayConfig { max_plies: Some(2) });
        let examples = supervised_examples_from_traces(&[first.clone(), second.clone()]);

        assert_eq!(
            examples,
            [
                supervised_examples_from_trace(&first),
                supervised_examples_from_trace(&second),
            ]
            .concat()
        );
    }

    #[test]
    fn packed_supervised_examples_from_trace_contains_policy_and_value_labels() {
        let trace = play_random_game(17, &RandomPlayConfig { max_plies: Some(6) });
        let packed = packed_supervised_examples_from_trace(&trace);

        assert_eq!(packed.len(), trace.boards.len());
        for (ply, example) in packed.iter().enumerate() {
            let expected_policy_target_index = match trace.moves.get(ply).copied() {
                Some(Some(mv)) => mv.square as i8,
                Some(None) => 64,
                None => -1,
            };
            assert_eq!(
                example,
                &PackedSupervisedExample {
                    board: PackedBoard {
                        black_bits: trace.boards[ply].black_bits,
                        white_bits: trace.boards[ply].white_bits,
                        side_to_move: trace.boards[ply].side_to_move,
                    },
                    ply: ply as u16,
                    moves_until_here: trace.moves[..ply]
                        .iter()
                        .map(|mv| mv.map(|mv| mv.square))
                        .collect(),
                    final_result: trace.final_result,
                    final_margin_from_black: trace.final_margin_from_black,
                    policy_target_index: expected_policy_target_index,
                }
            );
        }
    }

    #[test]
    fn packed_supervised_examples_from_traces_concatenates_trace_examples() {
        let first = play_random_game(1, &RandomPlayConfig { max_plies: Some(3) });
        let second = play_random_game(2, &RandomPlayConfig { max_plies: Some(2) });
        let packed = packed_supervised_examples_from_traces(&[first.clone(), second.clone()]);

        assert_eq!(
            packed,
            [
                packed_supervised_examples_from_trace(&first),
                packed_supervised_examples_from_trace(&second),
            ]
            .concat()
        );
    }
}
