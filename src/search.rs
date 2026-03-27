use crate::engine::{
    Board, BoardStatus, Move, apply_forced_pass, apply_move_unchecked, board_status, disc_count,
    final_margin_from_side_to_move, generate_legal_moves, legal_moves_to_vec,
};

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

#[cfg(test)]
mod tests {
    use super::{SolveConfig, SolveError, can_solve_exact, solve_exact};
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
}
