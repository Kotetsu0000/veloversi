use crate::engine::{
    Board, BoardStatus, Move, apply_forced_pass, apply_move_unchecked, board_status, disc_count,
    final_margin_from_side_to_move, generate_legal_moves, legal_moves_to_vec,
};
use rustc_hash::FxHashMap;
use std::sync::atomic::{AtomicI16, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const SCORE_MAX: i16 = 64;
const SCORE_INF: i16 = 127;
const EXACT_TT_MIN_EMPTY: u8 = 12;
const EXACT_DEADLINE_CHECK_INTERVAL: u64 = 1024;
const NOT_FILE_A: u64 = 0xFEFE_FEFE_FEFE_FEFE;
const NOT_FILE_H: u64 = 0x7F7F_7F7F_7F7F_7F7F;
const CORNER_MASK: u64 = 0x8100_0000_0000_0081;
const EDGE_MASK: u64 = 0x7E81_8181_8181_817E;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExactSearchFailureReason {
    Timeout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExactSearchFailure {
    pub reason: ExactSearchFailureReason,
    pub searched_nodes: u64,
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
    completed: bool,
}

struct SearchState {
    searched_nodes: u64,
    max_nodes: Option<u64>,
    exact_solver_empty_threshold: Option<u8>,
    transposition_table: Option<TranspositionTable>,
    deadline: Option<Instant>,
}

struct ExactSearchState {
    searched_nodes: u64,
    transposition_table: TranspositionTable,
    deadline: Option<Instant>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct BoardKey {
    black_bits: u64,
    white_bits: u64,
    black_to_move: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BoundKind {
    Exact,
    Lower,
    Upper,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TranspositionEntry {
    depth: u8,
    bound: BoundKind,
    score: i16,
    best_move: Option<Move>,
    is_exact: bool,
}

#[derive(Default)]
struct TranspositionTable {
    entries: FxHashMap<BoardKey, TranspositionEntry>,
}

#[derive(Clone, Copy)]
struct OrderedMove {
    mv: Move,
    next: Board,
    is_immediate_win: bool,
}

#[derive(Clone)]
struct RootCandidate {
    line: SearchLine,
}

pub fn can_solve_exact(board: &Board, config: &SolveConfig) -> bool {
    disc_count(board).empty <= config.exact_solver_empty_threshold
}

pub fn solve_exact(board: &Board, config: &SolveConfig) -> Result<SolveResult, SolveError> {
    if !can_solve_exact(board, config) {
        return Err(SolveError::NotEligible);
    }

    let mut state = ExactSearchState::new(None);
    let line = solve_exact_line(board, -SCORE_INF, SCORE_INF, &mut state)
        .expect("solve_exact must not time out");
    Ok(SolveResult {
        best_move: line.best_move,
        exact_margin: line.exact_margin,
        pv: line.pv,
        searched_nodes: state.searched_nodes,
    })
}

pub fn search_best_move_exact(
    board: &Board,
    time_limit: Duration,
) -> Result<SolveResult, ExactSearchFailure> {
    let deadline = Instant::now() + time_limit;
    exact_search_best_move_parallel(board, deadline).map_err(|reason| ExactSearchFailure {
        reason: ExactSearchFailureReason::Timeout,
        searched_nodes: reason,
    })
}

pub fn search_best_move(board: &Board, config: &SearchConfig) -> SearchResult {
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

    let requested_depth = config.max_depth.unwrap_or(disc_count(board).empty.max(1));
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
                    transposition_table: config
                        .use_transposition_table
                        .then(TranspositionTable::default),
                    deadline: deadline_from_config(config),
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
                    reached_depth: if line.completed { requested_depth } else { 0 },
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
        transposition_table: config
            .use_transposition_table
            .then(TranspositionTable::default),
        deadline: deadline_from_config(config),
    };
    let multi_pv = config.multi_pv.max(1);

    if config.time_limit_ms.is_some() {
        let mut last_completed = None;
        let mut last_partial = None;
        for depth in 1..=requested_depth {
            let line = search_root(board, legal, depth, multi_pv, &mut state);
            if line.completed {
                last_completed = Some((depth, line.clone()));
                last_partial = Some((depth, line));
                if state.time_limit_reached() {
                    break;
                }
            } else {
                if last_partial.is_none() {
                    last_partial = Some((depth, line));
                }
                break;
            }
        }
        let (reached_depth, line) = last_completed
            .or(last_partial)
            .expect("root legal moves guarantee at least one result");
        SearchResult {
            best_move: line.best_move,
            best_score: line.best_score,
            score_kind: ScoreKind::MarginFromSideToMove,
            pv: line.pv,
            searched_nodes: state.searched_nodes,
            reached_depth,
            is_exact: line.is_exact,
        }
    } else {
        let line = search_root(board, legal, requested_depth, multi_pv, &mut state);
        SearchResult {
            best_move: line.best_move,
            best_score: line.best_score,
            score_kind: ScoreKind::MarginFromSideToMove,
            pv: line.pv,
            searched_nodes: state.searched_nodes,
            reached_depth: if line.completed { requested_depth } else { 0 },
            is_exact: line.is_exact,
        }
    }
}

fn solve_exact_line(
    board: &Board,
    mut alpha: i16,
    beta: i16,
    state: &mut ExactSearchState,
) -> Result<ExactLine, ExactSearchFailureReason> {
    state.searched_nodes += 1;

    if state.time_limit_reached() {
        return Err(ExactSearchFailureReason::Timeout);
    }

    let depth = disc_count(board).empty;
    let use_tt = depth >= EXACT_TT_MIN_EMPTY;
    let original_alpha = alpha;
    let mut beta_bound = beta;
    let key = BoardKey::new(board);
    let mut tt_move = None;
    if use_tt && let Some(entry) = state.transposition_table.lookup(key, depth) {
        tt_move = entry.best_move;
        match entry.bound {
            BoundKind::Exact => {
                return Ok(ExactLine {
                    best_move: entry.best_move,
                    exact_margin: entry.score,
                    pv: entry.best_move.into_iter().collect(),
                });
            }
            BoundKind::Lower => alpha = alpha.max(entry.score),
            BoundKind::Upper => beta_bound = beta_bound.min(entry.score),
        }
        if alpha >= beta_bound {
            return Ok(ExactLine {
                best_move: entry.best_move,
                exact_margin: entry.score,
                pv: entry.best_move.into_iter().collect(),
            });
        }
    }

    let legal = generate_legal_moves(board);
    if legal.count == 0 {
        let line = match board_status(board) {
            BoardStatus::Terminal => ExactLine {
                best_move: None,
                exact_margin: final_margin_from_side_to_move(board) as i16,
                pv: Vec::new(),
            },
            BoardStatus::ForcedPass => {
                let passed = apply_forced_pass(board).expect("forced pass must succeed");
                let child = solve_exact_line(&passed, -beta_bound, -alpha, state)?;
                ExactLine {
                    best_move: None,
                    exact_margin: -child.exact_margin,
                    pv: child.pv,
                }
            }
            BoardStatus::Ongoing => unreachable!("legal.count == 0 なら ongoing にはならない"),
        };
        return Ok(line);
    }

    let mut best_move = None;
    let mut best_score = i16::MIN;
    let mut best_pv = Vec::new();

    for ordered in ordered_moves(board, legal, tt_move) {
        if state.time_limit_reached() {
            return Err(ExactSearchFailureReason::Timeout);
        }
        let child = if ordered.is_immediate_win {
            ExactLine {
                best_move: None,
                exact_margin: -SCORE_MAX,
                pv: Vec::new(),
            }
        } else {
            solve_exact_line(&ordered.next, -beta_bound, -alpha, state)?
        };
        let score = -child.exact_margin;
        if score > best_score {
            best_move = Some(ordered.mv);
            best_score = score;
            best_pv.clear();
            best_pv.push(ordered.mv);
            best_pv.extend(child.pv);
        }
        alpha = alpha.max(score);
        if alpha >= beta_bound {
            break;
        }
    }

    let line = ExactLine {
        best_move,
        exact_margin: best_score,
        pv: best_pv,
    };
    if use_tt {
        state.transposition_table.store(
            board,
            TranspositionEntry {
                depth,
                bound: determine_bound(line.exact_margin, original_alpha, beta),
                score: line.exact_margin,
                best_move: line.best_move,
                is_exact: true,
            },
        );
    }
    Ok(line)
}

fn exact_search_best_move_parallel(board: &Board, deadline: Instant) -> Result<SolveResult, u64> {
    let legal = generate_legal_moves(board);
    if legal.count == 0 {
        let mut state = ExactSearchState::new(Some(deadline));
        let line = solve_exact_line(board, -SCORE_INF, SCORE_INF, &mut state)
            .map_err(|_| state.searched_nodes)?;
        return Ok(SolveResult {
            best_move: line.best_move,
            exact_margin: line.exact_margin,
            pv: line.pv,
            searched_nodes: state.searched_nodes,
        });
    }

    let ordered = ordered_moves(board, legal, None);
    let empty = disc_count(board).empty;
    let parallelism = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    if parallelism <= 1 || ordered.len() <= 1 || empty < 18 || ordered.len() < 4 {
        let mut state = ExactSearchState::new(Some(deadline));
        let line = solve_exact_line(board, -SCORE_INF, SCORE_INF, &mut state)
            .map_err(|_| state.searched_nodes)?;
        return Ok(SolveResult {
            best_move: line.best_move,
            exact_margin: line.exact_margin,
            pv: line.pv,
            searched_nodes: state.searched_nodes,
        });
    }

    let serial_prefix_len = ordered.len().min(3);
    let mut best_idx = usize::MAX;
    let mut best_move = Move { square: 0 };
    let mut best_score = -SCORE_INF;
    let mut best_pv = Vec::new();
    let mut total_nodes = 1u64;

    for (idx, candidate) in ordered.iter().copied().enumerate().take(serial_prefix_len) {
        if Instant::now() >= deadline {
            return Err(total_nodes);
        }
        let (score, pv, nodes) = if candidate.is_immediate_win {
            (SCORE_MAX, vec![candidate.mv], 1u64)
        } else {
            let mut state = ExactSearchState::new(Some(deadline));
            let child = solve_exact_line(&candidate.next, -SCORE_INF, -best_score, &mut state)
                .map_err(|_| total_nodes + state.searched_nodes)?;
            let score = -child.exact_margin;
            let mut pv = Vec::with_capacity(child.pv.len() + 1);
            pv.push(candidate.mv);
            pv.extend(child.pv);
            (score, pv, state.searched_nodes)
        };
        total_nodes += nodes;
        if score > best_score || (score == best_score && idx < best_idx) {
            best_idx = idx;
            best_move = candidate.mv;
            best_score = score;
            best_pv = pv;
        }
        if best_score >= SCORE_MAX {
            return Ok(SolveResult {
                best_move: Some(best_move),
                exact_margin: best_score,
                pv: best_pv,
                searched_nodes: total_nodes,
            });
        }
    }

    let shared_alpha = AtomicI16::new(best_score);
    let mut results = Vec::with_capacity(ordered.len());
    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(ordered.len().saturating_sub(serial_prefix_len));
        for (idx, candidate) in ordered.iter().copied().enumerate().skip(serial_prefix_len) {
            let shared_alpha_ref = &shared_alpha;
            handles.push(scope.spawn(move || {
                if Instant::now() >= deadline {
                    return Err(0u64);
                }
                if candidate.is_immediate_win {
                    return Ok((idx, candidate.mv, SCORE_MAX, vec![candidate.mv], 1u64));
                }
                let alpha = shared_alpha_ref.load(Ordering::Relaxed);
                let mut state = ExactSearchState::new(Some(deadline));
                let child = solve_exact_line(&candidate.next, -SCORE_INF, -alpha, &mut state)
                    .map_err(|_| state.searched_nodes)?;
                let score = -child.exact_margin;
                loop {
                    let observed = shared_alpha_ref.load(Ordering::Relaxed);
                    if score <= observed {
                        break;
                    }
                    if shared_alpha_ref
                        .compare_exchange(observed, score, Ordering::Relaxed, Ordering::Relaxed)
                        .is_ok()
                    {
                        break;
                    }
                }
                let mut pv = Vec::with_capacity(child.pv.len() + 1);
                pv.push(candidate.mv);
                pv.extend(child.pv);
                Ok((idx, candidate.mv, score, pv, state.searched_nodes))
            }));
        }
        for handle in handles {
            results.push(handle.join().expect("exact worker must not panic"));
        }
    });

    for result in results {
        match result {
            Ok((idx, mv, score, pv, nodes)) => {
                total_nodes += nodes;
                if score > best_score || (score == best_score && idx < best_idx) {
                    best_idx = idx;
                    best_move = mv;
                    best_score = score;
                    best_pv = pv;
                }
            }
            Err(nodes) => {
                total_nodes += nodes;
                return Err(total_nodes);
            }
        }
    }

    Ok(SolveResult {
        best_move: Some(best_move),
        exact_margin: best_score,
        pv: best_pv,
        searched_nodes: total_nodes,
    })
}

fn search_root(
    board: &Board,
    legal: crate::engine::LegalMoves,
    depth: u8,
    multi_pv: u8,
    state: &mut SearchState,
) -> SearchLine {
    let mut tt_move = None;
    if let Some(table) = state.transposition_table.as_ref() {
        tt_move = table.best_move_for(board);
    }
    let moves = ordered_moves(board, legal, tt_move);
    let mut best_move = None;
    let mut best_score = i16::MIN;
    let mut best_pv = Vec::new();
    let mut alpha = -SCORE_INF;
    let beta = SCORE_INF;
    let original_alpha = alpha;
    let mut best_exact = true;
    let mut completed = true;
    let mut root_candidates = Vec::new();

    for (idx, ordered) in moves.into_iter().enumerate() {
        if state.should_stop() && best_move.is_some() {
            completed = false;
            break;
        }
        let child = if ordered.is_immediate_win {
            SearchLine {
                best_move: None,
                best_score: -SCORE_MAX,
                pv: Vec::new(),
                is_exact: true,
                completed: true,
            }
        } else {
            let remaining_depth = depth.saturating_sub(1);
            if idx == 0 {
                nega_scout(&ordered.next, remaining_depth, -beta, -alpha, false, state)
            } else {
                let mut probe = nega_scout(
                    &ordered.next,
                    remaining_depth,
                    -(alpha + 1),
                    -alpha,
                    false,
                    state,
                );
                let probe_score = -probe.best_score;
                if probe.completed && probe_score > alpha && probe_score < beta {
                    probe = nega_scout(&ordered.next, remaining_depth, -beta, -alpha, false, state);
                }
                probe
            }
        };

        let score = -child.best_score;
        let mut candidate_pv = Vec::with_capacity(child.pv.len() + 1);
        candidate_pv.push(ordered.mv);
        candidate_pv.extend(child.pv.clone());
        update_root_candidates(
            &mut root_candidates,
            RootCandidate {
                line: SearchLine {
                    best_move: Some(ordered.mv),
                    best_score: score,
                    pv: candidate_pv.clone(),
                    is_exact: child.is_exact,
                    completed: child.completed,
                },
            },
            multi_pv,
        );

        if (child.completed || best_move.is_none()) && score > best_score {
            best_move = Some(ordered.mv);
            best_score = score;
            best_exact = child.is_exact;
            best_pv = candidate_pv;
        }
        completed &= child.completed;
        alpha = alpha.max(score);
        if alpha >= beta || state.should_stop() {
            if state.should_stop() {
                completed = false;
            }
            break;
        }
    }

    if let Some(primary) = root_candidates.first() {
        best_move = primary.line.best_move;
        best_score = primary.line.best_score;
        best_exact = primary.line.is_exact;
        best_pv = primary.line.pv.clone();
    }

    if completed && let Some(table) = state.transposition_table.as_mut() {
        let bound = determine_bound(best_score, original_alpha, beta);
        table.store(
            board,
            TranspositionEntry {
                depth,
                bound,
                score: best_score,
                best_move,
                is_exact: best_exact,
            },
        );
    }

    SearchLine {
        best_move,
        best_score,
        pv: best_pv,
        is_exact: best_exact,
        completed,
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
                completed: true,
            };
        }
    }

    if state.should_stop() {
        return SearchLine {
            best_move: None,
            best_score: leaf_score(board),
            pv: Vec::new(),
            is_exact: false,
            completed: false,
        };
    }

    let original_alpha = alpha;
    let original_beta = beta;
    let mut beta_bound = beta;
    let tt_key = BoardKey::new(board);
    let mut tt_move = None;
    if let Some(table) = state.transposition_table.as_ref()
        && let Some(entry) = table.lookup(tt_key, depth)
    {
        tt_move = entry.best_move;
        match entry.bound {
            BoundKind::Exact => {
                return SearchLine {
                    best_move: entry.best_move,
                    best_score: entry.score,
                    pv: entry.best_move.into_iter().collect(),
                    is_exact: entry.is_exact,
                    completed: true,
                };
            }
            BoundKind::Lower => alpha = alpha.max(entry.score),
            BoundKind::Upper => beta_bound = beta_bound.min(entry.score),
        }
        if alpha >= beta_bound {
            return SearchLine {
                best_move: entry.best_move,
                best_score: entry.score,
                pv: entry.best_move.into_iter().collect(),
                is_exact: entry.is_exact,
                completed: true,
            };
        }
    }

    let legal = generate_legal_moves(board);
    if legal.count == 0 {
        return match board_status(board) {
            BoardStatus::Terminal => SearchLine {
                best_move: None,
                best_score: final_margin_from_side_to_move(board) as i16,
                pv: Vec::new(),
                is_exact: true,
                completed: true,
            },
            BoardStatus::ForcedPass => {
                if skipped {
                    SearchLine {
                        best_move: None,
                        best_score: final_margin_from_side_to_move(board) as i16,
                        pv: Vec::new(),
                        is_exact: true,
                        completed: true,
                    }
                } else {
                    let passed = apply_forced_pass(board).expect("forced pass must succeed");
                    let child = nega_scout(&passed, depth, -beta, -alpha, true, state);
                    SearchLine {
                        best_move: None,
                        best_score: -child.best_score,
                        pv: child.pv,
                        is_exact: child.is_exact,
                        completed: child.completed,
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
            completed: true,
        };
    }

    let moves = ordered_moves(board, legal, tt_move);
    let mut best_move = None;
    let mut best_score = i16::MIN;
    let mut best_pv = Vec::new();
    let mut best_exact = true;
    let mut completed = true;

    for (idx, ordered) in moves.into_iter().enumerate() {
        let child = if ordered.is_immediate_win {
            SearchLine {
                best_move: None,
                best_score: -SCORE_MAX,
                pv: Vec::new(),
                is_exact: true,
                completed: true,
            }
        } else if idx == 0 {
            nega_scout(&ordered.next, depth - 1, -beta_bound, -alpha, false, state)
        } else {
            let mut probe =
                nega_scout(&ordered.next, depth - 1, -(alpha + 1), -alpha, false, state);
            let probe_score = -probe.best_score;
            if probe_score > alpha && probe_score < beta_bound {
                probe = nega_scout(&ordered.next, depth - 1, -beta_bound, -alpha, false, state);
            }
            probe
        };
        let score = -child.best_score;
        if score > best_score {
            best_move = Some(ordered.mv);
            best_score = score;
            best_exact = child.is_exact;
            best_pv.clear();
            best_pv.push(ordered.mv);
            best_pv.extend(child.pv);
        }
        completed &= child.completed;
        if score > alpha {
            alpha = score;
        }
        if alpha >= beta_bound || !child.completed {
            break;
        }
        if state.node_limit_reached() {
            break;
        }
    }

    let line = SearchLine {
        best_move,
        best_score,
        pv: best_pv,
        is_exact: best_exact,
        completed,
    };
    if line.completed
        && let Some(table) = state.transposition_table.as_mut()
    {
        table.store(
            board,
            TranspositionEntry {
                depth,
                bound: determine_bound(line.best_score, original_alpha, original_beta),
                score: line.best_score,
                best_move: line.best_move,
                is_exact: line.is_exact,
            },
        );
    }
    line
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

fn deadline_from_config(config: &SearchConfig) -> Option<Instant> {
    config
        .time_limit_ms
        .map(|limit_ms| Instant::now() + Duration::from_millis(limit_ms))
}

fn update_root_candidates(
    candidates: &mut Vec<RootCandidate>,
    candidate: RootCandidate,
    multi_pv: u8,
) {
    candidates.push(candidate);
    candidates.sort_by(|left, right| {
        right
            .line
            .best_score
            .cmp(&left.line.best_score)
            .then_with(|| left.line.pv.len().cmp(&right.line.pv.len()))
    });
    candidates.truncate(multi_pv as usize);
}

fn ordered_moves(
    board: &Board,
    legal: crate::engine::LegalMoves,
    tt_move: Option<Move>,
) -> Vec<OrderedMove> {
    let mut ordered = Vec::with_capacity(legal.count as usize);
    for mv in legal_moves_to_vec(legal) {
        let next = apply_move_unchecked(board, mv);
        let is_immediate_win = matches!(board_status(&next), BoardStatus::Terminal)
            && -final_margin_from_side_to_move(&next) == SCORE_MAX as i8;
        ordered.push(OrderedMove {
            mv,
            next,
            is_immediate_win,
        });
    }
    ordered.sort_by_key(|candidate| {
        let is_tt = tt_move == Some(candidate.mv);
        (
            !is_immediate_win_priority(candidate, is_tt),
            !is_tt,
            candidate.mv.square,
        )
    });
    ordered
}

fn is_immediate_win_priority(candidate: &OrderedMove, is_tt_move: bool) -> bool {
    candidate.is_immediate_win || is_tt_move
}

fn determine_bound(score: i16, alpha: i16, beta: i16) -> BoundKind {
    if score <= alpha {
        BoundKind::Upper
    } else if score >= beta {
        BoundKind::Lower
    } else {
        BoundKind::Exact
    }
}

fn mid_evaluate_diff(board: &Board) -> i16 {
    let (player_bits, opponent_bits) = oriented_bits(board);
    let empty_bits = !(player_bits | opponent_bits);
    let empty_count = empty_bits.count_ones() as u8;

    let disc_diff = player_bits.count_ones() as i32 - opponent_bits.count_ones() as i32;
    let mobility_diff = generate_legal_moves(board).count as i32
        - generate_legal_moves(&opponent_board(board)).count as i32;
    let potential_mobility_diff =
        potential_mobility(opponent_bits, empty_bits) - potential_mobility(player_bits, empty_bits);
    let frontier_diff =
        frontier_count(opponent_bits, empty_bits) - frontier_count(player_bits, empty_bits);
    let corner_diff = (player_bits & CORNER_MASK).count_ones() as i32
        - (opponent_bits & CORNER_MASK).count_ones() as i32;
    let edge_diff = (player_bits & EDGE_MASK).count_ones() as i32
        - (opponent_bits & EDGE_MASK).count_ones() as i32;
    let corner_closeness_diff = corner_closeness_penalty(player_bits, opponent_bits);
    let parity_term = if empty_count.is_multiple_of(2) { -1 } else { 1 };

    let disc_weight = match empty_count {
        41..=60 => 0,
        21..=40 => 2,
        _ => 6,
    };
    let mobility_weight = match empty_count {
        41..=60 => 10,
        21..=40 => 7,
        _ => 4,
    };
    let potential_weight = match empty_count {
        41..=60 => 6,
        21..=40 => 4,
        _ => 2,
    };
    let frontier_weight = match empty_count {
        41..=60 => 6,
        21..=40 => 5,
        _ => 3,
    };

    let raw = 24 * corner_diff
        + 3 * edge_diff
        + mobility_weight * mobility_diff
        + potential_weight * potential_mobility_diff
        + frontier_weight * frontier_diff
        + disc_weight * disc_diff
        + 8 * corner_closeness_diff
        + 2 * parity_term;

    (raw / 8).clamp(-(SCORE_MAX as i32), SCORE_MAX as i32) as i16
}

fn oriented_bits(board: &Board) -> (u64, u64) {
    match board.side_to_move {
        crate::engine::Color::Black => (board.black_bits, board.white_bits),
        crate::engine::Color::White => (board.white_bits, board.black_bits),
    }
}

fn opponent_board(board: &Board) -> Board {
    Board {
        black_bits: board.black_bits,
        white_bits: board.white_bits,
        side_to_move: match board.side_to_move {
            crate::engine::Color::Black => crate::engine::Color::White,
            crate::engine::Color::White => crate::engine::Color::Black,
        },
    }
}

fn potential_mobility(bits: u64, empty_bits: u64) -> i32 {
    (neighbor_mask(bits) & empty_bits).count_ones() as i32
}

fn frontier_count(bits: u64, empty_bits: u64) -> i32 {
    (bits & neighbor_mask(empty_bits)).count_ones() as i32
}

fn corner_closeness_penalty(player_bits: u64, opponent_bits: u64) -> i32 {
    let corners = [
        (0u64, (1u64 << 1) | (1u64 << 8), 1u64 << 9),
        (1u64 << 7, (1u64 << 6) | (1u64 << 15), 1u64 << 14),
        (1u64 << 56, (1u64 << 48) | (1u64 << 57), 1u64 << 49),
        (1u64 << 63, (1u64 << 55) | (1u64 << 62), 1u64 << 54),
    ];
    let mut diff = 0i32;
    for (corner, c_mask, x_mask) in corners {
        if (player_bits | opponent_bits) & corner == 0 {
            diff += (opponent_bits & c_mask).count_ones() as i32;
            diff -= (player_bits & c_mask).count_ones() as i32;
            diff += 2 * (opponent_bits & x_mask).count_ones() as i32;
            diff -= 2 * (player_bits & x_mask).count_ones() as i32;
        }
    }
    diff
}

fn neighbor_mask(bits: u64) -> u64 {
    ((bits & NOT_FILE_H) << 1)
        | ((bits & NOT_FILE_A) >> 1)
        | (bits << 8)
        | (bits >> 8)
        | ((bits & NOT_FILE_H) << 9)
        | ((bits & NOT_FILE_H) >> 7)
        | ((bits & NOT_FILE_A) << 7)
        | ((bits & NOT_FILE_A) >> 9)
}

impl BoardKey {
    fn new(board: &Board) -> Self {
        Self {
            black_bits: board.black_bits,
            white_bits: board.white_bits,
            black_to_move: matches!(board.side_to_move, crate::engine::Color::Black),
        }
    }
}

impl TranspositionTable {
    fn lookup(&self, key: BoardKey, depth: u8) -> Option<TranspositionEntry> {
        self.entries
            .get(&key)
            .copied()
            .filter(|entry| entry.depth >= depth)
    }

    fn best_move_for(&self, board: &Board) -> Option<Move> {
        self.entries
            .get(&BoardKey::new(board))
            .and_then(|entry| entry.best_move)
    }

    fn store(&mut self, board: &Board, entry: TranspositionEntry) {
        let key = BoardKey::new(board);
        match self.entries.get(&key).copied() {
            Some(existing)
                if existing.depth > entry.depth
                    || (existing.depth == entry.depth && existing.bound == BoundKind::Exact) => {}
            _ => {
                self.entries.insert(key, entry);
            }
        }
    }
}

impl SearchState {
    fn node_limit_reached(&self) -> bool {
        self.max_nodes
            .is_some_and(|limit| self.searched_nodes >= limit)
    }

    fn time_limit_reached(&self) -> bool {
        self.deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
    }

    fn should_stop(&self) -> bool {
        self.node_limit_reached() || self.time_limit_reached()
    }
}

impl ExactSearchState {
    fn new(deadline: Option<Instant>) -> Self {
        Self {
            searched_nodes: 0,
            transposition_table: TranspositionTable::default(),
            deadline,
        }
    }

    fn time_limit_reached(&self) -> bool {
        self.deadline.is_some_and(|deadline| {
            (self.searched_nodes <= 1
                || self
                    .searched_nodes
                    .is_multiple_of(EXACT_DEADLINE_CHECK_INTERVAL))
                && Instant::now() >= deadline
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BoardKey, BoundKind, CORNER_MASK, EDGE_MASK, ExactSearchFailureReason, SCORE_INF,
        SCORE_MAX, ScoreKind, SearchConfig, SearchLine, SearchResult, SearchState, SolveConfig,
        SolveError, TranspositionEntry, TranspositionTable, can_solve_exact,
        corner_closeness_penalty, deadline_from_config, determine_bound, frontier_count,
        is_immediate_win_priority, leaf_score, mid_evaluate_diff, neighbor_mask, opponent_board,
        ordered_moves, oriented_bits, potential_mobility, search_best_move, search_best_move_exact,
        solve_exact, update_root_candidates,
    };
    use crate::engine::{
        Board, BoardStatus, Color, Move, apply_forced_pass, apply_move_unchecked, board_status,
        final_margin_from_side_to_move, generate_legal_moves, legal_moves_to_vec,
    };
    use crate::random_play::{RandomPlayConfig, play_random_game};
    use std::time::{Duration, Instant};

    fn bit(square: u8) -> u64 {
        1u64 << square
    }

    fn manual_neighbor_mask(bits: u64) -> u64 {
        let mut result = 0u64;
        for square in 0..64 {
            if bits & bit(square) == 0 {
                continue;
            }
            let file = (square % 8) as i8;
            let rank = (square / 8) as i8;
            for (df, dr) in [
                (-1, -1),
                (0, -1),
                (1, -1),
                (-1, 0),
                (1, 0),
                (-1, 1),
                (0, 1),
                (1, 1),
            ] {
                let next_file = file + df;
                let next_rank = rank + dr;
                if (0..8).contains(&next_file) && (0..8).contains(&next_rank) {
                    result |= bit((next_rank * 8 + next_file) as u8);
                }
            }
        }
        result
    }

    fn manual_corner_closeness_penalty(player_bits: u64, opponent_bits: u64) -> i32 {
        let corners = [
            (0u8, [1u8, 8u8], 9u8),
            (7u8, [6u8, 15u8], 14u8),
            (56u8, [48u8, 57u8], 49u8),
            (63u8, [55u8, 62u8], 54u8),
        ];
        let mut diff = 0i32;
        for (corner, c_squares, x_square) in corners {
            if (player_bits | opponent_bits) & bit(corner) != 0 {
                continue;
            }
            for square in c_squares {
                if opponent_bits & bit(square) != 0 {
                    diff += 1;
                }
                if player_bits & bit(square) != 0 {
                    diff -= 1;
                }
            }
            if opponent_bits & bit(x_square) != 0 {
                diff += 2;
            }
            if player_bits & bit(x_square) != 0 {
                diff -= 2;
            }
        }
        diff
    }

    fn manual_mid_evaluate_diff(board: &Board) -> i16 {
        let (player_bits, opponent_bits) = oriented_bits(board);
        let empty_bits = !(player_bits | opponent_bits);
        let empty_count = empty_bits.count_ones() as u8;
        let disc_diff = player_bits.count_ones() as i32 - opponent_bits.count_ones() as i32;
        let mobility_diff = generate_legal_moves(board).count as i32
            - generate_legal_moves(&opponent_board(board)).count as i32;
        let potential_mobility_diff = potential_mobility(opponent_bits, empty_bits)
            - potential_mobility(player_bits, empty_bits);
        let frontier_diff =
            frontier_count(opponent_bits, empty_bits) - frontier_count(player_bits, empty_bits);
        let corner_diff = (player_bits & CORNER_MASK).count_ones() as i32
            - (opponent_bits & CORNER_MASK).count_ones() as i32;
        let edge_diff = (player_bits & EDGE_MASK).count_ones() as i32
            - (opponent_bits & EDGE_MASK).count_ones() as i32;
        let corner_closeness_diff = manual_corner_closeness_penalty(player_bits, opponent_bits);
        let parity_term = if empty_count.is_multiple_of(2) { -1 } else { 1 };

        let (disc_weight, mobility_weight, potential_weight, frontier_weight) = if empty_count >= 41
        {
            (0, 10, 6, 6)
        } else if empty_count >= 21 {
            (2, 7, 4, 5)
        } else {
            (6, 4, 2, 3)
        };

        let raw = 24 * corner_diff
            + 3 * edge_diff
            + mobility_weight * mobility_diff
            + potential_weight * potential_mobility_diff
            + frontier_weight * frontier_diff
            + disc_weight * disc_diff
            + 8 * corner_closeness_diff
            + 2 * parity_term;

        (raw / 8).clamp(-(SCORE_MAX as i32), SCORE_MAX as i32) as i16
    }

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
        pick_endgame_board(6, 2)
    }

    fn pick_endgame_board(max_empty: u8, min_legal_count: u8) -> Board {
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
                if empty <= max_empty && legal.count >= min_legal_count {
                    return board;
                }
            }
        }
        panic!(
            "endgame board not found for max_empty={max_empty} min_legal_count={min_legal_count}"
        );
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
    fn search_best_move_exact_matches_solve_exact_on_small_endgame() {
        let board = apply_forced_pass(
            &Board::from_bits(0xFFFF_FFFF_FFFF_FF7E, 0x0000_0000_0000_0080, Color::Black)
                .expect("board must be valid"),
        )
        .expect("forced pass must succeed");
        let exact = solve_exact(
            &board,
            &SolveConfig {
                exact_solver_empty_threshold: 1,
            },
        )
        .expect("small endgame must be exact-solvable");
        let timed = search_best_move_exact(&board, Duration::from_secs_f64(1.0))
            .expect("timed exact search must succeed");

        assert_eq!(timed.best_move, exact.best_move);
        assert_eq!(timed.exact_margin, exact.exact_margin);
        assert_eq!(timed.pv, exact.pv);
        assert!(timed.searched_nodes >= 1);
    }

    #[test]
    fn search_best_move_exact_returns_timeout_on_large_board_with_zero_budget() {
        let board = Board::new_initial();
        let result = search_best_move_exact(&board, Duration::ZERO);

        assert_eq!(
            result,
            Err(super::ExactSearchFailure {
                reason: ExactSearchFailureReason::Timeout,
                searched_nodes: 1,
            })
        );
    }

    #[test]
    #[ignore = "benchmark helper"]
    fn exact_search_bench_multi_move_endgame() {
        let board = pick_multi_move_endgame_board();
        let iterations = 10u32;

        let serial_start = Instant::now();
        let mut serial_nodes = 0u64;
        for _ in 0..iterations {
            let result = solve_exact(
                &board,
                &SolveConfig {
                    exact_solver_empty_threshold: 6,
                },
            )
            .expect("benchmark board must be exact-solvable");
            serial_nodes += result.searched_nodes;
        }
        let serial_elapsed = serial_start.elapsed();

        let parallel_start = Instant::now();
        let mut parallel_nodes = 0u64;
        for _ in 0..iterations {
            let result = search_best_move_exact(&board, Duration::from_secs_f64(1.0))
                .expect("benchmark board must finish within timeout");
            parallel_nodes += result.searched_nodes;
        }
        let parallel_elapsed = parallel_start.elapsed();

        eprintln!(
            "exact bench multi-move endgame: iterations={iterations} serial={:?} parallel={:?} serial_nodes={} parallel_nodes={}",
            serial_elapsed, parallel_elapsed, serial_nodes, parallel_nodes
        );
    }

    #[test]
    #[ignore = "benchmark helper"]
    fn exact_search_bench_deeper_endgame() {
        let board = pick_endgame_board(10, 4);
        let iterations = 5u32;

        let serial_start = Instant::now();
        let mut serial_nodes = 0u64;
        for _ in 0..iterations {
            let result = solve_exact(
                &board,
                &SolveConfig {
                    exact_solver_empty_threshold: 10,
                },
            )
            .expect("benchmark board must be exact-solvable");
            serial_nodes += result.searched_nodes;
        }
        let serial_elapsed = serial_start.elapsed();

        let parallel_start = Instant::now();
        let mut parallel_nodes = 0u64;
        for _ in 0..iterations {
            let result = search_best_move_exact(&board, Duration::from_secs_f64(5.0))
                .expect("benchmark board must finish within timeout");
            parallel_nodes += result.searched_nodes;
        }
        let parallel_elapsed = parallel_start.elapsed();

        eprintln!(
            "exact bench deeper endgame: iterations={iterations} serial={:?} parallel={:?} serial_nodes={} parallel_nodes={}",
            serial_elapsed, parallel_elapsed, serial_nodes, parallel_nodes
        );
    }

    #[test]
    #[ignore = "benchmark helper"]
    fn exact_search_bench_larger_endgame() {
        let board = pick_endgame_board(12, 4);
        let iterations = 3u32;

        let serial_start = Instant::now();
        let mut serial_nodes = 0u64;
        for _ in 0..iterations {
            let result = solve_exact(
                &board,
                &SolveConfig {
                    exact_solver_empty_threshold: 12,
                },
            )
            .expect("benchmark board must be exact-solvable");
            serial_nodes += result.searched_nodes;
        }
        let serial_elapsed = serial_start.elapsed();

        let parallel_start = Instant::now();
        let mut parallel_nodes = 0u64;
        for _ in 0..iterations {
            let result = search_best_move_exact(&board, Duration::from_secs_f64(10.0))
                .expect("benchmark board must finish within timeout");
            parallel_nodes += result.searched_nodes;
        }
        let parallel_elapsed = parallel_start.elapsed();

        eprintln!(
            "exact bench larger endgame: iterations={iterations} serial={:?} parallel={:?} serial_nodes={} parallel_nodes={}",
            serial_elapsed, parallel_elapsed, serial_nodes, parallel_nodes
        );
    }

    #[test]
    #[ignore = "benchmark helper"]
    fn exact_search_bench_even_larger_endgame() {
        let board = pick_endgame_board(14, 4);
        let iterations = 2u32;

        let serial_start = Instant::now();
        let mut serial_nodes = 0u64;
        for _ in 0..iterations {
            let result = solve_exact(
                &board,
                &SolveConfig {
                    exact_solver_empty_threshold: 14,
                },
            )
            .expect("benchmark board must be exact-solvable");
            serial_nodes += result.searched_nodes;
        }
        let serial_elapsed = serial_start.elapsed();

        let parallel_start = Instant::now();
        let mut parallel_nodes = 0u64;
        for _ in 0..iterations {
            let result = search_best_move_exact(&board, Duration::from_secs_f64(20.0))
                .expect("benchmark board must finish within timeout");
            parallel_nodes += result.searched_nodes;
        }
        let parallel_elapsed = parallel_start.elapsed();

        eprintln!(
            "exact bench even larger endgame: iterations={iterations} serial={:?} parallel={:?} serial_nodes={} parallel_nodes={}",
            serial_elapsed, parallel_elapsed, serial_nodes, parallel_nodes
        );
    }

    #[test]
    #[ignore = "benchmark helper"]
    fn exact_search_bench_sixteen_empty_endgame() {
        let board = pick_endgame_board(16, 4);
        let iterations = 1u32;

        let serial_start = Instant::now();
        let mut serial_nodes = 0u64;
        for _ in 0..iterations {
            let result = solve_exact(
                &board,
                &SolveConfig {
                    exact_solver_empty_threshold: 16,
                },
            )
            .expect("benchmark board must be exact-solvable");
            serial_nodes += result.searched_nodes;
        }
        let serial_elapsed = serial_start.elapsed();

        let parallel_start = Instant::now();
        let mut parallel_nodes = 0u64;
        for _ in 0..iterations {
            let result = search_best_move_exact(&board, Duration::from_secs_f64(60.0))
                .expect("benchmark board must finish within timeout");
            parallel_nodes += result.searched_nodes;
        }
        let parallel_elapsed = parallel_start.elapsed();

        eprintln!(
            "exact bench sixteen-empty endgame: iterations={iterations} serial={:?} parallel={:?} serial_nodes={} parallel_nodes={}",
            serial_elapsed, parallel_elapsed, serial_nodes, parallel_nodes
        );
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
    fn search_best_move_matches_with_and_without_transposition_table() {
        let board = Board::new_initial();
        let without_tt = search_best_move(
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
        let with_tt = search_best_move(
            &board,
            &SearchConfig {
                max_depth: Some(4),
                max_nodes: None,
                time_limit_ms: None,
                exact_solver_empty_threshold: None,
                use_transposition_table: true,
                multi_pv: 1,
            },
        );

        assert_eq!(with_tt.best_move, without_tt.best_move);
        assert_eq!(with_tt.best_score, without_tt.best_score);
        assert_eq!(with_tt.score_kind, without_tt.score_kind);
        assert_eq!(with_tt.is_exact, without_tt.is_exact);
        assert!(with_tt.searched_nodes <= without_tt.searched_nodes);
    }

    #[test]
    fn transposition_table_does_not_change_exact_threshold_path() {
        let board = pick_multi_move_endgame_board();
        let without_tt = search_best_move(
            &board,
            &SearchConfig {
                max_depth: Some(4),
                max_nodes: None,
                time_limit_ms: None,
                exact_solver_empty_threshold: Some(6),
                use_transposition_table: false,
                multi_pv: 1,
            },
        );
        let with_tt = search_best_move(
            &board,
            &SearchConfig {
                max_depth: Some(4),
                max_nodes: None,
                time_limit_ms: None,
                exact_solver_empty_threshold: Some(6),
                use_transposition_table: true,
                multi_pv: 1,
            },
        );

        assert_eq!(with_tt.best_move, without_tt.best_move);
        assert_eq!(with_tt.best_score, without_tt.best_score);
        assert_eq!(with_tt.pv, without_tt.pv);
        assert_eq!(with_tt.is_exact, without_tt.is_exact);
    }

    #[test]
    fn search_best_move_time_limit_keeps_result_shape_valid() {
        let board = Board::new_initial();
        let unlimited = search_best_move(
            &board,
            &SearchConfig {
                max_depth: Some(5),
                max_nodes: None,
                time_limit_ms: None,
                exact_solver_empty_threshold: None,
                use_transposition_table: true,
                multi_pv: 1,
            },
        );
        let limited = search_best_move(
            &board,
            &SearchConfig {
                max_depth: Some(5),
                max_nodes: None,
                time_limit_ms: Some(1),
                exact_solver_empty_threshold: None,
                use_transposition_table: true,
                multi_pv: 1,
            },
        );

        assert_eq!(limited.score_kind, ScoreKind::MarginFromSideToMove);
        assert!(limited.reached_depth <= 5);
        assert!(limited.searched_nodes <= unlimited.searched_nodes);
        if let Some(best_move) = limited.best_move {
            assert!(generate_legal_moves(&board).bitmask & (1u64 << best_move.square) != 0);
        }
    }

    #[test]
    fn search_best_move_time_limit_does_not_interrupt_exact_solver() {
        let board = pick_multi_move_endgame_board();
        let unlimited = search_best_move(
            &board,
            &SearchConfig {
                max_depth: Some(4),
                max_nodes: None,
                time_limit_ms: None,
                exact_solver_empty_threshold: Some(6),
                use_transposition_table: true,
                multi_pv: 1,
            },
        );
        let limited = search_best_move(
            &board,
            &SearchConfig {
                max_depth: Some(4),
                max_nodes: None,
                time_limit_ms: Some(0),
                exact_solver_empty_threshold: Some(6),
                use_transposition_table: true,
                multi_pv: 1,
            },
        );

        assert_eq!(limited.best_move, unlimited.best_move);
        assert_eq!(limited.best_score, unlimited.best_score);
        assert_eq!(limited.pv, unlimited.pv);
        assert!(limited.is_exact);
    }

    #[test]
    fn multi_pv_does_not_change_best_line() {
        let board = Board::new_initial();
        let single = search_best_move(
            &board,
            &SearchConfig {
                max_depth: Some(4),
                max_nodes: None,
                time_limit_ms: None,
                exact_solver_empty_threshold: None,
                use_transposition_table: true,
                multi_pv: 1,
            },
        );
        let multi = search_best_move(
            &board,
            &SearchConfig {
                max_depth: Some(4),
                max_nodes: None,
                time_limit_ms: None,
                exact_solver_empty_threshold: None,
                use_transposition_table: true,
                multi_pv: 3,
            },
        );

        assert_eq!(multi.best_move, single.best_move);
        assert_eq!(multi.best_score, single.best_score);
        assert_eq!(multi.pv, single.pv);
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

    #[test]
    fn transposition_table_prefers_deeper_or_exact_entries() {
        let board = Board::new_initial();
        let mut table = TranspositionTable::default();
        let key = BoardKey::new(&board);

        table.store(
            &board,
            TranspositionEntry {
                depth: 4,
                bound: BoundKind::Upper,
                score: 3,
                best_move: Some(Move { square: 19 }),
                is_exact: false,
            },
        );
        table.store(
            &board,
            TranspositionEntry {
                depth: 3,
                bound: BoundKind::Exact,
                score: 5,
                best_move: Some(Move { square: 26 }),
                is_exact: true,
            },
        );
        assert_eq!(
            table.lookup(key, 1).expect("entry must exist").best_move,
            Some(Move { square: 19 })
        );

        table.store(
            &board,
            TranspositionEntry {
                depth: 4,
                bound: BoundKind::Exact,
                score: 7,
                best_move: Some(Move { square: 26 }),
                is_exact: true,
            },
        );
        assert_eq!(
            table.lookup(key, 1).expect("entry must exist").best_move,
            Some(Move { square: 26 })
        );

        table.store(
            &board,
            TranspositionEntry {
                depth: 5,
                bound: BoundKind::Lower,
                score: 9,
                best_move: Some(Move { square: 37 }),
                is_exact: false,
            },
        );
        assert_eq!(
            table.lookup(key, 1).expect("entry must exist").best_move,
            Some(Move { square: 37 })
        );
        assert_eq!(table.best_move_for(&board), Some(Move { square: 37 }));
    }

    #[test]
    fn search_state_limit_helpers_reflect_configuration() {
        let mut state = SearchState {
            searched_nodes: 5,
            max_nodes: Some(5),
            exact_solver_empty_threshold: None,
            transposition_table: None,
            deadline: Some(Instant::now() - Duration::from_millis(1)),
        };

        assert!(state.node_limit_reached());
        assert!(state.time_limit_reached());
        assert!(state.should_stop());

        state.max_nodes = Some(6);
        state.deadline = Some(Instant::now() + Duration::from_secs(60));
        assert!(!state.node_limit_reached());
        assert!(!state.time_limit_reached());
        assert!(!state.should_stop());
    }

    #[test]
    fn search_state_should_stop_when_single_limit_is_hit() {
        let node_only = SearchState {
            searched_nodes: 4,
            max_nodes: Some(4),
            exact_solver_empty_threshold: None,
            transposition_table: None,
            deadline: None,
        };
        let time_only = SearchState {
            searched_nodes: 0,
            max_nodes: None,
            exact_solver_empty_threshold: None,
            transposition_table: None,
            deadline: Some(Instant::now() - Duration::from_millis(1)),
        };

        assert!(node_only.should_stop());
        assert!(time_only.should_stop());
    }

    #[test]
    fn determine_bound_classifies_upper_exact_and_lower() {
        assert_eq!(determine_bound(-4, -4, SCORE_INF), BoundKind::Upper);
        assert_eq!(determine_bound(3, -4, 5), BoundKind::Exact);
        assert_eq!(determine_bound(5, -4, 5), BoundKind::Lower);
    }

    #[test]
    fn deadline_from_config_reflects_time_limit_presence() {
        assert!(
            deadline_from_config(&SearchConfig {
                max_depth: Some(1),
                max_nodes: None,
                time_limit_ms: None,
                exact_solver_empty_threshold: None,
                use_transposition_table: false,
                multi_pv: 1,
            })
            .is_none()
        );

        assert!(
            deadline_from_config(&SearchConfig {
                max_depth: Some(1),
                max_nodes: None,
                time_limit_ms: Some(10),
                exact_solver_empty_threshold: None,
                use_transposition_table: false,
                multi_pv: 1,
            })
            .is_some()
        );

        let now = Instant::now();
        let deadline = deadline_from_config(&SearchConfig {
            max_depth: Some(1),
            max_nodes: None,
            time_limit_ms: Some(50),
            exact_solver_empty_threshold: None,
            use_transposition_table: false,
            multi_pv: 1,
        })
        .expect("deadline must exist");
        assert!(deadline > now);
    }

    #[test]
    fn oriented_bits_follow_side_to_move() {
        let black_to_move = Board::from_bits(bit(0), bit(1), Color::Black).expect("valid");
        let white_to_move = Board::from_bits(bit(0), bit(1), Color::White).expect("valid");

        assert_eq!(oriented_bits(&black_to_move), (bit(0), bit(1)));
        assert_eq!(oriented_bits(&white_to_move), (bit(1), bit(0)));
    }

    #[test]
    fn neighbor_mask_matches_expected_for_corner_and_center() {
        assert_eq!(neighbor_mask(bit(0)), bit(1) | bit(8) | bit(9));
        assert_eq!(
            neighbor_mask(bit(27)),
            bit(18) | bit(19) | bit(20) | bit(26) | bit(28) | bit(34) | bit(35) | bit(36)
        );
        assert_eq!(
            neighbor_mask(bit(0) | bit(2)),
            bit(1) | bit(3) | bit(8) | bit(9) | bit(10) | bit(11)
        );
    }

    #[test]
    fn neighbor_mask_matches_manual_oracle_for_overlapping_patterns() {
        let mut patterns = vec![
            bit(0) | bit(1) | bit(8),
            bit(9) | bit(10) | bit(17) | bit(18),
            bit(27) | bit(28) | bit(35),
            bit(54) | bit(55) | bit(62),
            bit(7) | bit(14) | bit(15) | bit(22),
        ];
        let mut seed = 0x9c3f_27a1_5b4d_e681_u64;
        for _ in 0..64 {
            seed ^= seed << 7;
            seed ^= seed >> 9;
            seed ^= seed << 8;
            patterns.push(seed);
        }

        for bits in patterns {
            assert_eq!(neighbor_mask(bits), manual_neighbor_mask(bits));
        }
    }

    #[test]
    fn potential_mobility_and_frontier_count_match_hand_computed_values() {
        let bits = bit(27);
        let empty_bits =
            bit(0) | bit(18) | bit(19) | bit(20) | bit(26) | bit(28) | bit(34) | bit(35) | bit(36);

        assert_eq!(potential_mobility(bits, empty_bits), 8);
        assert_eq!(frontier_count(bits, empty_bits), 1);
    }

    #[test]
    fn corner_closeness_penalty_counts_c_and_x_squares_for_empty_corner() {
        let player_bits = bit(1) | bit(8) | bit(9);
        let opponent_bits = bit(6) | bit(14);

        assert_eq!(corner_closeness_penalty(player_bits, 0), -4);
        assert_eq!(corner_closeness_penalty(0, opponent_bits), 3);
        assert_eq!(corner_closeness_penalty(player_bits, opponent_bits), -1);
    }

    #[test]
    fn corner_closeness_penalty_matches_manual_oracle_for_all_corners() {
        let cases = [
            (bit(1) | bit(8) | bit(9), bit(6) | bit(14)),
            (bit(48) | bit(49) | bit(57), bit(55) | bit(54) | bit(62)),
            (
                bit(1) | bit(15) | bit(48) | bit(54),
                bit(8) | bit(14) | bit(57) | bit(62),
            ),
            (bit(56) | bit(48) | bit(49) | bit(57), 0),
            (0, bit(63) | bit(54) | bit(55) | bit(62)),
        ];

        for (player_bits, opponent_bits) in cases {
            assert_eq!(
                corner_closeness_penalty(player_bits, opponent_bits),
                manual_corner_closeness_penalty(player_bits, opponent_bits)
            );
        }
    }

    #[test]
    fn mid_evaluate_diff_matches_fixed_values_across_phase_bands() {
        let opening = Board::from_bits(240786604032, 134217728, Color::White).expect("valid");
        let midgame =
            Board::from_bits(2369140658154504705, 4534491720744960, Color::White).expect("valid");
        let endgame =
            Board::from_bits(36737469621651075, 8678570009477326860, Color::White).expect("valid");

        assert_eq!(mid_evaluate_diff(&opening), 8);
        assert_eq!(mid_evaluate_diff(&Board::new_initial()), 0);
        assert_eq!(mid_evaluate_diff(&midgame), 2);
        assert_eq!(mid_evaluate_diff(&endgame), -1);
    }

    #[test]
    fn mid_evaluate_diff_matches_manual_formula_for_rich_phase_positions() {
        let boards = [
            Board::from_bits(70647382802432, 61813581417984, Color::Black).expect("valid"),
            Board::from_bits(2384191241584640, 4620693287222644224, Color::Black).expect("valid"),
            Board::from_bits(16204198439771746336, 871477710842515100, Color::Black)
                .expect("valid"),
        ];

        for board in boards {
            assert_eq!(mid_evaluate_diff(&board), manual_mid_evaluate_diff(&board));
        }
    }

    #[test]
    fn ordered_moves_prefers_tt_move_then_natural_order() {
        let board = Board::new_initial();
        let ordered = ordered_moves(
            &board,
            generate_legal_moves(&board),
            Some(Move { square: 37 }),
        );
        let squares: Vec<u8> = ordered
            .iter()
            .map(|candidate| candidate.mv.square)
            .collect();

        assert_eq!(squares, vec![37, 19, 26, 44]);
        assert!(is_immediate_win_priority(&ordered[0], true));
        assert!(!is_immediate_win_priority(&ordered[1], false));
    }

    #[test]
    fn leaf_score_negates_for_forced_pass_positions() {
        let board = Board::from_bits(0xFFFF_FFFF_FFFF_FF7E, 0x0000_0000_0000_0080, Color::Black)
            .expect("board must be valid");
        let passed = apply_forced_pass(&board).expect("forced pass must succeed");

        assert_eq!(board_status(&board), BoardStatus::ForcedPass);
        assert_eq!(leaf_score(&board), -leaf_score(&passed));
    }

    #[test]
    fn update_root_candidates_orders_by_score_and_truncates() {
        let mut candidates = Vec::new();
        update_root_candidates(
            &mut candidates,
            super::RootCandidate {
                line: SearchLine {
                    best_move: Some(Move { square: 19 }),
                    best_score: 3,
                    pv: vec![Move { square: 19 }],
                    is_exact: false,
                    completed: true,
                },
            },
            2,
        );
        update_root_candidates(
            &mut candidates,
            super::RootCandidate {
                line: SearchLine {
                    best_move: Some(Move { square: 26 }),
                    best_score: 7,
                    pv: vec![Move { square: 26 }, Move { square: 18 }],
                    is_exact: false,
                    completed: true,
                },
            },
            2,
        );
        update_root_candidates(
            &mut candidates,
            super::RootCandidate {
                line: SearchLine {
                    best_move: Some(Move { square: 37 }),
                    best_score: 5,
                    pv: vec![Move { square: 37 }],
                    is_exact: false,
                    completed: true,
                },
            },
            2,
        );

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].line.best_move, Some(Move { square: 26 }));
        assert_eq!(candidates[1].line.best_move, Some(Move { square: 37 }));
    }

    #[test]
    fn search_best_move_matches_bruteforce_at_depth_three_on_curated_random_boards() {
        let mut boards = Vec::new();
        for seed in [7u64, 13, 29] {
            let trace = play_random_game(
                seed,
                &RandomPlayConfig {
                    max_plies: Some(20),
                },
            );
            for &idx in &[4usize, 8, 12] {
                boards.push((seed, idx, trace.boards[idx]));
            }
        }

        for (seed, idx, board) in boards {
            let expected = brute_force_midgame(&board, 3);
            let result = search_best_move(
                &board,
                &SearchConfig {
                    max_depth: Some(3),
                    max_nodes: None,
                    time_limit_ms: None,
                    exact_solver_empty_threshold: None,
                    use_transposition_table: false,
                    multi_pv: 1,
                },
            );

            assert_eq!(
                result.best_move, expected.0,
                "seed {seed}, board index {idx}"
            );
            assert_eq!(
                result.best_score, expected.1,
                "seed {seed}, board index {idx}"
            );
        }
    }
}
