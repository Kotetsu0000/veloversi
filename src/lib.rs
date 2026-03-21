mod flip_tables;

use flip_tables::{BB_DLINE02, BB_DLINE57, BB_FLIPPED, BB_H2VLINE, BB_MUL16, BB_SEED, BB_VLINE};
use pyo3::prelude::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::{
    __m128i, __m256i, _mm_cvtsi64_si128, _mm_cvtsi128_si64, _mm_or_si128, _mm_set_epi64x,
    _mm_set1_epi64x, _mm_shuffle_epi32, _mm_unpackhi_epi64, _mm_xor_si128, _mm256_add_epi64,
    _mm256_and_si256, _mm256_broadcastq_epi64, _mm256_castsi256_si128, _mm256_extracti128_si256,
    _mm256_or_si256, _mm256_set_epi64x, _mm256_sllv_epi64, _mm256_srlv_epi64,
};
use std::sync::OnceLock;

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

// 現局面の進行状態を表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoardStatus {
    Ongoing,
    ForcedPass,
    Terminal,
}

// 着手適用で起こりうる失敗理由を表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveError {
    IllegalMove,
    PassNotAllowed,
    TerminalBoard,
}

// Perft の実行時に起こりうる失敗理由を表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PerftError {
    InvalidMode,
}

// Perft におけるパスの数え方を表す内部モード。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PerftMode {
    Mode1,
    Mode2,
}

// 実行時に選択する SIMD 経路の優先設定を表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SimdPreference {
    Auto,
    Generic,
    Sse2,
    Avx2,
}

static SIMD_PREFERENCE: OnceLock<SimdPreference> = OnceLock::new();

// 環境変数から SIMD 経路の強制指定を読み取る。
fn simd_preference() -> SimdPreference {
    *SIMD_PREFERENCE.get_or_init(|| match std::env::var("VELOVERSI_SIMD") {
        Ok(value) => match value.to_ascii_lowercase().as_str() {
            "generic" => SimdPreference::Generic,
            "sse2" => SimdPreference::Sse2,
            "avx2" => SimdPreference::Avx2,
            _ => SimdPreference::Auto,
        },
        Err(_) => SimdPreference::Auto,
    })
}

// 合法手生成で使う実装経路名を返す。
fn selected_movegen_backend() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    {
        match simd_preference() {
            SimdPreference::Generic => "generic",
            SimdPreference::Sse2 => "generic",
            SimdPreference::Avx2 => {
                assert!(
                    std::arch::is_x86_feature_detected!("avx2"),
                    "VELOVERSI_SIMD=avx2 が指定されましたが、この CPU は avx2 非対応です"
                );
                "avx2"
            }
            SimdPreference::Auto => {
                if std::arch::is_x86_feature_detected!("avx2") {
                    "avx2"
                } else {
                    "generic"
                }
            }
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        "generic"
    }
}

// 盤面更新で使う実装経路名を返す。
fn selected_board_backend() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    {
        match simd_preference() {
            SimdPreference::Generic => "generic",
            SimdPreference::Sse2 | SimdPreference::Avx2 | SimdPreference::Auto => "sse2",
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        "generic"
    }
}

impl TryFrom<u8> for PerftMode {
    type Error = PerftError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Mode1),
            2 => Ok(Self::Mode2),
            _ => Err(PerftError::InvalidMode),
        }
    }
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

// 石配置を保ったまま手番だけを差し替えた盤面を返す。
fn board_with_side_to_move(board: &Board, side_to_move: Color) -> Board {
    Board {
        black_bits: board.black_bits,
        white_bits: board.white_bits,
        side_to_move,
    }
}

#[cfg(test)]
const NOT_A_FILE: u64 = 0xfefefefefefefefe;
#[cfg(test)]
const NOT_H_FILE: u64 = 0x7f7f7f7f7f7f7f7f;

// 内部で使う着手位置と反転ビットの組を表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Flip {
    pos: u8,
    move_bit: u64,
    flip: u64,
}

// 手番視点の player / opponent を保持する内部盤面表現。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OrientedBoard {
    player: u64,
    opponent: u64,
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn move_board_sse2(player: u64, opponent: u64, flip: Flip) -> (u64, u64) {
    let board = _mm_set_epi64x(opponent as i64, player as i64);
    let flips = _mm_set1_epi64x(flip.flip as i64);
    let swapped = _mm_shuffle_epi32(_mm_xor_si128(board, flips), 0x4e);
    let result = _mm_xor_si128(swapped, _mm_set_epi64x(flip.move_bit as i64, 0));
    let next_player = _mm_cvtsi128_si64(result) as u64;
    let next_opponent = _mm_cvtsi128_si64(_mm_unpackhi_epi64(result, result)) as u64;
    (next_player, next_opponent)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn undo_board_sse2(player: u64, opponent: u64, flip: Flip) -> (u64, u64) {
    let board = _mm_set_epi64x(opponent as i64, player as i64);
    let flips = _mm_set1_epi64x(flip.flip as i64);
    let swapped = _mm_shuffle_epi32(_mm_xor_si128(board, flips), 0x4e);
    let result = _mm_xor_si128(swapped, _mm_set_epi64x(0, flip.move_bit as i64));
    let prev_player = _mm_cvtsi128_si64(result) as u64;
    let prev_opponent = _mm_cvtsi128_si64(_mm_unpackhi_epi64(result, result)) as u64;
    (prev_player, prev_opponent)
}

impl OrientedBoard {
    // 外部公開用 Board から oriented な内部表現を作る。
    #[inline(always)]
    fn from_board(board: &Board) -> Self {
        let (player, opponent) = player_and_opponent_bits(board);
        Self { player, opponent }
    }

    // oriented な内部表現を指定手番の Board へ戻す。
    #[inline(always)]
    fn to_board(self, side_to_move: Color) -> Board {
        match side_to_move {
            Color::Black => Board {
                black_bits: self.player,
                white_bits: self.opponent,
                side_to_move,
            },
            Color::White => Board {
                black_bits: self.opponent,
                white_bits: self.player,
                side_to_move,
            },
        }
    }

    // 現手番の合法手集合を返す。
    #[inline(always)]
    fn legal_moves(self) -> u64 {
        legal_moves_bitmask(self.player, self.opponent)
    }

    // 指定マスの反転情報を返す。
    #[inline(always)]
    fn calc_flip(self, square: u8) -> Flip {
        Flip {
            pos: square,
            move_bit: if square < 64 { 1u64 << square } else { 0 },
            flip: flips_for_move_bits(self.player, self.opponent, square),
        }
    }

    // 合法手であることが分かっているマスの反転情報を返す。
    #[inline(always)]
    fn calc_flip_unchecked(self, square: u8) -> Flip {
        Flip {
            pos: square,
            move_bit: 1u64 << square,
            flip: flips_for_move_bits_unchecked(self.player, self.opponent, square),
        }
    }

    // 着手をその場で反映し、次手番視点へ更新する。
    #[inline(always)]
    fn move_board(&mut self, flip: Flip) {
        #[cfg(target_arch = "x86_64")]
        {
            if selected_board_backend() == "sse2" {
                let (next_player, next_opponent) =
                    unsafe { move_board_sse2(self.player, self.opponent, flip) };
                self.player = next_player;
                self.opponent = next_opponent;
                return;
            }
        }

        let next_player = self.opponent ^ flip.flip;
        let next_opponent = self.player ^ flip.flip ^ flip.move_bit;
        self.player = next_player;
        self.opponent = next_opponent;
    }

    // 着手を戻して元の手番視点へ戻す。
    #[inline(always)]
    fn undo_board(&mut self, flip: Flip) {
        #[cfg(target_arch = "x86_64")]
        {
            if selected_board_backend() == "sse2" {
                let (prev_player, prev_opponent) =
                    unsafe { undo_board_sse2(self.player, self.opponent, flip) };
                self.player = prev_player;
                self.opponent = prev_opponent;
                return;
            }
        }

        let prev_player = self.opponent ^ flip.flip ^ flip.move_bit;
        let prev_opponent = self.player ^ flip.flip;
        self.player = prev_player;
        self.opponent = prev_opponent;
    }

    // 着手済みの次局面をコピーとして返す。
    #[inline(always)]
    fn move_copy(self, flip: Flip) -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            if selected_board_backend() == "sse2" {
                let (player, opponent) =
                    unsafe { move_board_sse2(self.player, self.opponent, flip) };
                return Self { player, opponent };
            }
        }

        Self {
            player: self.opponent ^ flip.flip,
            opponent: self.player ^ flip.flip ^ flip.move_bit,
        }
    }

    // パスとして手番だけを入れ替える。
    #[inline(always)]
    fn pass(&mut self) {
        std::mem::swap(&mut self.player, &mut self.opponent);
    }
}

// 指定した 1 マスへの着手で裏返る相手石をビット集合で返す。
#[cfg(test)]
fn flips_for_move(board: &Board, mv: Move) -> u64 {
    OrientedBoard::from_board(board).calc_flip(mv.square).flip
}

// oriented ビットボードから合法手集合のビットマスクだけを返す。
#[inline(always)]
fn legal_moves_bitmask_generic(player_bits: u64, opponent_bits: u64) -> u64 {
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
    moves & !(player_bits | opponent_bits)
}

// 実行環境に応じて合法手生成を generic / AVX2 で切り替える。
#[inline(always)]
fn legal_moves_bitmask(player_bits: u64, opponent_bits: u64) -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        if selected_movegen_backend() == "avx2" {
            // AVX2 が使える環境では 4 方向を SIMD でまとめて処理する。
            return unsafe { legal_moves_bitmask_avx2(player_bits, opponent_bits) };
        }
    }

    legal_moves_bitmask_generic(player_bits, opponent_bits)
}

// AVX2 実装の合法手生成。
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn legal_moves_bitmask_avx2(player_bits: u64, opponent_bits: u64) -> u64 {
    let shifts: __m256i = _mm256_set_epi64x(7, 9, 8, 1);
    let shift_twice: __m256i = _mm256_add_epi64(shifts, shifts);
    let horizontal_mask: __m256i = _mm256_set_epi64x(
        0x7e7e7e7e7e7e7e7e_u64 as i64,
        0x7e7e7e7e7e7e7e7e_u64 as i64,
        -1,
        0x7e7e7e7e7e7e7e7e_u64 as i64,
    );
    let player_vec = _mm256_broadcastq_epi64(_mm_cvtsi64_si128(player_bits as i64));
    let opponent_vec = _mm256_and_si256(
        _mm256_broadcastq_epi64(_mm_cvtsi64_si128(opponent_bits as i64)),
        horizontal_mask,
    );

    let mut flip_l = _mm256_and_si256(opponent_vec, _mm256_sllv_epi64(player_vec, shifts));
    let mut flip_r = _mm256_and_si256(opponent_vec, _mm256_srlv_epi64(player_vec, shifts));

    flip_l = _mm256_or_si256(
        flip_l,
        _mm256_and_si256(opponent_vec, _mm256_sllv_epi64(flip_l, shifts)),
    );
    flip_r = _mm256_or_si256(
        flip_r,
        _mm256_and_si256(opponent_vec, _mm256_srlv_epi64(flip_r, shifts)),
    );

    let pre_l = _mm256_and_si256(opponent_vec, _mm256_sllv_epi64(opponent_vec, shifts));
    let pre_r = _mm256_srlv_epi64(pre_l, shifts);

    flip_l = _mm256_or_si256(
        flip_l,
        _mm256_and_si256(pre_l, _mm256_sllv_epi64(flip_l, shift_twice)),
    );
    flip_r = _mm256_or_si256(
        flip_r,
        _mm256_and_si256(pre_r, _mm256_srlv_epi64(flip_r, shift_twice)),
    );
    flip_l = _mm256_or_si256(
        flip_l,
        _mm256_and_si256(pre_l, _mm256_sllv_epi64(flip_l, shift_twice)),
    );
    flip_r = _mm256_or_si256(
        flip_r,
        _mm256_and_si256(pre_r, _mm256_srlv_epi64(flip_r, shift_twice)),
    );

    let moves_vec = _mm256_or_si256(
        _mm256_sllv_epi64(flip_l, shifts),
        _mm256_srlv_epi64(flip_r, shifts),
    );
    let mut moves128: __m128i = _mm_or_si128(
        _mm256_castsi256_si128(moves_vec),
        _mm256_extracti128_si256(moves_vec, 1),
    );
    moves128 = _mm_or_si128(moves128, _mm_unpackhi_epi64(moves128, moves128));

    (_mm_cvtsi128_si64(moves128) as u64) & !(player_bits | opponent_bits)
}

// `bb_seed` を横方向専用の 4 バイト単位アクセスとして読み出す。
#[inline(always)]
fn horizontal_seed(index: usize, x: usize) -> u8 {
    unsafe { *BB_SEED.as_ptr().cast::<u8>().add(index * 4 + x) }
}

// `bb_h2vline` を C++ 実装と同じく u32 オフセット経由で読み出す。
#[inline(always)]
fn read_h2vline(offset_u32: usize) -> u64 {
    let row = offset_u32 >> 1;
    if offset_u32 & 1 == 0 {
        BB_H2VLINE[row]
    } else {
        (BB_H2VLINE[row] >> 32) | ((BB_H2VLINE[row + 1] & 0xffff_ffff) << 32)
    }
}

// 斜め方向 1 本分の反転ビットを table から求める。
#[inline(always)]
fn diagonal_flip(line_mask: u64, player_bits: u64, opponent_bits: u64, x: usize) -> u64 {
    let mut outflank =
        BB_SEED[(((opponent_bits & line_mask).wrapping_mul(BB_VLINE[1])) >> 58) as usize][x];
    outflank &= (((player_bits & line_mask).wrapping_mul(0x0101010101010101)) >> 56) as u8;
    u64::from(BB_FLIPPED[outflank as usize][x]).wrapping_mul(0x0101010101010101) & line_mask
}

// oriented ビットボードに対して着手時の反転ビットを返す。
fn flips_for_move_bits(player_bits: u64, opponent_bits: u64, square: u8) -> u64 {
    if square >= 64 {
        return 0;
    }

    let move_bit = 1u64 << square;
    if (player_bits | opponent_bits) & move_bit != 0 {
        return 0;
    }

    flips_for_move_bits_unchecked(player_bits, opponent_bits, square)
}

// 合法手であることが分かっているマスの反転ビットを返す。
#[inline(always)]
fn flips_for_move_bits_unchecked(player_bits: u64, opponent_bits: u64, square: u8) -> u64 {
    let square = square as usize;
    let x = square & 7;
    let y = square >> 3;

    let mut outflank =
        ((((player_bits >> x) & 0x0101010101010101).wrapping_mul(0x0102040810204080)) >> 56) as u8;
    outflank &=
        BB_SEED[(((opponent_bits & BB_VLINE[x]).wrapping_mul(BB_MUL16[x])) >> 58) as usize][y];

    let mut flip = read_h2vline(BB_FLIPPED[outflank as usize][y] as usize) << x;

    let row_shift = square & 0x38;
    outflank = horizontal_seed(((opponent_bits >> row_shift) & 0x7e) as usize, x);
    outflank &= (player_bits >> row_shift) as u8;
    flip |= u64::from(BB_FLIPPED[outflank as usize][x]) << row_shift;

    flip |= diagonal_flip(BB_DLINE02[square], player_bits, opponent_bits, x);
    flip |= diagonal_flip(BB_DLINE57[square], player_bits, opponent_bits, x);

    flip
}

// 旧来の 8 方向走査版をテスト用の比較基準として残す。
#[cfg(test)]
fn flips_for_move_bits_scan(player_bits: u64, opponent_bits: u64, square: u8) -> u64 {
    if square >= 64 {
        return 0;
    }

    let move_bit = 1u64 << square;
    if (player_bits | opponent_bits) & move_bit != 0 {
        return 0;
    }

    let mut flips = 0u64;
    macro_rules! collect_flips {
        ($shift:expr, $mask:expr) => {{
            let mut line = $shift(move_bit) & $mask & opponent_bits;
            line |= $shift(line) & $mask & opponent_bits;
            line |= $shift(line) & $mask & opponent_bits;
            line |= $shift(line) & $mask & opponent_bits;
            line |= $shift(line) & $mask & opponent_bits;
            line |= $shift(line) & $mask & opponent_bits;
            if $shift(line) & $mask & player_bits != 0u64 {
                flips |= line;
            }
        }};
    }

    collect_flips!(|bits| bits << 1, NOT_A_FILE);
    collect_flips!(|bits| bits >> 1, NOT_H_FILE);
    collect_flips!(|bits| bits << 8, u64::MAX);
    collect_flips!(|bits| bits >> 8, u64::MAX);
    collect_flips!(|bits| bits << 9, NOT_A_FILE);
    collect_flips!(|bits| bits << 7, NOT_H_FILE);
    collect_flips!(|bits| bits >> 7, NOT_A_FILE);
    collect_flips!(|bits| bits >> 9, NOT_H_FILE);

    flips
}

// 合法手である前提で着手を反映した次局面を返す。
pub fn apply_move_unchecked(board: &Board, mv: Move) -> Board {
    let oriented = OrientedBoard::from_board(board);
    let next = oriented.move_copy(oriented.calc_flip(mv.square));
    next.to_board(match board.side_to_move {
        Color::Black => Color::White,
        Color::White => Color::Black,
    })
}

// 合法性を確認してから着手を反映した次局面を返す。
pub fn apply_move(board: &Board, mv: Move) -> Result<Board, MoveError> {
    let oriented = OrientedBoard::from_board(board);
    let flip = oriented.calc_flip(mv.square);
    if flip.flip == 0 {
        return Err(MoveError::IllegalMove);
    }

    let next = oriented.move_copy(flip);
    Ok(next.to_board(match board.side_to_move {
        Color::Black => Color::White,
        Color::White => Color::Black,
    }))
}

// 強制パス局面でのみ手番を反転した盤面を返す。
pub fn apply_forced_pass(board: &Board) -> Result<Board, MoveError> {
    match board_status(board) {
        BoardStatus::Ongoing => Err(MoveError::PassNotAllowed),
        BoardStatus::ForcedPass => Ok(board_with_side_to_move(
            board,
            match board.side_to_move {
                Color::Black => Color::White,
                Color::White => Color::Black,
            },
        )),
        BoardStatus::Terminal => Err(MoveError::TerminalBoard),
    }
}

// 現局面が継続中、強制パス、終局のどれかを返す。
pub fn board_status(board: &Board) -> BoardStatus {
    let oriented = OrientedBoard::from_board(board);
    if oriented.legal_moves() != 0 {
        return BoardStatus::Ongoing;
    }

    if legal_moves_bitmask(oriented.opponent, oriented.player) != 0 {
        BoardStatus::ForcedPass
    } else {
        BoardStatus::Terminal
    }
}

// oriented ビットボードのまま Perft を再帰実行する。
#[inline(always)]
fn perft_count_depth_two(board: &mut OrientedBoard, legal: u64, mode: PerftMode) -> u64 {
    let mut bitmask = legal;
    let mut nodes = 0u64;

    while bitmask != 0 {
        let square = bitmask.trailing_zeros() as u8;
        let flip = board.calc_flip_unchecked(square);
        board.move_board(flip);
        let next_legal = board.legal_moves();

        if next_legal != 0 {
            nodes += next_legal.count_ones() as u64;
        } else {
            board.pass();
            let reply_legal = board.legal_moves();
            board.pass();
            if reply_legal == 0 {
                nodes += 1;
            } else {
                nodes += match mode {
                    PerftMode::Mode1 => 1,
                    PerftMode::Mode2 => reply_legal.count_ones() as u64,
                };
            }
        }

        board.undo_board(flip);
        bitmask &= bitmask - 1;
    }

    nodes
}

// oriented ビットボードのまま Perft を再帰実行する。
#[inline(always)]
fn perft_with_mode_oriented(
    board: &mut OrientedBoard,
    depth: u8,
    mode: PerftMode,
    passed: bool,
) -> u64 {
    if depth == 0 {
        return 1;
    }

    let legal = board.legal_moves();
    if legal != 0 {
        if depth == 1 {
            return legal.count_ones() as u64;
        }
        if depth == 2 {
            return perft_count_depth_two(board, legal, mode);
        }
        if depth == 3 {
            let mut bitmask = legal;
            let mut nodes = 0u64;

            while bitmask != 0 {
                let square = bitmask.trailing_zeros() as u8;
                let flip = board.calc_flip_unchecked(square);
                board.move_board(flip);
                let next_legal = board.legal_moves();
                if next_legal != 0 {
                    nodes += perft_count_depth_two(board, next_legal, mode);
                } else {
                    board.pass();
                    let reply_legal = board.legal_moves();
                    if reply_legal == 0 {
                        nodes += 1;
                    } else {
                        nodes += match mode {
                            PerftMode::Mode1 => reply_legal.count_ones() as u64,
                            PerftMode::Mode2 => perft_count_depth_two(board, reply_legal, mode),
                        };
                    }
                    board.pass();
                }
                board.undo_board(flip);
                bitmask &= bitmask - 1;
            }

            return nodes;
        }

        let mut bitmask = legal;
        let mut nodes = 0u64;

        while bitmask != 0 {
            let square = bitmask.trailing_zeros() as u8;
            let flip = board.calc_flip_unchecked(square);
            board.move_board(flip);
            nodes += perft_with_mode_oriented(board, depth - 1, mode, false);
            board.undo_board(flip);
            bitmask &= bitmask - 1;
        }

        return nodes;
    }

    if passed {
        1
    } else {
        board.pass();
        let res = if board.legal_moves() == 0 {
            1
        } else {
            match mode {
                PerftMode::Mode1 => perft_with_mode_oriented(board, depth - 1, mode, true),
                PerftMode::Mode2 => perft_with_mode_oriented(board, depth, mode, true),
            }
        };
        board.pass();
        res
    }
}

// Perft の再帰本体をモード差だけ切り替えながら実行する。
fn perft_with_mode(board: &Board, depth: u8, mode: PerftMode) -> u64 {
    let mut oriented = OrientedBoard::from_board(board);
    perft_with_mode_oriented(&mut oriented, depth, mode, false)
}

// モード差を切り替えながら Perft の葉数を返す。
pub fn perft(board: &Board, depth: u8, mode: u8) -> Result<u64, PerftError> {
    Ok(perft_with_mode(board, depth, PerftMode::try_from(mode)?))
}

// ビットボード演算で現在手番の合法手を列挙する。
pub fn generate_legal_moves(board: &Board) -> LegalMoves {
    let moves = OrientedBoard::from_board(board).legal_moves();

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
        BB_H2VLINE, BB_SEED, Board, BoardError, BoardStatus, Color, D4, D5, E4, E5, LegalMoves,
        Move, MoveError, OrientedBoard, PerftError, PerftMode, apply_forced_pass, apply_move,
        apply_move_unchecked, board_status, board_with_side_to_move, flips_for_move,
        flips_for_move_bits_scan, generate_legal_moves, horizontal_seed, perft,
        perft_with_mode_oriented, read_h2vline,
    };
    use rayon::prelude::*;

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

    // table 版と旧走査版が同じ反転ビットを返すことを直接確認する。
    fn assert_flip_implementations_match(board: &Board, mv: Move) {
        let (player_bits, opponent_bits) = match board.side_to_move {
            Color::Black => (board.black_bits, board.white_bits),
            Color::White => (board.white_bits, board.black_bits),
        };

        assert_eq!(
            flips_for_move(board, mv),
            flips_for_move_bits_scan(player_bits, opponent_bits, mv.square)
        );
        assert_eq!(flips_for_move(board, mv), flips_for_move_naive(board, mv));
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

    // 素朴実装で Perft を計算し、内部再帰との照合に使う。
    fn perft_naive(board: &Board, depth: u8, mode: PerftMode, passed: bool) -> u64 {
        if depth == 0 {
            return 1;
        }

        let legal = generate_legal_moves_naive(board);
        if legal.count == 0 {
            if passed {
                return 1;
            }

            let passed_board = board_with_side_to_move(
                board,
                match board.side_to_move {
                    Color::Black => Color::White,
                    Color::White => Color::Black,
                },
            );

            return match mode {
                PerftMode::Mode1 => perft_naive(&passed_board, depth - 1, mode, true),
                PerftMode::Mode2 => perft_naive(&passed_board, depth, mode, true),
            };
        }

        if depth == 1 {
            return legal.count as u64;
        }

        legal_move_list(legal)
            .into_iter()
            .map(|mv| {
                let next = apply_move_naive(board, mv).unwrap();
                perft_naive(&next, depth - 1, mode, false)
            })
            .sum()
    }

    // depth 2 の終局分岐と強制パス分岐を踏む局面を決定的に探索する。
    fn find_depth_two_branch_examples() -> (Board, Move, Board, Move) {
        let mut seed = 0x91c7_2d4a_5be3_f801_u64;
        let mut terminal_example = None;
        let mut forced_example = None;

        for _ in 0..200_000 {
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

            for mv in legal_move_list(generate_legal_moves(&board)) {
                let next = apply_move_naive(&board, mv).unwrap();
                let next_legal = generate_legal_moves(&next);
                if next_legal.count != 0 {
                    continue;
                }

                let reply_board = board_with_side_to_move(
                    &next,
                    match next.side_to_move {
                        Color::Black => Color::White,
                        Color::White => Color::Black,
                    },
                );
                let reply_legal = generate_legal_moves(&reply_board);

                if reply_legal.count == 0 && terminal_example.is_none() {
                    terminal_example = Some((board, mv));
                } else if reply_legal.count > 0 && forced_example.is_none() {
                    forced_example = Some((board, mv));
                }

                if let (Some(terminal), Some(forced)) = (terminal_example, forced_example) {
                    return (terminal.0, terminal.1, forced.0, forced.1);
                }
            }
        }

        panic!("depth 2 branch examples not found");
    }

    // depth 3 の終局分岐と強制パス分岐を踏む局面を決定的に探索する。
    fn find_depth_three_branch_examples() -> (Board, Board) {
        let mut seed = 0x73e1_9ac4_0db2_f615_u64;
        let mut terminal_example = None;
        let mut forced_example = None;

        for _ in 0..300_000 {
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
            let legal = legal_move_list(generate_legal_moves(&board));
            if legal.is_empty() {
                continue;
            }

            for mv in legal {
                let next = apply_move_naive(&board, mv).unwrap();
                let next_legal = generate_legal_moves(&next);
                if next_legal.count != 0 {
                    continue;
                }

                let reply_board = board_with_side_to_move(
                    &next,
                    match next.side_to_move {
                        Color::Black => Color::White,
                        Color::White => Color::Black,
                    },
                );
                let reply_legal = generate_legal_moves(&reply_board);

                if reply_legal.count == 0 && terminal_example.is_none() {
                    terminal_example = Some(board);
                } else if reply_legal.count > 0 && forced_example.is_none() {
                    forced_example = Some(board);
                }

                if let (Some(terminal), Some(forced)) = (terminal_example, forced_example) {
                    return (terminal, forced);
                }
            }
        }

        panic!("depth 3 branch examples not found");
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

    // 黒番が強制パスになる固定局面を返す。
    fn forced_pass_board() -> Board {
        Board {
            black_bits: !bit(square(0, 0)) & !bit(square(7, 0)),
            white_bits: bit(square(7, 0)),
            side_to_move: Color::Black,
        }
    }

    // 両者とも合法手が存在しない固定局面を返す。
    fn terminal_board() -> Board {
        Board {
            black_bits: u64::MAX,
            white_bits: 0,
            side_to_move: Color::Black,
        }
    }

    // Perft の mode 差が深さ 1 で出る固定強制パス局面を返す。
    fn perft_forced_pass_difference_board() -> Board {
        Board {
            black_bits: 216_316_972_774_802_026,
            white_bits: 7_503_032_164_117_119_233,
            side_to_move: Color::Black,
        }
    }

    // ignored テストでルート手ごとの進捗を表示しながら Perft を確認する。
    fn assert_perft_long_with_progress(depth: u8, mode: u8, expected: u64) {
        let board = Board::new_initial();
        let legal = generate_legal_moves(&board);
        let moves = legal_move_list(legal);
        let mode_u8 = mode;
        let mode = PerftMode::try_from(mode).unwrap();
        let oriented = OrientedBoard::from_board(&board);
        let mut results: Vec<(u8, u64)> = moves
            .par_iter()
            .map(|mv| {
                let mut next = oriented.move_copy(oriented.calc_flip_unchecked(mv.square));
                let child = perft_with_mode_oriented(&mut next, depth - 1, mode, false);
                (mv.square, child)
            })
            .collect();

        results.sort_unstable_by_key(|(square, _)| *square);
        let total: u64 = results.iter().map(|(_, child)| *child).sum();

        println!("perft mode={} depth={}", mode_u8, depth);
        println!("root moves: {}", results.len());

        for (index, (square, child)) in results.iter().enumerate() {
            println!(
                "[{}/{}] square={} nodes={}",
                index + 1,
                results.len(),
                square,
                child
            );
        }

        println!("total={}", total);
        assert_eq!(total, expected);
    }

    // 現在選択されている SIMD 経路を表示する。
    fn print_simd_status() {
        println!("VELOVERSI_SIMD={:?}", crate::simd_preference());
        println!("movegen backend={}", crate::selected_movegen_backend());
        println!("board backend={}", crate::selected_board_backend());
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
    fn perft_error_rejects_invalid_mode() {
        // PerftError が不正なモード指定を拒否できることを確認する。
        assert_eq!(
            perft(&Board::new_initial(), 1, 3),
            Err(PerftError::InvalidMode)
        );
    }

    #[test]
    fn board_status_represents_all_game_states() {
        // BoardStatus が継続、強制パス、終局の 3 状態を区別できることを確認する。
        assert_eq!(BoardStatus::Ongoing, BoardStatus::Ongoing);
        assert_eq!(BoardStatus::ForcedPass, BoardStatus::ForcedPass);
        assert_eq!(BoardStatus::Terminal, BoardStatus::Terminal);
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
        assert_flip_implementations_match(&board, mv);
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
        assert_flip_implementations_match(&board, mv);
    }

    #[test]
    fn flips_for_move_matches_scan_implementation_for_curated_positions() {
        // table 版と旧走査版が代表局面の全合法手で一致することを確認する。
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
            for mv in legal_move_list(generate_legal_moves_naive(&board)) {
                assert_flip_implementations_match(&board, mv);
            }
        }
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
                assert_flip_implementations_match(&board, mv);
                assert_eq!(apply_move(&board, mv), apply_move_naive(&board, mv));
                assert_eq!(
                    apply_move_unchecked(&board, mv),
                    apply_move_naive(&board, mv).unwrap()
                );
            }
        }
    }

    #[test]
    fn oriented_move_and_undo_restore_original_state() {
        // 共通内部基盤の move / undo が全合法手で元の oriented 状態へ戻ることを確認する。
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
            let original = OrientedBoard::from_board(&board);
            for mv in legal_move_list(generate_legal_moves(&board)) {
                let mut oriented = original;
                let flip = oriented.calc_flip_unchecked(mv.square);
                oriented.move_board(flip);
                oriented.undo_board(flip);
                assert_eq!(oriented, original);
            }
        }
    }

    #[test]
    fn oriented_move_matches_public_apply_result() {
        // oriented 基盤の move_copy が公開 API の apply_move と同じ盤面を生成することを確認する。
        let positions = [
            Board::new_initial(),
            Board {
                black_bits: bit(square(3, 3)) | bit(square(4, 3)) | bit(square(3, 4)),
                white_bits: bit(square(4, 4)),
                side_to_move: Color::White,
            },
        ];

        for board in positions {
            let oriented = OrientedBoard::from_board(&board);
            let next_side = match board.side_to_move {
                Color::Black => Color::White,
                Color::White => Color::Black,
            };
            for mv in legal_move_list(generate_legal_moves(&board)) {
                let next = oriented
                    .move_copy(oriented.calc_flip_unchecked(mv.square))
                    .to_board(next_side);
                assert_eq!(next, apply_move(&board, mv).unwrap());
            }
        }
    }

    #[test]
    fn oriented_move_board_updates_internal_state_as_expected() {
        // move_board 後の oriented 状態が素朴実装の次局面と一致することを確認する。
        let positions = [
            Board::new_initial(),
            Board {
                black_bits: bit(square(3, 3)) | bit(square(4, 3)) | bit(square(3, 4)),
                white_bits: bit(square(4, 4)),
                side_to_move: Color::White,
            },
        ];

        for board in positions {
            for mv in legal_move_list(generate_legal_moves(&board)) {
                let expected = OrientedBoard::from_board(&apply_move_naive(&board, mv).unwrap());
                let mut oriented = OrientedBoard::from_board(&board);
                oriented.move_board(oriented.calc_flip_unchecked(mv.square));
                assert_eq!(oriented, expected);
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
                assert_flip_implementations_match(&board, mv);
                assert_eq!(apply_move(&board, mv), apply_move_naive(&board, mv));
                assert_eq!(
                    apply_move_unchecked(&board, mv),
                    apply_move_naive(&board, mv).unwrap()
                );
            }
        }
    }

    #[test]
    fn flips_for_move_matches_scan_implementation_for_deterministic_random_positions() {
        // table 版と旧走査版が決定的ランダム局面でも一致することを確認する。
        let mut seed = 0x5e87_0fd4_a9b2_c361_u64;

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

            for mv in legal_move_list(generate_legal_moves_naive(&board)) {
                assert_flip_implementations_match(&board, mv);
            }
        }
    }

    #[test]
    fn horizontal_seed_matches_flat_seed_layout() {
        // C++ 側の 4 バイト単位キャストと同じ位置を読めていることを確認する。
        let flat: Vec<u8> = BB_SEED.iter().flat_map(|row| row.iter().copied()).collect();

        for index in 0..=126usize {
            for x in 0..8usize {
                let byte_index = index * 4 + x;
                assert_eq!(horizontal_seed(index, x), flat[byte_index]);
            }
        }
    }

    #[test]
    fn read_h2vline_matches_unaligned_little_endian_view() {
        // C++ 側の u32 オフセット + u64 読み出しと同じ値を返すことを確認する。
        let bytes: Vec<u8> = BB_H2VLINE
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();

        for offset in 0..=126usize {
            let expected =
                u64::from_le_bytes(bytes[offset * 4..offset * 4 + 8].try_into().unwrap());
            assert_eq!(read_h2vline(offset), expected);
        }
    }

    #[test]
    fn board_status_returns_ongoing_when_current_side_has_legal_moves() {
        // 合法手が存在する局面を Ongoing と判定することを確認する。
        assert_eq!(board_status(&Board::new_initial()), BoardStatus::Ongoing);
    }

    #[test]
    fn board_status_returns_forced_pass_when_only_opponent_can_move() {
        // 現手番は打てず相手番のみ打てる局面を ForcedPass と判定することを確認する。
        let board = forced_pass_board();
        let opponent_board = Board {
            side_to_move: Color::White,
            ..board
        };

        assert_eq!(generate_legal_moves(&board).count, 0);
        assert!(generate_legal_moves(&opponent_board).count > 0);
        assert_eq!(board_status(&board), BoardStatus::ForcedPass);
    }

    #[test]
    fn board_status_returns_terminal_when_neither_side_can_move() {
        // 両者とも合法手がない局面を Terminal と判定することを確認する。
        let board = terminal_board();
        let opponent_board = Board {
            side_to_move: Color::White,
            ..board
        };

        assert_eq!(generate_legal_moves(&board).count, 0);
        assert_eq!(generate_legal_moves(&opponent_board).count, 0);
        assert_eq!(board_status(&board), BoardStatus::Terminal);
    }

    #[test]
    fn apply_forced_pass_flips_side_without_changing_discs() {
        // 強制パスでは石配置を変えずに手番だけ反転することを確認する。
        let board = forced_pass_board();
        let passed = apply_forced_pass(&board).unwrap();

        assert_eq!(passed.black_bits, board.black_bits);
        assert_eq!(passed.white_bits, board.white_bits);
        assert_eq!(passed.side_to_move, Color::White);
    }

    #[test]
    fn apply_forced_pass_rejects_position_with_legal_moves() {
        // 合法手がある局面では強制パスできないことを確認する。
        assert_eq!(
            apply_forced_pass(&Board::new_initial()),
            Err(MoveError::PassNotAllowed)
        );
    }

    #[test]
    fn apply_forced_pass_rejects_terminal_board() {
        // 終局局面では強制パスではなく TerminalBoard エラーを返すことを確認する。
        assert_eq!(
            apply_forced_pass(&terminal_board()),
            Err(MoveError::TerminalBoard)
        );
    }

    #[test]
    fn perft_returns_one_at_depth_zero_for_both_modes() {
        // 深さ 0 ではモードに関わらず葉 1 を返すことを確認する。
        let board = Board::new_initial();

        assert_eq!(perft(&board, 0, 1), Ok(1));
        assert_eq!(perft(&board, 0, 2), Ok(1));
    }

    #[test]
    fn perft_matches_known_values_to_depth_eight_for_mode_one() {
        // 初期局面の既知値に mode 1 が一致することを確認する。
        let board = Board::new_initial();
        let expected = [1_u64, 4, 12, 56, 244, 1396, 8200, 55092, 390216];

        for (depth, expected_nodes) in expected.iter().enumerate() {
            assert_eq!(perft(&board, depth as u8, 1), Ok(*expected_nodes));
        }
    }

    #[test]
    fn perft_matches_known_values_to_depth_eight_for_mode_two() {
        // 初期局面の既知値に mode 2 が一致することを確認する。
        let board = Board::new_initial();
        let expected = [1_u64, 4, 12, 56, 244, 1396, 8200, 55092, 390216];

        for (depth, expected_nodes) in expected.iter().enumerate() {
            assert_eq!(perft(&board, depth as u8, 2), Ok(*expected_nodes));
        }
    }

    #[test]
    fn perft_counts_forced_pass_differently_by_mode() {
        // 強制パスを含む局面で mode 1 と mode 2 の深さ消費差が出ることを確認する。
        let board = perft_forced_pass_difference_board();

        assert_eq!(board_status(&board), BoardStatus::ForcedPass);
        assert_eq!(perft(&board, 1, 1), Ok(1));
        assert_eq!(perft(&board, 1, 2), Ok(5));
        assert_ne!(perft(&board, 1, 1), perft(&board, 1, 2));
    }

    #[test]
    fn internal_perft_matches_naive_oracle_for_curated_positions() {
        // 内部 Perft 再帰が代表局面群で素朴実装と一致することを確認する。
        let positions = [
            Board::new_initial(),
            perft_forced_pass_difference_board(),
            Board {
                black_bits: bit(square(3, 3)) | bit(square(4, 3)) | bit(square(3, 4)),
                white_bits: bit(square(4, 4)),
                side_to_move: Color::White,
            },
        ];

        for board in positions {
            for depth in 0..=4 {
                let mut oriented = OrientedBoard::from_board(&board);
                assert_eq!(
                    perft_with_mode_oriented(&mut oriented, depth, PerftMode::Mode1, false),
                    perft_naive(&board, depth, PerftMode::Mode1, false)
                );

                let mut oriented = OrientedBoard::from_board(&board);
                assert_eq!(
                    perft_with_mode_oriented(&mut oriented, depth, PerftMode::Mode2, false),
                    perft_naive(&board, depth, PerftMode::Mode2, false)
                );
            }
        }
    }

    #[test]
    fn internal_perft_matches_naive_oracle_for_deterministic_random_positions() {
        // 内部 Perft 再帰がさまざまな局面で素朴実装と一致することを確認する。
        let mut seed = 0x4d31_8ab7_c205_f19e_u64;

        for _ in 0..64 {
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

            for depth in 0..=3 {
                let mut oriented = OrientedBoard::from_board(&board);
                assert_eq!(
                    perft_with_mode_oriented(&mut oriented, depth, PerftMode::Mode1, false),
                    perft_naive(&board, depth, PerftMode::Mode1, false)
                );

                let mut oriented = OrientedBoard::from_board(&board);
                assert_eq!(
                    perft_with_mode_oriented(&mut oriented, depth, PerftMode::Mode2, false),
                    perft_naive(&board, depth, PerftMode::Mode2, false)
                );
            }
        }
    }

    #[test]
    fn internal_perft_depth_two_matches_terminal_and_forced_pass_examples() {
        // depth 2 の終局分岐と強制パス分岐が素朴実装と一致することを確認する。
        let (terminal_board, terminal_move, forced_board, forced_move) =
            find_depth_two_branch_examples();

        let terminal_next = apply_move_naive(&terminal_board, terminal_move).unwrap();
        assert_eq!(generate_legal_moves(&terminal_next).count, 0);
        assert_eq!(
            generate_legal_moves(&board_with_side_to_move(
                &terminal_next,
                match terminal_next.side_to_move {
                    Color::Black => Color::White,
                    Color::White => Color::Black,
                },
            ))
            .count,
            0
        );

        let forced_next = apply_move_naive(&forced_board, forced_move).unwrap();
        assert_eq!(generate_legal_moves(&forced_next).count, 0);
        assert!(
            generate_legal_moves(&board_with_side_to_move(
                &forced_next,
                match forced_next.side_to_move {
                    Color::Black => Color::White,
                    Color::White => Color::Black,
                },
            ))
            .count
                > 0
        );

        let mut oriented = OrientedBoard::from_board(&terminal_board);
        assert_eq!(
            perft_with_mode_oriented(&mut oriented, 2, PerftMode::Mode1, false),
            perft_naive(&terminal_board, 2, PerftMode::Mode1, false)
        );
        let mut oriented = OrientedBoard::from_board(&terminal_board);
        assert_eq!(
            perft_with_mode_oriented(&mut oriented, 2, PerftMode::Mode2, false),
            perft_naive(&terminal_board, 2, PerftMode::Mode2, false)
        );

        let mut oriented = OrientedBoard::from_board(&forced_board);
        assert_eq!(
            perft_with_mode_oriented(&mut oriented, 2, PerftMode::Mode1, false),
            perft_naive(&forced_board, 2, PerftMode::Mode1, false)
        );
        let mut oriented = OrientedBoard::from_board(&forced_board);
        assert_eq!(
            perft_with_mode_oriented(&mut oriented, 2, PerftMode::Mode2, false),
            perft_naive(&forced_board, 2, PerftMode::Mode2, false)
        );
    }

    #[test]
    fn internal_perft_depth_three_matches_terminal_and_forced_pass_examples() {
        // depth 3 の終局分岐と強制パス分岐が素朴実装と一致することを確認する。
        let (terminal_board, forced_board) = find_depth_three_branch_examples();

        let mut oriented = OrientedBoard::from_board(&terminal_board);
        assert_eq!(
            perft_with_mode_oriented(&mut oriented, 3, PerftMode::Mode1, false),
            perft_naive(&terminal_board, 3, PerftMode::Mode1, false)
        );
        let mut oriented = OrientedBoard::from_board(&terminal_board);
        assert_eq!(
            perft_with_mode_oriented(&mut oriented, 3, PerftMode::Mode2, false),
            perft_naive(&terminal_board, 3, PerftMode::Mode2, false)
        );

        let mut oriented = OrientedBoard::from_board(&forced_board);
        assert_eq!(
            perft_with_mode_oriented(&mut oriented, 3, PerftMode::Mode1, false),
            perft_naive(&forced_board, 3, PerftMode::Mode1, false)
        );
        let mut oriented = OrientedBoard::from_board(&forced_board);
        assert_eq!(
            perft_with_mode_oriented(&mut oriented, 3, PerftMode::Mode2, false),
            perft_naive(&forced_board, 3, PerftMode::Mode2, false)
        );
    }

    #[test]
    fn perft_returns_one_for_terminal_board_regardless_of_depth_or_mode() {
        // 終局局面では深さとモードに関わらず葉 1 を返すことを確認する。
        let board = terminal_board();

        assert_eq!(perft(&board, 1, 1), Ok(1));
        assert_eq!(perft(&board, 1, 2), Ok(1));
        assert_eq!(perft(&board, 8, 1), Ok(1));
        assert_eq!(perft(&board, 8, 2), Ok(1));
    }

    #[test]
    #[ignore = "long-running perft verification"]
    fn perft_long_initial_position_mode_one_to_depth_fifteen() {
        // 初期局面の mode 1 既知値を深さ 9 から 15 まで確認する。
        print_simd_status();
        let expected = [
            (9_u8, 3005288_u64),
            (10, 24571284),
            (11, 212258800),
            (12, 1939886636),
            (13, 18429641748),
            (14, 184042084512),
            (15, 1891832540064),
        ];

        for (depth, expected_nodes) in expected {
            assert_perft_long_with_progress(depth, 1, expected_nodes);
        }
    }

    #[test]
    #[ignore = "long-running perft verification"]
    fn perft_long_initial_position_mode_two_to_depth_fifteen() {
        // 初期局面の mode 2 既知値を深さ 9 から 15 まで確認する。
        print_simd_status();
        let expected = [
            (9_u8, 3005320_u64),
            (10, 24571420),
            (11, 212260880),
            (12, 1939899208),
            (13, 18429791868),
            (14, 184043158384),
            (15, 1891845643044),
        ];

        for (depth, expected_nodes) in expected {
            assert_perft_long_with_progress(depth, 2, expected_nodes);
        }
    }

    #[test]
    #[ignore = "simd benchmark helper"]
    fn perft_bench_initial_position_mode_one_to_depth_thirteen() {
        // SIMD 実装比較用に、mode 1 の深さ 12 と 13 を進捗付きで確認する。
        print_simd_status();
        assert_perft_long_with_progress(12, 1, 1_939_886_636);
        assert_perft_long_with_progress(13, 1, 18_429_641_748);
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
