use crate::engine::{
    Board, BoardStatus, Move, apply_forced_pass, apply_move_unchecked, board_status, disc_count,
    final_margin_from_side_to_move, generate_legal_moves, legal_moves_to_vec,
};
use crate::search_eval_data::{FEATURE_CELL_COUNTS, FEATURE_TO_PATTERN, FEATURE_TO_SQUARES};
use std::sync::OnceLock;

const SCORE_MAX: i16 = 64;
const SCORE_INF: i16 = 127;
const N_PHASES: usize = 60;
const MAX_STONE_NUM: usize = 65;
const STEP: i32 = 32;
const STEP_2: i32 = 16;
const N_ZEROS_PLUS: i16 = 1 << 12;
const PATTERN_SIZES: [usize; 16] = [8, 9, 8, 9, 8, 9, 7, 10, 10, 10, 10, 10, 10, 10, 10, 10];
const POW3: [usize; 11] = [1, 3, 9, 27, 81, 243, 729, 2187, 6561, 19683, 59049];

static EVAL_TABLES: OnceLock<EvalTables> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchConfig {
    pub max_depth: Option<u8>,
    pub max_nodes: Option<u64>,
    pub time_limit_ms: Option<u64>,
    pub exact_solver_empty_threshold: Option<u8>,
    pub use_transposition_table: bool,
    pub multi_pv: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchResult {
    pub best_move: Option<Move>,
    pub best_score: i16,
    pub score_kind: ScoreKind,
    pub pv: Vec<Move>,
    pub searched_nodes: u64,
    pub reached_depth: u8,
    pub is_exact: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScoreKind {
    MarginFromSideToMove,
    MarginFromBlack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SolveConfig {
    pub exact_solver_empty_threshold: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolveResult {
    pub best_move: Option<Move>,
    pub exact_margin: i16,
    pub pv: Vec<Move>,
    pub searched_nodes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolveError {
    NotEligible,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExactLine {
    best_move: Option<Move>,
    exact_margin: i16,
    pv: Vec<Move>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SearchLine {
    best_move: Option<Move>,
    best_score: i16,
    pv: Vec<Move>,
    is_exact: bool,
}

struct SearchState {
    searched_nodes: u64,
    max_nodes: Option<u64>,
    exact_solver_empty_threshold: Option<u8>,
}

struct EvalTables {
    raw: Box<[i16]>,
    pattern_offsets: [usize; 16],
    pattern_phase_span: usize,
    phase_stride: usize,
}

pub fn can_solve_exact(board: &Board, config: &SolveConfig) -> bool {
    disc_count(board).empty <= config.exact_solver_empty_threshold
}

pub fn solve_exact(board: &Board, config: &SolveConfig) -> Result<SolveResult, SolveError> {
    if !can_solve_exact(board, config) {
        return Err(SolveError::NotEligible);
    }

    let mut searched_nodes = 0;
    let line = solve_exact_line(board, &mut searched_nodes);
    Ok(SolveResult {
        best_move: line.best_move,
        exact_margin: line.exact_margin,
        pv: line.pv,
        searched_nodes,
    })
}

pub fn search_best_move(board: &Board, config: &SearchConfig) -> SearchResult {
    let _ = (
        config.time_limit_ms,
        config.use_transposition_table,
        config.multi_pv,
    );

    if let Some(threshold) = config.exact_solver_empty_threshold {
        let exact_config = SolveConfig {
            exact_solver_empty_threshold: threshold,
        };
        if can_solve_exact(board, &exact_config) {
            let exact = solve_exact(board, &exact_config)
                .expect("exact_solver_empty_threshold eligibility was checked");
            return SearchResult {
                best_move: exact.best_move,
                best_score: exact.exact_margin,
                score_kind: ScoreKind::MarginFromSideToMove,
                pv: exact.pv,
                searched_nodes: exact.searched_nodes,
                reached_depth: disc_count(board).empty,
                is_exact: true,
            };
        }
    }

    let requested_depth = config.max_depth.unwrap_or(1);
    if requested_depth == 0 {
        return SearchResult {
            best_move: None,
            best_score: leaf_score(board),
            score_kind: ScoreKind::MarginFromSideToMove,
            pv: Vec::new(),
            searched_nodes: 0,
            reached_depth: 0,
            is_exact: matches!(board_status(board), BoardStatus::Terminal),
        };
    }

    let legal = generate_legal_moves(board);
    if legal.count == 0 {
        return match board_status(board) {
            BoardStatus::Terminal => SearchResult {
                best_move: None,
                best_score: final_margin_from_side_to_move(board) as i16,
                score_kind: ScoreKind::MarginFromSideToMove,
                pv: Vec::new(),
                searched_nodes: 0,
                reached_depth: 0,
                is_exact: true,
            },
            BoardStatus::ForcedPass => {
                let mut state = SearchState {
                    searched_nodes: 0,
                    max_nodes: config.max_nodes,
                    exact_solver_empty_threshold: config.exact_solver_empty_threshold,
                };
                let passed = apply_forced_pass(board).expect("forced pass must succeed");
                let line = nega_scout(
                    &passed,
                    requested_depth,
                    -SCORE_INF,
                    SCORE_INF,
                    true,
                    &mut state,
                );
                SearchResult {
                    best_move: None,
                    best_score: -line.best_score,
                    score_kind: ScoreKind::MarginFromSideToMove,
                    pv: line.pv,
                    searched_nodes: state.searched_nodes,
                    reached_depth: requested_depth,
                    is_exact: line.is_exact,
                }
            }
            BoardStatus::Ongoing => unreachable!("legal.count == 0 なら ongoing にはならない"),
        };
    }

    let mut state = SearchState {
        searched_nodes: 1,
        max_nodes: config.max_nodes,
        exact_solver_empty_threshold: config.exact_solver_empty_threshold,
    };
    let moves = legal_moves_to_vec(legal);
    let mut best_move = None;
    let mut best_score = i16::MIN;
    let mut best_pv = Vec::new();
    let mut alpha = -SCORE_INF;
    let beta = SCORE_INF;
    let mut best_exact = true;

    for (idx, mv) in moves.into_iter().enumerate() {
        if state.node_limit_reached() && best_move.is_some() {
            break;
        }
        let next = apply_move_unchecked(board, mv);
        let child = if idx == 0 {
            nega_scout(&next, requested_depth - 1, -beta, -alpha, false, &mut state)
        } else {
            let mut probe = nega_scout(
                &next,
                requested_depth - 1,
                -(alpha + 1),
                -alpha,
                false,
                &mut state,
            );
            let probe_score = -probe.best_score;
            if probe_score > alpha && probe_score < beta {
                probe = nega_scout(&next, requested_depth - 1, -beta, -alpha, false, &mut state);
            }
            probe
        };
        let score = -child.best_score;
        if score > best_score {
            best_move = Some(mv);
            best_score = score;
            best_exact = child.is_exact;
            best_pv.clear();
            best_pv.push(mv);
            best_pv.extend(child.pv);
        }
        alpha = alpha.max(score);
    }

    SearchResult {
        best_move,
        best_score,
        score_kind: ScoreKind::MarginFromSideToMove,
        pv: best_pv,
        searched_nodes: state.searched_nodes,
        reached_depth: requested_depth,
        is_exact: best_exact,
    }
}

fn solve_exact_line(board: &Board, searched_nodes: &mut u64) -> ExactLine {
    *searched_nodes += 1;

    let legal = generate_legal_moves(board);
    if legal.count == 0 {
        return match board_status(board) {
            BoardStatus::Terminal => ExactLine {
                best_move: None,
                exact_margin: final_margin_from_side_to_move(board) as i16,
                pv: Vec::new(),
            },
            BoardStatus::ForcedPass => {
                let passed = apply_forced_pass(board).expect("forced pass must succeed");
                let child = solve_exact_line(&passed, searched_nodes);
                ExactLine {
                    best_move: None,
                    exact_margin: -child.exact_margin,
                    pv: child.pv,
                }
            }
            BoardStatus::Ongoing => unreachable!("legal.count == 0 なら ongoing にはならない"),
        };
    }

    let mut best_move = None;
    let mut best_score = i16::MIN;
    let mut best_pv = Vec::new();

    for mv in legal_moves_to_vec(legal) {
        let next = apply_move_unchecked(board, mv);
        let child = solve_exact_line(&next, searched_nodes);
        let score = -child.exact_margin;
        if score > best_score {
            best_move = Some(mv);
            best_score = score;
            best_pv.clear();
            best_pv.push(mv);
            best_pv.extend(child.pv);
        }
    }

    ExactLine {
        best_move,
        exact_margin: best_score,
        pv: best_pv,
    }
}

fn nega_scout(
    board: &Board,
    depth: u8,
    mut alpha: i16,
    beta: i16,
    skipped: bool,
    state: &mut SearchState,
) -> SearchLine {
    state.searched_nodes += 1;

    if let Some(threshold) = state.exact_solver_empty_threshold {
        let exact_config = SolveConfig {
            exact_solver_empty_threshold: threshold,
        };
        if can_solve_exact(board, &exact_config) {
            let exact = solve_exact(board, &exact_config)
                .expect("exact_solver_empty_threshold eligibility was checked");
            state.searched_nodes += exact.searched_nodes.saturating_sub(1);
            return SearchLine {
                best_move: exact.best_move,
                best_score: exact.exact_margin,
                pv: exact.pv,
                is_exact: true,
            };
        }
    }

    if state.node_limit_reached() {
        return SearchLine {
            best_move: None,
            best_score: leaf_score(board),
            pv: Vec::new(),
            is_exact: false,
        };
    }

    let legal = generate_legal_moves(board);
    if legal.count == 0 {
        return match board_status(board) {
            BoardStatus::Terminal => SearchLine {
                best_move: None,
                best_score: final_margin_from_side_to_move(board) as i16,
                pv: Vec::new(),
                is_exact: true,
            },
            BoardStatus::ForcedPass => {
                if skipped {
                    SearchLine {
                        best_move: None,
                        best_score: final_margin_from_side_to_move(board) as i16,
                        pv: Vec::new(),
                        is_exact: true,
                    }
                } else {
                    let passed = apply_forced_pass(board).expect("forced pass must succeed");
                    let child = nega_scout(&passed, depth, -beta, -alpha, true, state);
                    SearchLine {
                        best_move: None,
                        best_score: -child.best_score,
                        pv: child.pv,
                        is_exact: child.is_exact,
                    }
                }
            }
            BoardStatus::Ongoing => unreachable!("legal.count == 0 なら ongoing にはならない"),
        };
    }

    if depth == 0 {
        return SearchLine {
            best_move: None,
            best_score: leaf_score(board),
            pv: Vec::new(),
            is_exact: false,
        };
    }

    let moves = legal_moves_to_vec(legal);
    let mut best_move = None;
    let mut best_score = i16::MIN;
    let mut best_pv = Vec::new();
    let mut best_exact = true;

    for (idx, mv) in moves.into_iter().enumerate() {
        let next = apply_move_unchecked(board, mv);
        let child = if idx == 0 {
            nega_scout(&next, depth - 1, -beta, -alpha, false, state)
        } else {
            let mut probe = nega_scout(&next, depth - 1, -(alpha + 1), -alpha, false, state);
            let probe_score = -probe.best_score;
            if probe_score > alpha && probe_score < beta {
                probe = nega_scout(&next, depth - 1, -beta, -alpha, false, state);
            }
            probe
        };
        let score = -child.best_score;
        if score > best_score {
            best_move = Some(mv);
            best_score = score;
            best_exact = child.is_exact;
            best_pv.clear();
            best_pv.push(mv);
            best_pv.extend(child.pv);
        }
        if score > alpha {
            alpha = score;
        }
        if alpha >= beta {
            break;
        }
        if state.node_limit_reached() {
            break;
        }
    }

    SearchLine {
        best_move,
        best_score,
        pv: best_pv,
        is_exact: best_exact,
    }
}

fn leaf_score(board: &Board) -> i16 {
    match board_status(board) {
        BoardStatus::Terminal => final_margin_from_side_to_move(board) as i16,
        BoardStatus::ForcedPass => {
            let passed = apply_forced_pass(board).expect("forced pass must succeed");
            -leaf_score(&passed)
        }
        BoardStatus::Ongoing => mid_evaluate_diff(board),
    }
}

fn mid_evaluate_diff(board: &Board) -> i16 {
    let (player_bits, opponent_bits) = oriented_bits(board);
    let tables = eval_tables();
    let n_discs = (player_bits | opponent_bits).count_ones() as usize;
    let phase_idx = (n_discs - 4).min(N_PHASES - 1);
    let player_count = player_bits.count_ones() as usize;
    let board_array = build_board_array(player_bits, opponent_bits);

    let mut res = 0i32;
    for feature_idx in 0..FEATURE_TO_SQUARES.len() {
        let pattern_idx = FEATURE_TO_PATTERN[feature_idx];
        let feature_value = pick_pattern_idx(
            &board_array,
            &FEATURE_TO_SQUARES[feature_idx],
            FEATURE_CELL_COUNTS[feature_idx] as usize,
        );
        res += tables.pattern_value(phase_idx, pattern_idx, feature_value) as i32;
    }
    res += tables.eval_num_value(phase_idx, player_count) as i32;
    res += if res >= 0 { STEP_2 } else { -STEP_2 };
    let normalized = (res / STEP).clamp(-(SCORE_MAX as i32), SCORE_MAX as i32);
    normalized as i16
}

fn oriented_bits(board: &Board) -> (u64, u64) {
    match board.side_to_move {
        crate::engine::Color::Black => (board.black_bits, board.white_bits),
        crate::engine::Color::White => (board.white_bits, board.black_bits),
    }
}

fn build_board_array(player_bits: u64, opponent_bits: u64) -> [u8; 64] {
    let mut board_array = [2u8; 64];
    for (square, cell) in board_array.iter_mut().enumerate() {
        let bit = 1u64 << square;
        *cell = if player_bits & bit != 0 {
            0
        } else if opponent_bits & bit != 0 {
            1
        } else {
            2
        };
    }
    board_array
}

fn pick_pattern_idx(board_array: &[u8; 64], squares: &[u8; 10], len: usize) -> usize {
    let mut index = 0usize;
    for &square in &squares[..len] {
        index *= 3;
        index += board_array[square as usize] as usize;
    }
    index
}

fn eval_tables() -> &'static EvalTables {
    EVAL_TABLES.get_or_init(EvalTables::load)
}

impl EvalTables {
    fn load() -> Self {
        let bytes = include_bytes!("../ref/Egaroucid/bin/resources/eval.egev2");
        let compressed_count = i32::from_le_bytes(
            bytes[0..4]
                .try_into()
                .expect("eval.egev2 header must contain entry count"),
        ) as usize;
        let compressed_bytes = &bytes[4..];
        assert_eq!(
            compressed_bytes.len(),
            compressed_count * 2,
            "eval.egev2 size does not match the compressed entry count"
        );

        let mut raw = Vec::with_capacity(expected_unzipped_len());
        for chunk in compressed_bytes.chunks_exact(2) {
            let value = i16::from_le_bytes([chunk[0], chunk[1]]);
            if value >= N_ZEROS_PLUS {
                raw.extend(std::iter::repeat_n(0i16, (value - N_ZEROS_PLUS) as usize));
            } else {
                raw.push(value);
            }
        }

        assert_eq!(
            raw.len(),
            expected_unzipped_len(),
            "eval.egev2 unzipped length mismatch"
        );

        let mut pattern_offsets = [0usize; 16];
        let mut cursor = 0usize;
        for (idx, size) in PATTERN_SIZES.iter().enumerate() {
            pattern_offsets[idx] = cursor;
            cursor += POW3[*size];
        }

        Self {
            raw: raw.into_boxed_slice(),
            pattern_offsets,
            pattern_phase_span: cursor,
            phase_stride: cursor + MAX_STONE_NUM,
        }
    }

    fn pattern_value(&self, phase_idx: usize, pattern_idx: usize, feature_value: usize) -> i16 {
        let index =
            phase_idx * self.phase_stride + self.pattern_offsets[pattern_idx] + feature_value;
        self.raw[index]
    }

    fn eval_num_value(&self, phase_idx: usize, player_count: usize) -> i16 {
        let index = phase_idx * self.phase_stride + self.pattern_phase_span + player_count;
        self.raw[index]
    }
}

impl SearchState {
    fn node_limit_reached(&self) -> bool {
        self.max_nodes
            .is_some_and(|limit| self.searched_nodes >= limit)
    }
}

fn expected_unzipped_len() -> usize {
    let pattern_phase_span: usize = PATTERN_SIZES.iter().map(|&size| POW3[size]).sum();
    N_PHASES * (pattern_phase_span + MAX_STONE_NUM)
}

#[cfg(test)]
mod tests {
    use super::{
        ScoreKind, SearchConfig, SearchResult, SolveConfig, SolveError, can_solve_exact,
        mid_evaluate_diff, search_best_move, solve_exact,
    };
    use crate::engine::{
        Board, BoardStatus, Color, Move, apply_forced_pass, apply_move_unchecked, board_status,
        final_margin_from_side_to_move, generate_legal_moves, legal_moves_to_vec,
    };
    use crate::random_play::{RandomPlayConfig, play_random_game};

    fn brute_force_exact(board: &Board) -> (Option<Move>, i16) {
        let legal = generate_legal_moves(board);
        if legal.count == 0 {
            return match board_status(board) {
                BoardStatus::Terminal => (None, final_margin_from_side_to_move(board) as i16),
                BoardStatus::ForcedPass => {
                    let passed = apply_forced_pass(board).expect("forced pass must succeed");
                    let (_, score) = brute_force_exact(&passed);
                    (None, -score)
                }
                BoardStatus::Ongoing => unreachable!("legal.count == 0 なら ongoing にはならない"),
            };
        }

        let mut best_move = None;
        let mut best_score = i16::MIN;
        for mv in legal_moves_to_vec(legal) {
            let next = apply_move_unchecked(board, mv);
            let (_, child_score) = brute_force_exact(&next);
            let score = -child_score;
            if score > best_score {
                best_move = Some(mv);
                best_score = score;
            }
        }
        (best_move, best_score)
    }

    fn brute_force_midgame(board: &Board, depth: u8) -> (Option<Move>, i16) {
        let legal = generate_legal_moves(board);
        if legal.count == 0 {
            return match board_status(board) {
                BoardStatus::Terminal => (None, final_margin_from_side_to_move(board) as i16),
                BoardStatus::ForcedPass => {
                    let passed = apply_forced_pass(board).expect("forced pass must succeed");
                    let (_, score) = brute_force_midgame(&passed, depth);
                    (None, -score)
                }
                BoardStatus::Ongoing => unreachable!("legal.count == 0 なら ongoing にはならない"),
            };
        }

        if depth == 0 {
            return (None, mid_evaluate_diff(board));
        }

        let mut best_move = None;
        let mut best_score = i16::MIN;
        for mv in legal_moves_to_vec(legal) {
            let next = apply_move_unchecked(board, mv);
            let (_, child_score) = brute_force_midgame(&next, depth - 1);
            let score = -child_score;
            if score > best_score {
                best_move = Some(mv);
                best_score = score;
            }
        }
        (best_move, best_score)
    }

    fn pick_multi_move_endgame_board() -> Board {
        for seed in 0..256 {
            let trace = play_random_game(
                seed,
                &RandomPlayConfig {
                    max_plies: Some(60),
                },
            );
            for board in trace.boards {
                let legal = generate_legal_moves(&board);
                let empty = board.empty_bits().count_ones() as u8;
                if empty <= 6 && legal.count >= 2 {
                    return board;
                }
            }
        }
        panic!("multi-move endgame board not found");
    }

    #[test]
    fn can_solve_exact_checks_empty_threshold() {
        let board = Board::new_initial();
        assert!(!can_solve_exact(
            &board,
            &SolveConfig {
                exact_solver_empty_threshold: 16,
            }
        ));
        assert!(can_solve_exact(
            &board,
            &SolveConfig {
                exact_solver_empty_threshold: 60,
            }
        ));
    }

    #[test]
    fn solve_exact_rejects_board_outside_threshold() {
        let board = Board::new_initial();
        assert_eq!(
            solve_exact(
                &board,
                &SolveConfig {
                    exact_solver_empty_threshold: 12,
                }
            ),
            Err(SolveError::NotEligible)
        );
    }

    #[test]
    fn solve_exact_returns_terminal_margin_without_move() {
        let board = Board::from_bits(u64::MAX, 0, Color::Black).expect("board must be valid");
        let result = solve_exact(
            &board,
            &SolveConfig {
                exact_solver_empty_threshold: 0,
            },
        )
        .expect("terminal board is exact-solvable");

        assert_eq!(result.best_move, None);
        assert_eq!(result.exact_margin, 64);
        assert!(result.pv.is_empty());
        assert!(result.searched_nodes >= 1);
    }

    #[test]
    fn solve_exact_handles_forced_pass_root() {
        let board = Board::from_bits(0xFFFF_FFFF_FFFF_FF7E, 0x0000_0000_0000_0080, Color::Black)
            .expect("board must be valid");
        let result = solve_exact(
            &board,
            &SolveConfig {
                exact_solver_empty_threshold: 1,
            },
        )
        .expect("forced-pass endgame is exact-solvable");

        assert_eq!(result.best_move, None);
        assert_eq!(result.exact_margin, 48);
        assert_eq!(result.pv, vec![Move { square: 0 }]);
    }

    #[test]
    fn solve_exact_matches_known_single_move_reply_after_pass() {
        let board = apply_forced_pass(
            &Board::from_bits(0xFFFF_FFFF_FFFF_FF7E, 0x0000_0000_0000_0080, Color::Black)
                .expect("board must be valid"),
        )
        .expect("forced pass must succeed");
        let result = solve_exact(
            &board,
            &SolveConfig {
                exact_solver_empty_threshold: 1,
            },
        )
        .expect("single-move endgame is exact-solvable");

        assert_eq!(result.best_move, Some(Move { square: 0 }));
        assert_eq!(result.exact_margin, -48);
        assert_eq!(result.pv, vec![Move { square: 0 }]);
    }

    #[test]
    fn solve_exact_matches_bruteforce_on_multi_move_endgame() {
        let board = pick_multi_move_endgame_board();
        let expected = brute_force_exact(&board);
        let result = solve_exact(
            &board,
            &SolveConfig {
                exact_solver_empty_threshold: 6,
            },
        )
        .expect("chosen endgame board is exact-solvable");

        assert_eq!(result.best_move, expected.0);
        assert_eq!(result.exact_margin, expected.1);
        if let Some(first) = result.best_move {
            assert_eq!(result.pv.first().copied(), Some(first));
        }
    }

    #[test]
    fn search_best_move_returns_exact_result_when_threshold_matches() {
        let board = pick_multi_move_endgame_board();
        let search = search_best_move(
            &board,
            &SearchConfig {
                max_depth: Some(2),
                max_nodes: None,
                time_limit_ms: None,
                exact_solver_empty_threshold: Some(6),
                use_transposition_table: false,
                multi_pv: 1,
            },
        );
        let exact = solve_exact(
            &board,
            &SolveConfig {
                exact_solver_empty_threshold: 6,
            },
        )
        .expect("exact search must succeed");

        assert_eq!(search.best_move, exact.best_move);
        assert_eq!(search.best_score, exact.exact_margin);
        assert_eq!(search.pv, exact.pv);
        assert_eq!(search.score_kind, ScoreKind::MarginFromSideToMove);
        assert!(search.is_exact);
    }

    #[test]
    fn search_best_move_matches_bruteforce_at_depth_one() {
        let board = Board::new_initial();
        let expected = brute_force_midgame(&board, 1);
        let result = search_best_move(
            &board,
            &SearchConfig {
                max_depth: Some(1),
                max_nodes: None,
                time_limit_ms: None,
                exact_solver_empty_threshold: None,
                use_transposition_table: false,
                multi_pv: 1,
            },
        );

        assert_eq!(result.best_move, expected.0);
        assert_eq!(result.best_score, expected.1);
        assert_eq!(result.score_kind, ScoreKind::MarginFromSideToMove);
        assert_eq!(result.reached_depth, 1);
        assert!(!result.is_exact);
        assert!(result.searched_nodes > 0);
    }

    #[test]
    fn search_best_move_matches_bruteforce_at_depth_two() {
        let board = Board::new_initial();
        let expected = brute_force_midgame(&board, 2);
        let result = search_best_move(
            &board,
            &SearchConfig {
                max_depth: Some(2),
                max_nodes: None,
                time_limit_ms: None,
                exact_solver_empty_threshold: None,
                use_transposition_table: false,
                multi_pv: 1,
            },
        );

        assert_eq!(result.best_move, expected.0);
        assert_eq!(result.best_score, expected.1);
        assert_eq!(result.reached_depth, 2);
    }

    #[test]
    fn search_best_move_handles_terminal_root() {
        let board = Board::from_bits(u64::MAX, 0, Color::Black).expect("board must be valid");
        let result = search_best_move(
            &board,
            &SearchConfig {
                max_depth: Some(4),
                max_nodes: None,
                time_limit_ms: None,
                exact_solver_empty_threshold: None,
                use_transposition_table: false,
                multi_pv: 1,
            },
        );

        assert_eq!(
            result,
            SearchResult {
                best_move: None,
                best_score: 64,
                score_kind: ScoreKind::MarginFromSideToMove,
                pv: Vec::new(),
                searched_nodes: 0,
                reached_depth: 0,
                is_exact: true,
            }
        );
    }
}
