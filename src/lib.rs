use pyo3::prelude::*;

// 初期局面で使う 4 マスのインデックス。
const D4: u8 = 27;
const E4: u8 = 28;
const D5: u8 = 35;
const E5: u8 = 36;

// 手番または石の色を表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    Black,
    White,
}

// 盤面を 2 枚のビットボードと手番で表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Board {
    pub black_bits: u64,
    pub white_bits: u64,
    pub side_to_move: Color,
}

// 盤面生成や検証で見つかった不整合を表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoardError {
    OverlappingBits,
}

// 着手位置 1 マスを表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub square: u8,
}

// 合法手のビット集合と件数をまとめて保持する。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LegalMoves {
    pub bitmask: u64,
    pub count: u8,
}

// 着手適用で起こりうる失敗理由を表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveError {
    IllegalMove,
    PassNotAllowed,
    TerminalBoard,
}

impl Board {
    // 標準的な初期局面を返す。
    pub fn new_initial() -> Self {
        Self {
            black_bits: (1u64 << E4) | (1u64 << D5),
            white_bits: (1u64 << D4) | (1u64 << E5),
            side_to_move: Color::Black,
        }
    }

    // 外部から渡されたビット列から盤面を作り、最低限の整合性を確認する。
    pub fn from_bits(
        black_bits: u64,
        white_bits: u64,
        side_to_move: Color,
    ) -> Result<Self, BoardError> {
        let board = Self {
            black_bits,
            white_bits,
            side_to_move,
        };
        board.validate()?;
        Ok(board)
    }

    // 内部で保持している盤面情報をそのまま返す。
    pub fn to_bits(&self) -> (u64, u64, Color) {
        (self.black_bits, self.white_bits, self.side_to_move)
    }

    // 黒石と白石が置かれている全マスを返す。
    pub fn occupied_bits(&self) -> u64 {
        self.black_bits | self.white_bits
    }

    // 石が置かれていない全マスを返す。
    pub fn empty_bits(&self) -> u64 {
        !self.occupied_bits()
    }

    // 盤面として最低限成立しているかだけを確認する。
    pub fn validate(&self) -> Result<(), BoardError> {
        if self.black_bits & self.white_bits != 0 {
            return Err(BoardError::OverlappingBits);
        }
        Ok(())
    }
}

// 現在の手番に対応する石配置を player / opponent の並びで返す。
fn player_and_opponent_bits(board: &Board) -> (u64, u64) {
    match board.side_to_move {
        Color::Black => (board.black_bits, board.white_bits),
        Color::White => (board.white_bits, board.black_bits),
    }
}

// 指定した 1 マスへの着手で裏返る相手石をビット集合で返す。
fn flips_for_move(board: &Board, mv: Move) -> u64 {
    if mv.square >= 64 {
        return 0;
    }

    let move_bit = 1u64 << mv.square;
    let (player_bits, opponent_bits) = player_and_opponent_bits(board);

    if (player_bits | opponent_bits) & move_bit != 0 {
        return 0;
    }

    let move_file = (mv.square % 8) as i8;
    let move_rank = (mv.square / 8) as i8;
    let directions = [
        (-1_i8, -1_i8),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ];
    let mut flips = 0u64;

    for (df, dr) in directions {
        let mut next_file = move_file + df;
        let mut next_rank = move_rank + dr;
        let mut line_flips = 0u64;

        while (0..8).contains(&next_file) && (0..8).contains(&next_rank) {
            let next_square = (next_rank as u8) * 8 + next_file as u8;
            let next_bit = 1u64 << next_square;

            if opponent_bits & next_bit != 0 {
                line_flips |= next_bit;
                next_file += df;
                next_rank += dr;
                continue;
            }

            if player_bits & next_bit != 0 {
                flips |= line_flips;
            }
            break;
        }
    }

    flips
}

// 合法手である前提で着手を反映した次局面を返す。
pub fn apply_move_unchecked(board: &Board, mv: Move) -> Board {
    let flips = flips_for_move(board, mv);
    let move_bit = 1u64 << mv.square;

    match board.side_to_move {
        Color::Black => Board {
            black_bits: board.black_bits | move_bit | flips,
            white_bits: board.white_bits & !flips,
            side_to_move: Color::White,
        },
        Color::White => Board {
            black_bits: board.black_bits & !flips,
            white_bits: board.white_bits | move_bit | flips,
            side_to_move: Color::Black,
        },
    }
}

// 合法性を確認してから着手を反映した次局面を返す。
pub fn apply_move(board: &Board, mv: Move) -> Result<Board, MoveError> {
    if flips_for_move(board, mv) == 0 {
        return Err(MoveError::IllegalMove);
    }

    Ok(apply_move_unchecked(board, mv))
}

// ビットボード演算で現在手番の合法手を列挙する。
pub fn generate_legal_moves(board: &Board) -> LegalMoves {
    let (player_bits, opponent_bits) = player_and_opponent_bits(board);
    let horizontal_opponent = opponent_bits & 0x7e7e7e7e7e7e7e7e_u64;

    let mut flip1 = horizontal_opponent & (player_bits << 1);
    let mut flip7 = horizontal_opponent & (player_bits << 7);
    let mut flip9 = horizontal_opponent & (player_bits << 9);
    let mut flip8 = opponent_bits & (player_bits << 8);

    flip1 |= horizontal_opponent & (flip1 << 1);
    flip7 |= horizontal_opponent & (flip7 << 7);
    flip9 |= horizontal_opponent & (flip9 << 9);
    flip8 |= opponent_bits & (flip8 << 8);

    let mut pre1 = horizontal_opponent & (horizontal_opponent << 1);
    let mut pre7 = horizontal_opponent & (horizontal_opponent << 7);
    let mut pre9 = horizontal_opponent & (horizontal_opponent << 9);
    let mut pre8 = opponent_bits & (opponent_bits << 8);

    flip1 |= pre1 & (flip1 << 2);
    flip7 |= pre7 & (flip7 << 14);
    flip9 |= pre9 & (flip9 << 18);
    flip8 |= pre8 & (flip8 << 16);

    flip1 |= pre1 & (flip1 << 2);
    flip7 |= pre7 & (flip7 << 14);
    flip9 |= pre9 & (flip9 << 18);
    flip8 |= pre8 & (flip8 << 16);

    let mut moves = flip1 << 1;
    moves |= flip7 << 7;
    moves |= flip9 << 9;
    moves |= flip8 << 8;

    flip1 = horizontal_opponent & (player_bits >> 1);
    flip7 = horizontal_opponent & (player_bits >> 7);
    flip9 = horizontal_opponent & (player_bits >> 9);
    flip8 = opponent_bits & (player_bits >> 8);

    flip1 |= horizontal_opponent & (flip1 >> 1);
    flip7 |= horizontal_opponent & (flip7 >> 7);
    flip9 |= horizontal_opponent & (flip9 >> 9);
    flip8 |= opponent_bits & (flip8 >> 8);

    pre1 >>= 1;
    pre7 >>= 7;
    pre9 >>= 9;
    pre8 >>= 8;

    flip1 |= pre1 & (flip1 >> 2);
    flip7 |= pre7 & (flip7 >> 14);
    flip9 |= pre9 & (flip9 >> 18);
    flip8 |= pre8 & (flip8 >> 16);

    flip1 |= pre1 & (flip1 >> 2);
    flip7 |= pre7 & (flip7 >> 14);
    flip9 |= pre9 & (flip9 >> 18);
    flip8 |= pre8 & (flip8 >> 16);

    moves |= flip1 >> 1;
    moves |= flip7 >> 7;
    moves |= flip9 >> 9;
    moves |= flip8 >> 8;
    moves &= !(player_bits | opponent_bits);

    LegalMoves {
        bitmask: moves,
        count: moves.count_ones() as u8,
    }
}

// Python 拡張モジュールのエントリポイント。
#[pymodule]
fn _core(_py: Python<'_>, _module: &Bound<'_, PyModule>) -> PyResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        Board, BoardError, Color, D4, D5, E4, E5, LegalMoves, Move, MoveError, apply_move,
        apply_move_unchecked, flips_for_move, generate_legal_moves,
    };

    // 1 マス分のビットを作る簡易ヘルパー。
    fn bit(square: u8) -> u64 {
        1u64 << square
    }

    // 0 始まりの file/rank から盤面インデックスへ変換する。
    fn square(file: u8, rank: u8) -> u8 {
        rank * 8 + file
    }

    // テスト側の基準値として使う素朴な合法手生成を実装する。
    fn generate_legal_moves_naive(board: &Board) -> LegalMoves {
        let (player_bits, opponent_bits) = match board.side_to_move {
            Color::Black => (board.black_bits, board.white_bits),
            Color::White => (board.white_bits, board.black_bits),
        };
        let directions = [
            (-1_i8, -1_i8),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ];
        let mut moves = 0u64;

        for rank in 0..8 {
            for file in 0..8 {
                let sq = square(file, rank);
                let sq_bit = bit(sq);

                if (player_bits | opponent_bits) & sq_bit != 0 {
                    continue;
                }

                let mut is_legal = false;
                for (df, dr) in directions {
                    let mut next_file = file as i8 + df;
                    let mut next_rank = rank as i8 + dr;
                    let mut seen_opponent = false;

                    while (0..8).contains(&next_file) && (0..8).contains(&next_rank) {
                        let next_sq = square(next_file as u8, next_rank as u8);
                        let next_bit = bit(next_sq);

                        if opponent_bits & next_bit != 0 {
                            seen_opponent = true;
                            next_file += df;
                            next_rank += dr;
                            continue;
                        }

                        if seen_opponent && player_bits & next_bit != 0 {
                            is_legal = true;
                        }
                        break;
                    }

                    if is_legal {
                        break;
                    }
                }

                if is_legal {
                    moves |= sq_bit;
                }
            }
        }

        LegalMoves {
            bitmask: moves,
            count: moves.count_ones() as u8,
        }
    }

    // 最適化版と素朴実装の結果、および期待値をまとめて照合する。
    fn assert_legal_moves(board: &Board, expected: u64) {
        let legal = generate_legal_moves(board);
        let naive = generate_legal_moves_naive(board);

        assert_eq!(legal.bitmask, expected);
        assert_eq!(legal.count, expected.count_ones() as u8);
        assert_eq!(legal, naive);
    }

    // 素朴実装で反転対象ビットを求め、最適化版との照合に使う。
    fn flips_for_move_naive(board: &Board, mv: Move) -> u64 {
        if mv.square >= 64 {
            return 0;
        }

        let move_bit = bit(mv.square);
        let (player_bits, opponent_bits) = match board.side_to_move {
            Color::Black => (board.black_bits, board.white_bits),
            Color::White => (board.white_bits, board.black_bits),
        };

        if (player_bits | opponent_bits) & move_bit != 0 {
            return 0;
        }

        let move_file = (mv.square % 8) as i8;
        let move_rank = (mv.square / 8) as i8;
        let directions = [
            (-1_i8, -1_i8),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ];
        let mut flips = 0u64;

        for (df, dr) in directions {
            let mut next_file = move_file + df;
            let mut next_rank = move_rank + dr;
            let mut line_flips = 0u64;

            while (0..8).contains(&next_file) && (0..8).contains(&next_rank) {
                let next_square = square(next_file as u8, next_rank as u8);
                let next_bit = bit(next_square);

                if opponent_bits & next_bit != 0 {
                    line_flips |= next_bit;
                    next_file += df;
                    next_rank += dr;
                    continue;
                }

                if player_bits & next_bit != 0 {
                    flips |= line_flips;
                }
                break;
            }
        }

        flips
    }

    // 素朴実装で合法手を 1 手適用した結果を返す。
    fn apply_move_naive(board: &Board, mv: Move) -> Result<Board, MoveError> {
        let flips = flips_for_move_naive(board, mv);
        if flips == 0 {
            return Err(MoveError::IllegalMove);
        }

        let move_bit = bit(mv.square);
        Ok(match board.side_to_move {
            Color::Black => Board {
                black_bits: board.black_bits | move_bit | flips,
                white_bits: board.white_bits & !flips,
                side_to_move: Color::White,
            },
            Color::White => Board {
                black_bits: board.black_bits & !flips,
                white_bits: board.white_bits | move_bit | flips,
                side_to_move: Color::Black,
            },
        })
    }

    // 盤面上の合法手をビット集合から列挙する。
    fn legal_move_list(legal: LegalMoves) -> Vec<Move> {
        let mut bitmask = legal.bitmask;
        let mut moves = Vec::new();

        while bitmask != 0 {
            let square = bitmask.trailing_zeros() as u8;
            moves.push(Move { square });
            bitmask &= bitmask - 1;
        }

        moves
    }

    #[test]
    fn color_represents_black_and_white() {
        // 色の列挙型が黒と白の 2 値を正しく区別できることを確認する。
        assert_eq!(Color::Black, Color::Black);
        assert_eq!(Color::White, Color::White);
        assert_ne!(Color::Black, Color::White);
    }

    #[test]
    fn board_keeps_black_white_bits_and_side_to_move() {
        // Board が黒石・白石・手番の値をそのまま保持できることを確認する。
        let board = Board {
            black_bits: 0x12,
            white_bits: 0x34,
            side_to_move: Color::White,
        };

        assert_eq!(board.black_bits, 0x12);
        assert_eq!(board.white_bits, 0x34);
        assert_eq!(board.side_to_move, Color::White);
    }

    #[test]
    fn new_initial_returns_standard_initial_position() {
        // 初期局面の石配置、石数、手番が想定どおりであることを確認する。
        let board = Board::new_initial();

        assert_eq!(board.black_bits, (1u64 << E4) | (1u64 << D5));
        assert_eq!(board.white_bits, (1u64 << D4) | (1u64 << E5));
        assert_eq!(board.side_to_move, Color::Black);
        assert_eq!(board.black_bits.count_ones(), 2);
        assert_eq!(board.white_bits.count_ones(), 2);
        assert!(board.validate().is_ok());
    }

    #[test]
    fn from_bits_rejects_overlapping_bits() {
        // 同じマスに黒白両方の石がある不正入力を拒否することを確認する。
        let result = Board::from_bits(1u64 << D4, 1u64 << D4, Color::Black);

        assert_eq!(result, Err(BoardError::OverlappingBits));
    }

    #[test]
    fn to_bits_returns_internal_state_without_changes() {
        // 保持している盤面情報を to_bits でそのまま取り出せることを確認する。
        let board = Board {
            black_bits: 0x12,
            white_bits: 0x34,
            side_to_move: Color::Black,
        };

        assert_eq!(board.to_bits(), (0x12, 0x34, Color::Black));
    }

    #[test]
    fn occupied_bits_matches_union_of_black_and_white() {
        // occupied_bits が黒石と白石の OR になっていることを確認する。
        let board = Board {
            black_bits: 0x12,
            white_bits: 0x34,
            side_to_move: Color::Black,
        };

        assert_eq!(board.occupied_bits(), 0x36);
    }

    #[test]
    fn empty_bits_returns_complement_of_occupied_bits() {
        // empty_bits が occupied_bits の補集合であることを確認する。
        let board = Board::new_initial();

        assert_eq!(board.empty_bits(), !board.occupied_bits());
        assert_eq!(board.occupied_bits() & board.empty_bits(), 0);
    }

    #[test]
    fn validate_checks_basic_consistency_only() {
        // validate が黒白ビットの重なりだけを基本整合性として検査することを確認する。
        let valid = Board::from_bits(0x02, 0x34, Color::Black);
        let invalid = Board {
            black_bits: 1u64 << E4,
            white_bits: 1u64 << E4,
            side_to_move: Color::White,
        };

        assert!(valid.is_ok());
        assert_eq!(invalid.validate(), Err(BoardError::OverlappingBits));
    }

    #[test]
    fn move_represent_square_index() {
        // Move が盤面インデックス 1 マスを保持できることを確認する。
        let mv = Move { square: 19 };

        assert_eq!(mv.square, 19);
    }

    #[test]
    fn move_error_represent_illegal_move() {
        // MoveError が非合法手を表現できることを確認する。
        assert_eq!(MoveError::IllegalMove, MoveError::IllegalMove);
        assert_ne!(MoveError::IllegalMove, MoveError::PassNotAllowed);
    }

    #[test]
    fn legal_moves_keeps_bitmask_and_count() {
        // LegalMoves がビット集合と件数をそのまま保持できることを確認する。
        let legal = LegalMoves {
            bitmask: 0b1010,
            count: 2,
        };

        assert_eq!(legal.bitmask, 0b1010);
        assert_eq!(legal.count, 2);
    }

    #[test]
    fn flips_for_move_returns_expected_discs_for_single_direction() {
        // 1 方向だけを反転する着手で反転対象ビットを正しく返すことを確認する。
        let board = Board {
            black_bits: bit(square(5, 3)),
            white_bits: bit(square(4, 3)),
            side_to_move: Color::Black,
        };
        let mv = Move {
            square: square(3, 3),
        };

        assert_eq!(flips_for_move(&board, mv), bit(square(4, 3)));
        assert_eq!(flips_for_move(&board, mv), flips_for_move_naive(&board, mv));
    }

    #[test]
    fn flips_for_move_returns_expected_discs_for_multiple_directions() {
        // 1 手で複数方向を同時に反転する場合も対象ビットを正しく返すことを確認する。
        let board = Board {
            black_bits: bit(square(1, 1))
                | bit(square(3, 1))
                | bit(square(5, 1))
                | bit(square(1, 3))
                | bit(square(5, 3))
                | bit(square(1, 5))
                | bit(square(3, 5))
                | bit(square(5, 5)),
            white_bits: bit(square(2, 2))
                | bit(square(3, 2))
                | bit(square(4, 2))
                | bit(square(2, 3))
                | bit(square(4, 3))
                | bit(square(2, 4))
                | bit(square(3, 4))
                | bit(square(4, 4)),
            side_to_move: Color::Black,
        };
        let mv = Move {
            square: square(3, 3),
        };
        let expected = bit(square(2, 2))
            | bit(square(3, 2))
            | bit(square(4, 2))
            | bit(square(2, 3))
            | bit(square(4, 3))
            | bit(square(2, 4))
            | bit(square(3, 4))
            | bit(square(4, 4));

        assert_eq!(flips_for_move(&board, mv), expected);
        assert_eq!(flips_for_move(&board, mv), flips_for_move_naive(&board, mv));
    }

    #[test]
    fn apply_move_unchecked_applies_initial_legal_move() {
        // 初期局面の合法手を unchecked 版で適用した結果が正しいことを確認する。
        let board = Board::new_initial();
        let next = apply_move_unchecked(
            &board,
            Move {
                square: square(3, 2),
            },
        );

        assert_eq!(
            next.black_bits,
            bit(square(3, 2)) | bit(square(3, 3)) | bit(square(3, 4)) | bit(square(4, 3))
        );
        assert_eq!(next.white_bits, bit(square(4, 4)));
        assert_eq!(next.side_to_move, Color::White);
    }

    #[test]
    fn apply_move_updates_discs_and_side_to_move() {
        // apply_move が石配置更新、反転処理、手番反転を行うことを確認する。
        let board = Board::new_initial();
        let next = apply_move(
            &board,
            Move {
                square: square(3, 2),
            },
        )
        .unwrap();

        assert_eq!(
            next,
            Board {
                black_bits: bit(square(3, 2))
                    | bit(square(3, 3))
                    | bit(square(3, 4))
                    | bit(square(4, 3)),
                white_bits: bit(square(4, 4)),
                side_to_move: Color::White,
            }
        );
    }

    #[test]
    fn apply_move_rejects_illegal_move() {
        // 既に石があるマスや挟めないマスへの着手を拒否することを確認する。
        let board = Board::new_initial();

        assert_eq!(
            apply_move(
                &board,
                Move {
                    square: square(3, 3)
                }
            ),
            Err(MoveError::IllegalMove)
        );
        assert_eq!(
            apply_move(
                &board,
                Move {
                    square: square(0, 0)
                }
            ),
            Err(MoveError::IllegalMove)
        );
    }

    #[test]
    fn apply_move_keeps_board_consistency_and_disc_counts() {
        // 着手後も黒白ビットが重ならず、石数が期待どおり変化することを確認する。
        let board = Board::new_initial();
        let mv = Move {
            square: square(3, 2),
        };
        let flips = flips_for_move(&board, mv);
        let next = apply_move(&board, mv).unwrap();

        assert_eq!(next.black_bits & next.white_bits, 0);
        assert_eq!(
            next.black_bits.count_ones(),
            board.black_bits.count_ones() + 1 + flips.count_ones()
        );
        assert_eq!(
            next.white_bits.count_ones() + flips.count_ones(),
            board.white_bits.count_ones()
        );
    }

    #[test]
    fn apply_move_handles_white_turn_position() {
        // 白番の合法手でも反転処理と手番更新が正しく行われることを確認する。
        let board = Board {
            black_bits: bit(square(3, 3)) | bit(square(4, 3)) | bit(square(3, 4)),
            white_bits: bit(square(4, 4)),
            side_to_move: Color::White,
        };
        let mv = Move {
            square: square(2, 2),
        };

        assert_eq!(apply_move(&board, mv), apply_move_naive(&board, mv));
        assert_eq!(
            apply_move(&board, mv).unwrap(),
            Board {
                black_bits: bit(square(4, 3)) | bit(square(3, 4)),
                white_bits: bit(square(2, 2)) | bit(square(3, 3)) | bit(square(4, 4)),
                side_to_move: Color::Black,
            }
        );
    }

    #[test]
    fn flips_for_move_rejects_occupied_and_out_of_range_squares() {
        // 既占有マスや盤面外インデックスでは反転対象 0 を返すことを確認する。
        let board = Board::new_initial();

        assert_eq!(
            flips_for_move(
                &board,
                Move {
                    square: square(4, 4)
                }
            ),
            0
        );
        assert_eq!(flips_for_move(&board, Move { square: 63 }), 0);
        assert_eq!(flips_for_move(&board, Move { square: 64 }), 0);
    }

    #[test]
    fn flips_for_move_rejects_occupied_square_even_if_it_would_flip_when_empty() {
        // 空きなら合法手になる筋でも、実際に埋まっていれば反転対象 0 を返すことを確認する。
        let board = Board {
            black_bits: bit(square(3, 3)) | bit(square(5, 3)),
            white_bits: bit(square(4, 3)),
            side_to_move: Color::Black,
        };
        let mv = Move {
            square: square(3, 3),
        };

        assert_eq!(flips_for_move(&board, mv), 0);
    }

    #[test]
    fn apply_move_matches_naive_oracle_for_curated_positions() {
        // 代表局面群の全合法手で最適化版と素朴実装が一致することを確認する。
        let positions = [
            Board::new_initial(),
            Board {
                black_bits: bit(square(3, 3)) | bit(square(4, 3)) | bit(square(3, 4)),
                white_bits: bit(square(4, 4)),
                side_to_move: Color::White,
            },
            Board {
                black_bits: bit(square(0, 0))
                    | bit(square(2, 0))
                    | bit(square(0, 2))
                    | bit(square(7, 7)),
                white_bits: bit(square(1, 0)) | bit(square(0, 1)) | bit(square(6, 6)),
                side_to_move: Color::Black,
            },
        ];

        for board in positions {
            for mv in legal_move_list(generate_legal_moves(&board)) {
                assert_eq!(flips_for_move(&board, mv), flips_for_move_naive(&board, mv));
                assert_eq!(apply_move(&board, mv), apply_move_naive(&board, mv));
                assert_eq!(
                    apply_move_unchecked(&board, mv),
                    apply_move_naive(&board, mv).unwrap()
                );
            }
        }
    }

    #[test]
    fn apply_move_matches_naive_oracle_for_deterministic_random_positions() {
        // さまざまな局面の全合法手で最適化版と素朴実装が一致することを確認する。
        let mut seed = 0x8f4a_c9d1_72be_31a5_u64;

        for _ in 0..128 {
            seed ^= seed << 7;
            seed ^= seed >> 9;
            let occupancy = seed;

            seed ^= seed << 7;
            seed ^= seed >> 9;
            let black_bits = seed & occupancy;
            let white_bits = occupancy & !black_bits;

            seed ^= seed << 7;
            seed ^= seed >> 9;
            let side_to_move = if seed & 1 == 0 {
                Color::Black
            } else {
                Color::White
            };

            let board = Board {
                black_bits,
                white_bits,
                side_to_move,
            };
            let legal = generate_legal_moves(&board);

            for mv in legal_move_list(legal) {
                assert_eq!(flips_for_move(&board, mv), flips_for_move_naive(&board, mv));
                assert_eq!(apply_move(&board, mv), apply_move_naive(&board, mv));
                assert_eq!(
                    apply_move_unchecked(&board, mv),
                    apply_move_naive(&board, mv).unwrap()
                );
            }
        }
    }

    #[test]
    fn generate_legal_moves_returns_expected_moves_for_initial_position() {
        // 初期局面の黒番合法手 4 つが正しく列挙されることを確認する。
        let expected = (1u64 << 19) | (1u64 << 26) | (1u64 << 37) | (1u64 << 44);
        assert_legal_moves(&Board::new_initial(), expected);
    }

    #[test]
    fn generate_legal_moves_returns_expected_moves_for_white_turn_position() {
        // 同じ石配置でも手番が白に変わると合法手集合が変わることを確認する。
        let board = Board {
            black_bits: (1u64 << E4) | (1u64 << D5),
            white_bits: (1u64 << D4) | (1u64 << E5),
            side_to_move: Color::White,
        };
        let expected = (1u64 << 20) | (1u64 << 29) | (1u64 << 34) | (1u64 << 43);
        assert_legal_moves(&board, expected);
    }

    #[test]
    fn generate_legal_moves_returns_empty_set_when_no_move_exists() {
        // 合法手が存在しない局面では空集合と件数 0 を返すことを確認する。
        let board = Board {
            black_bits: u64::MAX,
            white_bits: 0,
            side_to_move: Color::Black,
        };
        assert_legal_moves(&board, 0);
    }

    #[test]
    fn generate_legal_moves_returns_empty_set_when_empty_squares_exist_but_no_flip_exists() {
        // 空きマスが残っていても挟める筋がなければ合法手 0 件になることを確認する。
        let board = Board {
            black_bits: bit(square(0, 0)),
            white_bits: bit(square(7, 7)),
            side_to_move: Color::Black,
        };

        assert_legal_moves(&board, 0);
    }

    #[test]
    fn generate_legal_moves_handles_each_direction_independently() {
        // 8 方向のどの筋でも単独で合法手を見つけられることを確認する。
        let move_square = square(3, 3);
        let directions = [
            (-1_i8, -1_i8),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ];

        for (df, dr) in directions {
            let opponent_1 = square((3_i8 + df) as u8, (3_i8 + dr) as u8);
            let opponent_2 = square((3_i8 + 2 * df) as u8, (3_i8 + 2 * dr) as u8);
            let player = square((3_i8 + 3 * df) as u8, (3_i8 + 3 * dr) as u8);
            let board = Board {
                black_bits: bit(player),
                white_bits: bit(opponent_1) | bit(opponent_2),
                side_to_move: Color::Black,
            };

            assert_legal_moves(&board, bit(move_square));
        }
    }

    #[test]
    fn generate_legal_moves_handles_long_lines_in_each_direction() {
        // 6 個連続の相手石をまたぐ合法手でも 8 方向すべて正しく見つけられることを確認する。
        let directions = [
            (-1_i8, -1_i8),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ];

        for (df, dr) in directions {
            let move_file = if df < 0 { 7 } else { 0 };
            let move_rank = if dr < 0 { 7 } else { 0 };
            let move_square = square(move_file, move_rank);
            let mut white_bits = 0u64;

            for step in 1..=6 {
                let sq = square(
                    (move_file as i8 + df * step) as u8,
                    (move_rank as i8 + dr * step) as u8,
                );
                white_bits |= bit(sq);
            }

            let player_square = square(
                (move_file as i8 + df * 7) as u8,
                (move_rank as i8 + dr * 7) as u8,
            );
            let board = Board {
                black_bits: bit(player_square),
                white_bits,
                side_to_move: Color::Black,
            };

            assert_legal_moves(&board, bit(move_square));
        }
    }

    #[test]
    fn generate_legal_moves_handles_multiple_directions_for_one_move() {
        // 1 手で複数方向を同時に挟める局面でも着手先を正しく列挙することを確認する。
        let move_square = square(3, 3);
        let board = Board {
            black_bits: bit(square(1, 1))
                | bit(square(3, 1))
                | bit(square(5, 1))
                | bit(square(1, 3))
                | bit(square(5, 3))
                | bit(square(1, 5))
                | bit(square(3, 5))
                | bit(square(5, 5)),
            white_bits: bit(square(2, 2))
                | bit(square(3, 2))
                | bit(square(4, 2))
                | bit(square(2, 3))
                | bit(square(4, 3))
                | bit(square(2, 4))
                | bit(square(3, 4))
                | bit(square(4, 4)),
            side_to_move: Color::Black,
        };

        assert_legal_moves(&board, bit(move_square));
    }

    #[test]
    fn generate_legal_moves_matches_naive_oracle_for_curated_positions() {
        // 代表局面群に対して最適化版と素朴実装が常に一致することを確認する。
        let positions = [
            Board::new_initial(),
            Board {
                black_bits: (1u64 << E4) | (1u64 << D5),
                white_bits: (1u64 << D4) | (1u64 << E5),
                side_to_move: Color::White,
            },
            Board {
                black_bits: bit(square(0, 0)),
                white_bits: bit(square(7, 7)),
                side_to_move: Color::Black,
            },
            Board {
                black_bits: bit(square(3, 1))
                    | bit(square(5, 3))
                    | bit(square(1, 5))
                    | bit(square(5, 5)),
                white_bits: bit(square(3, 2))
                    | bit(square(4, 3))
                    | bit(square(2, 4))
                    | bit(square(4, 4)),
                side_to_move: Color::Black,
            },
            Board {
                black_bits: bit(square(0, 0)) | bit(square(7, 0)) | bit(square(0, 7)),
                white_bits: bit(square(1, 1)) | bit(square(2, 2)) | bit(square(6, 1)),
                side_to_move: Color::Black,
            },
        ];

        for board in positions {
            let legal = generate_legal_moves(&board);
            let naive = generate_legal_moves_naive(&board);

            assert_eq!(legal, naive);
        }
    }

    #[test]
    fn generate_legal_moves_matches_naive_oracle_for_deterministic_random_positions() {
        // さまざまな盤面密度と手番で最適化版と素朴実装が一致することを確認する。
        let mut seed = 0x4d59_5df4_d0f3_3173_u64;

        for _ in 0..256 {
            seed ^= seed << 7;
            seed ^= seed >> 9;
            let occupancy = seed;

            seed ^= seed << 7;
            seed ^= seed >> 9;
            let black_bits = seed & occupancy;
            let white_bits = occupancy & !black_bits;

            seed ^= seed << 7;
            seed ^= seed >> 9;
            let side_to_move = if seed & 1 == 0 {
                Color::Black
            } else {
                Color::White
            };

            let board = Board {
                black_bits,
                white_bits,
                side_to_move,
            };
            let legal = generate_legal_moves(&board);
            let naive = generate_legal_moves_naive(&board);

            assert_eq!(legal, naive);
        }
    }
}
