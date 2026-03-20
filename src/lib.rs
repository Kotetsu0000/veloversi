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

// Python 拡張モジュールのエントリポイント。
#[pymodule]
fn _core(_py: Python<'_>, _module: &Bound<'_, PyModule>) -> PyResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Board, BoardError, Color, D4, D5, E4, E5};

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
}
