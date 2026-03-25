use crate::engine::{Board, BoardError, Color};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackedBoard {
    pub black_bits: u64,
    pub white_bits: u64,
    pub side_to_move: Color,
}

pub fn pack_board(board: &Board) -> PackedBoard {
    PackedBoard {
        black_bits: board.black_bits,
        white_bits: board.white_bits,
        side_to_move: board.side_to_move,
    }
}

pub fn unpack_board(packed: PackedBoard) -> Result<Board, BoardError> {
    Board::from_bits(packed.black_bits, packed.white_bits, packed.side_to_move)
}

#[cfg(test)]
mod tests {
    use super::{PackedBoard, pack_board, unpack_board};
    use crate::{Board, BoardError, Color};

    const D4: u8 = 27;

    #[test]
    fn packed_board_keeps_black_white_bits_and_side_to_move() {
        let packed = PackedBoard {
            black_bits: 1,
            white_bits: 2,
            side_to_move: Color::White,
        };

        assert_eq!(packed.black_bits, 1);
        assert_eq!(packed.white_bits, 2);
        assert_eq!(packed.side_to_move, Color::White);
    }

    #[test]
    fn pack_board_matches_initial_position_expected_values() {
        let packed = pack_board(&Board::new_initial());
        assert_eq!(packed.black_bits, 0x0000_0008_1000_0000);
        assert_eq!(packed.white_bits, 0x0000_0010_0800_0000);
        assert_eq!(packed.side_to_move, Color::Black);
    }

    #[test]
    fn pack_then_unpack_restores_original_board() {
        let board = Board {
            black_bits: 0x0123_4567_89ab_cdef,
            white_bits: 0x1000_0000_0000_0000,
            side_to_move: Color::White,
        };

        assert_eq!(unpack_board(pack_board(&board)), Ok(board));
    }

    #[test]
    fn unpack_then_pack_restores_packed_representation() {
        let packed = PackedBoard {
            black_bits: 0x55aa,
            white_bits: 0xaa55_0000,
            side_to_move: Color::Black,
        };

        assert_eq!(
            unpack_board(packed).map(|board| pack_board(&board)),
            Ok(packed)
        );
    }

    #[test]
    fn unpack_board_rejects_overlapping_discs() {
        let packed = PackedBoard {
            black_bits: 1u64 << D4,
            white_bits: 1u64 << D4,
            side_to_move: Color::Black,
        };

        assert_eq!(unpack_board(packed), Err(BoardError::OverlappingDiscs));
    }
}
