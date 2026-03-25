use crate::engine::Board;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symmetry {
    Identity,
    Rot90,
    Rot180,
    Rot270,
    FlipHorizontal,
    FlipVertical,
    FlipDiag,
    FlipAntiDiag,
}

pub fn all_symmetries() -> [Symmetry; 8] {
    [
        Symmetry::Identity,
        Symmetry::Rot90,
        Symmetry::Rot180,
        Symmetry::Rot270,
        Symmetry::FlipHorizontal,
        Symmetry::FlipVertical,
        Symmetry::FlipDiag,
        Symmetry::FlipAntiDiag,
    ]
}

pub fn transform_square(square: u8, sym: Symmetry) -> u8 {
    if square >= 64 {
        return square;
    }

    let file = square % 8;
    let rank = square / 8;
    let (new_file, new_rank) = match sym {
        Symmetry::Identity => (file, rank),
        Symmetry::Rot90 => (7 - rank, file),
        Symmetry::Rot180 => (7 - file, 7 - rank),
        Symmetry::Rot270 => (rank, 7 - file),
        Symmetry::FlipHorizontal => (7 - file, rank),
        Symmetry::FlipVertical => (file, 7 - rank),
        Symmetry::FlipDiag => (rank, file),
        Symmetry::FlipAntiDiag => (7 - rank, 7 - file),
    };
    new_rank * 8 + new_file
}

fn transform_bits(bits: u64, sym: Symmetry) -> u64 {
    let mut src = bits;
    let mut dst = 0u64;
    while src != 0 {
        let square = src.trailing_zeros() as u8;
        dst |= 1u64 << transform_square(square, sym);
        src &= src - 1;
    }
    dst
}

pub fn transform_board(board: &Board, sym: Symmetry) -> Board {
    Board {
        black_bits: transform_bits(board.black_bits, sym),
        white_bits: transform_bits(board.white_bits, sym),
        side_to_move: board.side_to_move,
    }
}

#[cfg(test)]
mod tests {
    use super::{Symmetry, all_symmetries, transform_board, transform_square};
    use crate::{
        Board, Color, all_symmetries as crate_all_symmetries, disc_count, final_margin_from_black,
        generate_legal_moves, legal_moves_to_vec,
    };

    fn bit(square: u8) -> u64 {
        1u64 << square
    }

    fn square(file: u8, rank: u8) -> u8 {
        rank * 8 + file
    }

    #[test]
    fn all_symmetries_returns_fixed_order() {
        assert_eq!(all_symmetries(), crate_all_symmetries());
    }

    #[test]
    fn transform_square_matches_fixed_coordinate_mapping() {
        let sq = square(3, 2);
        assert_eq!(transform_square(sq, Symmetry::Identity), 19);
        assert_eq!(transform_square(sq, Symmetry::Rot90), 29);
        assert_eq!(transform_square(sq, Symmetry::Rot180), 44);
        assert_eq!(transform_square(sq, Symmetry::Rot270), 34);
        assert_eq!(transform_square(sq, Symmetry::FlipHorizontal), 20);
        assert_eq!(transform_square(sq, Symmetry::FlipVertical), 43);
        assert_eq!(transform_square(sq, Symmetry::FlipDiag), 26);
        assert_eq!(transform_square(sq, Symmetry::FlipAntiDiag), 37);
    }

    #[test]
    fn transform_board_preserves_side_to_move_and_counts() {
        let board = Board::new_initial();
        let transformed = transform_board(&board, Symmetry::Rot90);

        assert_eq!(transformed.side_to_move, Color::Black);
        assert_eq!(disc_count(&transformed), disc_count(&board));
        assert_eq!(final_margin_from_black(&transformed), 0);
    }

    #[test]
    fn transform_board_matches_squarewise_mapping() {
        let board = Board {
            black_bits: bit(0) | bit(7) | bit(35),
            white_bits: bit(9) | bit(18),
            side_to_move: Color::White,
        };

        let transformed = transform_board(&board, Symmetry::FlipDiag);
        let expected_black = bit(transform_square(0, Symmetry::FlipDiag))
            | bit(transform_square(7, Symmetry::FlipDiag))
            | bit(transform_square(35, Symmetry::FlipDiag));
        let expected_white = bit(transform_square(9, Symmetry::FlipDiag))
            | bit(transform_square(18, Symmetry::FlipDiag));

        assert_eq!(transformed.black_bits, expected_black);
        assert_eq!(transformed.white_bits, expected_white);
    }

    #[test]
    fn rotational_and_reflection_symmetries_round_trip() {
        let board = Board {
            black_bits: bit(0) | bit(9) | bit(63),
            white_bits: bit(7) | bit(18) | bit(54),
            side_to_move: Color::White,
        };

        assert_eq!(
            transform_board(
                &transform_board(
                    &transform_board(&transform_board(&board, Symmetry::Rot90), Symmetry::Rot90),
                    Symmetry::Rot90,
                ),
                Symmetry::Rot90,
            ),
            board
        );
        assert_eq!(
            transform_board(
                &transform_board(&board, Symmetry::FlipHorizontal),
                Symmetry::FlipHorizontal
            ),
            board
        );
        assert_eq!(
            transform_board(
                &transform_board(&board, Symmetry::FlipVertical),
                Symmetry::FlipVertical
            ),
            board
        );
    }

    #[test]
    fn transformed_legal_moves_match_transformed_squares() {
        let board = Board::new_initial();
        let transformed = transform_board(&board, Symmetry::Rot90);
        let original_moves = legal_moves_to_vec(generate_legal_moves(&board));
        let transformed_moves = legal_moves_to_vec(generate_legal_moves(&transformed));

        let expected: Vec<u8> = original_moves
            .iter()
            .map(|mv| transform_square(mv.square, Symmetry::Rot90))
            .collect();
        let mut expected = expected;
        expected.sort_unstable();
        let mut actual: Vec<u8> = transformed_moves.iter().map(|mv| mv.square).collect();
        actual.sort_unstable();
        assert_eq!(actual, expected);
    }
}
