mod flip_tables;

use flip_tables::{BB_DLINE02, BB_DLINE57, BB_FLIPPED, BB_H2VLINE, BB_MUL16, BB_SEED, BB_VLINE};
use pyo3::prelude::*;
use smallvec::SmallVec;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::{
    __m128i, __m256i, _mm_cvtsi64_si128, _mm_cvtsi128_si64, _mm_or_si128, _mm_set_epi64x,
    _mm_set1_epi64x, _mm_shuffle_epi32, _mm_unpackhi_epi64, _mm_xor_si128, _mm256_add_epi64,
    _mm256_and_si256, _mm256_andnot_si256, _mm256_broadcastq_epi64, _mm256_castsi256_si128,
    _mm256_cmpeq_epi64, _mm256_extracti128_si256, _mm256_or_si256, _mm256_set_epi64x,
    _mm256_setzero_si256, _mm256_sllv_epi64, _mm256_srlv_epi64, _mm256_sub_epi64,
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
    OverlappingDiscs,
}

// 着手位置 1 マスを表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub square: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameResult {
    BlackWin,
    WhiteWin,
    Draw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiscCount {
    pub black: u8,
    pub white: u8,
    pub empty: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackedBoard {
    pub black_bits: u64,
    pub white_bits: u64,
    pub side_to_move: Color,
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PositionSamplingConfig {
    pub num_positions: u32,
    pub min_plies: u16,
    pub max_plies: u16,
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MovegenBackend {
    Generic,
    Avx2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BoardBackend {
    Generic,
    Sse2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FlipBackend {
    Generic,
    Avx2,
}

static SIMD_PREFERENCE: OnceLock<SimdPreference> = OnceLock::new();
static MOVEGEN_BACKEND: OnceLock<MovegenBackend> = OnceLock::new();
static BOARD_BACKEND: OnceLock<BoardBackend> = OnceLock::new();
static FLIP_BACKEND: OnceLock<FlipBackend> = OnceLock::new();

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
#[cfg(target_arch = "x86_64")]
static FLIP_MASKS: OnceLock<[FlipMasks; 64]> = OnceLock::new();
#[cfg(target_arch = "x86_64")]
static AVX2_MOBILITY_CONSTANTS: OnceLock<Avx2MobilityConstants> = OnceLock::new();
#[cfg(target_arch = "x86_64")]
static AVX2_FLIP_CONSTANTS: OnceLock<Avx2FlipConstants> = OnceLock::new();

fn parse_simd_preference(raw: Option<&str>) -> SimdPreference {
    match raw.map(str::to_ascii_lowercase).as_deref() {
        Some("generic") => SimdPreference::Generic,
        Some("sse2") => SimdPreference::Sse2,
        Some("avx2") => SimdPreference::Avx2,
        _ => SimdPreference::Auto,
    }
}

#[cfg(target_arch = "x86_64")]
fn resolve_movegen_backend(preference: SimdPreference, has_avx2: bool) -> MovegenBackend {
    match preference {
        SimdPreference::Generic | SimdPreference::Sse2 => MovegenBackend::Generic,
        SimdPreference::Avx2 => {
            assert!(
                has_avx2,
                "VELOVERSI_SIMD=avx2 が指定されましたが、この CPU は avx2 非対応です"
            );
            MovegenBackend::Avx2
        }
        SimdPreference::Auto => {
            if has_avx2 {
                MovegenBackend::Avx2
            } else {
                MovegenBackend::Generic
            }
        }
    }
}

#[cfg(target_arch = "x86_64")]
fn resolve_board_backend(preference: SimdPreference) -> BoardBackend {
    match preference {
        SimdPreference::Generic => BoardBackend::Generic,
        SimdPreference::Sse2 | SimdPreference::Avx2 | SimdPreference::Auto => BoardBackend::Sse2,
    }
}

#[cfg(target_arch = "x86_64")]
fn resolve_flip_backend(preference: SimdPreference, has_avx2: bool) -> FlipBackend {
    match preference {
        SimdPreference::Generic | SimdPreference::Sse2 => FlipBackend::Generic,
        SimdPreference::Avx2 => {
            assert!(
                has_avx2,
                "VELOVERSI_SIMD=avx2 が指定されましたが、この CPU は avx2 非対応です"
            );
            FlipBackend::Avx2
        }
        SimdPreference::Auto => {
            if has_avx2 {
                FlipBackend::Avx2
            } else {
                FlipBackend::Generic
            }
        }
    }
}

// 環境変数から SIMD 経路の強制指定を読み取る。
fn simd_preference() -> SimdPreference {
    *SIMD_PREFERENCE
        .get_or_init(|| parse_simd_preference(std::env::var("VELOVERSI_SIMD").ok().as_deref()))
}

#[cfg(target_arch = "x86_64")]
fn selected_movegen_backend_kind() -> MovegenBackend {
    *MOVEGEN_BACKEND.get_or_init(|| {
        resolve_movegen_backend(
            simd_preference(),
            std::arch::is_x86_feature_detected!("avx2"),
        )
    })
}

#[cfg(not(target_arch = "x86_64"))]
fn selected_movegen_backend_kind() -> MovegenBackend {
    MovegenBackend::Generic
}

#[cfg(target_arch = "x86_64")]
fn selected_board_backend_kind() -> BoardBackend {
    *BOARD_BACKEND.get_or_init(|| resolve_board_backend(simd_preference()))
}

#[cfg(not(target_arch = "x86_64"))]
fn selected_board_backend_kind() -> BoardBackend {
    BoardBackend::Generic
}

#[cfg(target_arch = "x86_64")]
fn selected_flip_backend_kind() -> FlipBackend {
    *FLIP_BACKEND.get_or_init(|| {
        resolve_flip_backend(
            simd_preference(),
            std::arch::is_x86_feature_detected!("avx2"),
        )
    })
}

#[cfg(not(target_arch = "x86_64"))]
fn selected_flip_backend_kind() -> FlipBackend {
    FlipBackend::Generic
}

// 合法手生成で使う実装経路名を返す。
#[cfg(test)]
fn selected_movegen_backend() -> &'static str {
    match selected_movegen_backend_kind() {
        MovegenBackend::Generic => "generic",
        MovegenBackend::Avx2 => "avx2",
    }
}

// 盤面更新で使う実装経路名を返す。
#[cfg(test)]
fn selected_board_backend() -> &'static str {
    match selected_board_backend_kind() {
        BoardBackend::Generic => "generic",
        BoardBackend::Sse2 => "sse2",
    }
}

// 反転計算で使う実装経路名を返す。
#[cfg(test)]
fn selected_flip_backend() -> &'static str {
    match selected_flip_backend_kind() {
        FlipBackend::Generic => "generic",
        FlipBackend::Avx2 => "avx2",
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
            return Err(BoardError::OverlappingDiscs);
        }
        Ok(())
    }
}

// 盤面を固定長の軽量表現へ変換する。
pub fn pack_board(board: &Board) -> PackedBoard {
    PackedBoard {
        black_bits: board.black_bits,
        white_bits: board.white_bits,
        side_to_move: board.side_to_move,
    }
}

// 固定長表現から盤面を復元する。
pub fn unpack_board(packed: PackedBoard) -> Result<Board, BoardError> {
    Board::from_bits(packed.black_bits, packed.white_bits, packed.side_to_move)
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

// 利用可能な対称変換を固定順で返す。
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

// 対称変換後のマス対応を返す。
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

// 盤面に対して対称変換を適用した盤面を返す。手番は保持する。
pub fn transform_board(board: &Board, sym: Symmetry) -> Board {
    Board {
        black_bits: transform_bits(board.black_bits, sym),
        white_bits: transform_bits(board.white_bits, sym),
        side_to_move: board.side_to_move,
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

#[cfg(target_arch = "x86_64")]
#[derive(Clone, Copy, Debug, Default)]
struct FlipMasks {
    left: [u64; 4],
    right: [u64; 4],
}

#[cfg(target_arch = "x86_64")]
#[derive(Clone, Copy)]
struct Avx2MobilityConstants {
    shifts: __m256i,
    horizontal_mask: __m256i,
}

#[cfg(target_arch = "x86_64")]
#[derive(Clone, Copy)]
struct Avx2FlipConstants {
    shifts: __m256i,
    shifts2: __m256i,
    shifts4: __m256i,
    zero: __m256i,
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
            if selected_board_backend_kind() == BoardBackend::Sse2 {
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
            if selected_board_backend_kind() == BoardBackend::Sse2 {
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
            if selected_board_backend_kind() == BoardBackend::Sse2 {
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
        if selected_movegen_backend_kind() == MovegenBackend::Avx2 {
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
    let constants = avx2_mobility_constants();
    let shifts = constants.shifts;
    let shift_twice: __m256i = _mm256_add_epi64(shifts, shifts);
    let player_vec = _mm256_broadcastq_epi64(_mm_cvtsi64_si128(player_bits as i64));
    let opponent_vec = _mm256_and_si256(
        _mm256_broadcastq_epi64(_mm_cvtsi64_si128(opponent_bits as i64)),
        constants.horizontal_mask,
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

#[cfg(target_arch = "x86_64")]
fn init_flip_masks() -> [FlipMasks; 64] {
    let mut masks = [FlipMasks::default(); 64];

    for x in 0..8 {
        let mut left = [
            (0x0102040810204080_u64 >> ((7 - x) * 8)) & 0xffff_ffff_ffff_ff00,
            (0x8040201008040201_u64 >> (x * 8)) & 0xffff_ffff_ffff_ff00,
            (0x0101010101010101_u64 << x) & 0xffff_ffff_ffff_ff00,
            ((0xfe_u64 << x) & 0xff),
        ];
        let mut right = [
            (0x0102040810204080_u64 << (x * 8)) & 0x00ff_ffff_ffff_ffff,
            (0x8040201008040201_u64 << ((7 - x) * 8)) & 0x00ff_ffff_ffff_ffff,
            (0x0101010101010101_u64 << x) & 0x00ff_ffff_ffff_ffff,
            (u64::from(0x7f_u8 >> (7 - x))) << 56,
        ];

        for y in 0..8 {
            masks[y * 8 + x].left = left;
            masks[(7 - y) * 8 + x].right = right;

            for lane in &mut left {
                *lane <<= 8;
            }
            for lane in &mut right {
                *lane >>= 8;
            }
        }
    }

    masks
}

#[cfg(target_arch = "x86_64")]
fn flip_masks(square: u8) -> &'static FlipMasks {
    &FLIP_MASKS.get_or_init(init_flip_masks)[square as usize]
}

#[cfg(target_arch = "x86_64")]
fn avx2_mobility_constants() -> &'static Avx2MobilityConstants {
    AVX2_MOBILITY_CONSTANTS.get_or_init(|| Avx2MobilityConstants {
        shifts: unsafe { _mm256_set_epi64x(7, 9, 8, 1) },
        horizontal_mask: unsafe {
            _mm256_set_epi64x(
                0x7e7e7e7e7e7e7e7e_u64 as i64,
                0x7e7e7e7e7e7e7e7e_u64 as i64,
                -1,
                0x7e7e7e7e7e7e7e7e_u64 as i64,
            )
        },
    })
}

#[cfg(target_arch = "x86_64")]
fn avx2_flip_constants() -> &'static Avx2FlipConstants {
    AVX2_FLIP_CONSTANTS.get_or_init(|| Avx2FlipConstants {
        shifts: unsafe { _mm256_set_epi64x(7, 9, 8, 1) },
        shifts2: unsafe { _mm256_set_epi64x(14, 18, 16, 2) },
        shifts4: unsafe { _mm256_set_epi64x(28, 36, 32, 4) },
        zero: unsafe { _mm256_setzero_si256() },
    })
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
    #[cfg(target_arch = "x86_64")]
    {
        if selected_flip_backend_kind() == FlipBackend::Avx2 {
            return unsafe { flips_for_move_bits_avx2(player_bits, opponent_bits, square) };
        }
    }

    flips_for_move_bits_unchecked_generic(player_bits, opponent_bits, square)
}

#[inline(always)]
fn flips_for_move_bits_unchecked_generic(player_bits: u64, opponent_bits: u64, square: u8) -> u64 {
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

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn flips_for_move_bits_avx2(player_bits: u64, opponent_bits: u64, square: u8) -> u64 {
    let constants = avx2_flip_constants();
    let shifts = constants.shifts;
    let shifts2 = constants.shifts2;
    let shifts4 = constants.shifts4;
    let mask = flip_masks(square);
    let player = _mm256_broadcastq_epi64(_mm_cvtsi64_si128(player_bits as i64));
    let opponent = _mm256_broadcastq_epi64(_mm_cvtsi64_si128(opponent_bits as i64));

    let right_mask = _mm256_set_epi64x(
        mask.right[0] as i64,
        mask.right[1] as i64,
        mask.right[2] as i64,
        mask.right[3] as i64,
    );
    let mut eraser = _mm256_andnot_si256(opponent, right_mask);
    let mut right = _mm256_sllv_epi64(_mm256_and_si256(player, right_mask), shifts);
    eraser = _mm256_or_si256(eraser, _mm256_srlv_epi64(eraser, shifts));
    right = _mm256_andnot_si256(eraser, right);
    right = _mm256_andnot_si256(_mm256_srlv_epi64(eraser, shifts2), right);
    right = _mm256_andnot_si256(_mm256_srlv_epi64(eraser, shifts4), right);
    let mut flips = _mm256_and_si256(right_mask, _mm256_sub_epi64(constants.zero, right));

    let left_mask = _mm256_set_epi64x(
        mask.left[0] as i64,
        mask.left[1] as i64,
        mask.left[2] as i64,
        mask.left[3] as i64,
    );
    let mut left = _mm256_andnot_si256(opponent, left_mask);
    left = _mm256_and_si256(left, _mm256_sub_epi64(constants.zero, left));
    left = _mm256_and_si256(left, player);
    let left_fill = _mm256_sub_epi64(_mm256_cmpeq_epi64(left, constants.zero), left);
    flips = _mm256_or_si256(flips, _mm256_andnot_si256(left_fill, left_mask));

    let halves = _mm_or_si128(
        _mm256_castsi256_si128(flips),
        _mm256_extracti128_si256(flips, 1),
    );
    let merged = _mm_or_si128(halves, _mm_shuffle_epi32(halves, 0x4e));
    _mm_cvtsi128_si64(merged) as u64
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

// 指定した手が合法かを返す。
pub fn is_legal_move(board: &Board, mv: Move) -> bool {
    if mv.square >= 64 {
        return false;
    }
    generate_legal_moves(board).bitmask & (1u64 << mv.square) != 0
}

// 合法手ビットマスクを盤面インデックス昇順の配列へ変換する。
pub fn legal_moves_to_vec(legal: LegalMoves) -> SmallVec<[Move; 32]> {
    let mut moves = SmallVec::<[Move; 32]>::new();
    let mut bitmask = legal.bitmask;
    while bitmask != 0 {
        let square = bitmask.trailing_zeros() as u8;
        moves.push(Move { square });
        bitmask &= bitmask - 1;
    }
    moves
}

// 現局面の石数を返す。
pub fn disc_count(board: &Board) -> DiscCount {
    let black = board.black_bits.count_ones() as u8;
    let white = board.white_bits.count_ones() as u8;
    DiscCount {
        black,
        white,
        empty: 64 - black - white,
    }
}

// 常に黒視点の石差を返す。終局局面では最終石差として解釈できる。
pub fn final_margin_from_black(board: &Board) -> i8 {
    let counts = disc_count(board);
    counts.black as i8 - counts.white as i8
}

// 常に現手番視点の石差を返す。終局局面では最終石差として解釈できる。
pub fn final_margin_from_side_to_move(board: &Board) -> i8 {
    match board.side_to_move {
        Color::Black => final_margin_from_black(board),
        Color::White => -final_margin_from_black(board),
    }
}

// 常に現在局面の石数比較から勝敗を返す。終局局面では最終結果として解釈できる。
pub fn game_result(board: &Board) -> GameResult {
    match final_margin_from_black(board).cmp(&0) {
        std::cmp::Ordering::Greater => GameResult::BlackWin,
        std::cmp::Ordering::Less => GameResult::WhiteWin,
        std::cmp::Ordering::Equal => GameResult::Draw,
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

#[pyclass(name = "Board")]
#[derive(Clone, Copy)]
struct PyBoard {
    inner: Board,
}

fn color_to_py_str(color: Color) -> &'static str {
    match color {
        Color::Black => "black",
        Color::White => "white",
    }
}

fn board_status_to_py_str(status: BoardStatus) -> &'static str {
    match status {
        BoardStatus::Ongoing => "ongoing",
        BoardStatus::ForcedPass => "forced_pass",
        BoardStatus::Terminal => "terminal",
    }
}

fn game_result_to_py_str(result: GameResult) -> &'static str {
    match result {
        GameResult::BlackWin => "black_win",
        GameResult::WhiteWin => "white_win",
        GameResult::Draw => "draw",
    }
}

fn symmetry_to_py_str(sym: Symmetry) -> &'static str {
    match sym {
        Symmetry::Identity => "identity",
        Symmetry::Rot90 => "rot90",
        Symmetry::Rot180 => "rot180",
        Symmetry::Rot270 => "rot270",
        Symmetry::FlipHorizontal => "flip_horizontal",
        Symmetry::FlipVertical => "flip_vertical",
        Symmetry::FlipDiag => "flip_diag",
        Symmetry::FlipAntiDiag => "flip_anti_diag",
    }
}

fn py_str_to_symmetry(value: &str) -> PyResult<Symmetry> {
    match value {
        "identity" => Ok(Symmetry::Identity),
        "rot90" => Ok(Symmetry::Rot90),
        "rot180" => Ok(Symmetry::Rot180),
        "rot270" => Ok(Symmetry::Rot270),
        "flip_horizontal" => Ok(Symmetry::FlipHorizontal),
        "flip_vertical" => Ok(Symmetry::FlipVertical),
        "flip_diag" => Ok(Symmetry::FlipDiag),
        "flip_anti_diag" => Ok(Symmetry::FlipAntiDiag),
        _ => Err(pyo3::exceptions::PyValueError::new_err(
            "unknown symmetry name",
        )),
    }
}

fn py_str_to_color(value: &str) -> PyResult<Color> {
    match value.to_ascii_lowercase().as_str() {
        "black" => Ok(Color::Black),
        "white" => Ok(Color::White),
        _ => Err(pyo3::exceptions::PyValueError::new_err(
            "side_to_move must be 'black' or 'white'",
        )),
    }
}

#[cfg(not(any(test, coverage)))]
fn packed_board_to_py_tuple(packed: PackedBoard) -> (u64, u64, &'static str) {
    (
        packed.black_bits,
        packed.white_bits,
        color_to_py_str(packed.side_to_move),
    )
}

#[cfg(not(any(test, coverage)))]
fn board_to_py_tuple(board: &Board) -> (u64, u64, &'static str) {
    (
        board.black_bits,
        board.white_bits,
        color_to_py_str(board.side_to_move),
    )
}

#[cfg(not(any(test, coverage)))]
fn move_to_py_option_square(mv: Option<Move>) -> Option<u8> {
    mv.map(|mv| mv.square)
}

#[cfg(not(any(test, coverage)))]
type PyBoardBits = (u64, u64, &'static str);

#[cfg(not(any(test, coverage)))]
type PyRandomGameParts = (
    Vec<PyBoardBits>,
    Vec<Option<u8>>,
    &'static str,
    i8,
    u16,
    bool,
);

#[pymethods]
impl PyBoard {
    #[new]
    fn py_new(black_bits: u64, white_bits: u64, side_to_move: &str) -> PyResult<Self> {
        let color = py_str_to_color(side_to_move)?;
        let inner = Board::from_bits(black_bits, white_bits, color)
            .map_err(|_| pyo3::exceptions::PyValueError::new_err("invalid board bits"))?;
        Ok(Self { inner })
    }

    #[staticmethod]
    fn new_initial() -> Self {
        Self {
            inner: Board::new_initial(),
        }
    }

    #[getter]
    fn black_bits(&self) -> u64 {
        self.inner.black_bits
    }

    #[getter]
    fn white_bits(&self) -> u64 {
        self.inner.white_bits
    }

    #[getter]
    fn side_to_move(&self) -> &'static str {
        color_to_py_str(self.inner.side_to_move)
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_bits(&self) -> (u64, u64, &'static str) {
        (
            self.inner.black_bits,
            self.inner.white_bits,
            color_to_py_str(self.inner.side_to_move),
        )
    }
}

#[pyfunction]
fn initial_board() -> PyBoard {
    PyBoard::new_initial()
}

#[pyfunction]
fn board_from_bits(black_bits: u64, white_bits: u64, side_to_move: &str) -> PyResult<PyBoard> {
    PyBoard::py_new(black_bits, white_bits, side_to_move)
}

#[pyfunction(name = "pack_board")]
#[cfg(not(any(test, coverage)))]
fn pack_board_py(board: &PyBoard) -> (u64, u64, &'static str) {
    packed_board_to_py_tuple(pack_board(&board.inner))
}

#[pyfunction(name = "_unpack_board_parts")]
#[cfg(not(any(test, coverage)))]
fn unpack_board_parts_py(
    black_bits: u64,
    white_bits: u64,
    side_to_move: &str,
) -> PyResult<PyBoard> {
    let packed = PackedBoard {
        black_bits,
        white_bits,
        side_to_move: py_str_to_color(side_to_move)?,
    };
    unpack_board(packed)
        .map(|inner| PyBoard { inner })
        .map_err(|_| pyo3::exceptions::PyValueError::new_err("invalid board bits"))
}

#[pyfunction(name = "_play_random_game_parts")]
#[cfg(not(any(test, coverage)))]
fn play_random_game_parts_py(seed: u64, max_plies: Option<u16>) -> PyRandomGameParts {
    let trace = play_random_game(seed, &RandomPlayConfig { max_plies });
    (
        trace.boards.iter().map(board_to_py_tuple).collect(),
        trace
            .moves
            .iter()
            .copied()
            .map(move_to_py_option_square)
            .collect(),
        game_result_to_py_str(trace.final_result),
        trace.final_margin_from_black,
        trace.plies_played,
        trace.reached_terminal,
    )
}

#[pyfunction(name = "_sample_reachable_positions_parts")]
#[cfg(not(any(test, coverage)))]
fn sample_reachable_positions_parts_py(
    seed: u64,
    num_positions: u32,
    min_plies: u16,
    max_plies: u16,
) -> Vec<PyBoardBits> {
    sample_reachable_positions(
        seed,
        &PositionSamplingConfig {
            num_positions,
            min_plies,
            max_plies,
        },
    )
    .iter()
    .map(board_to_py_tuple)
    .collect()
}

#[pyfunction(name = "generate_legal_moves")]
fn generate_legal_moves_py(board: &PyBoard) -> u64 {
    generate_legal_moves(&board.inner).bitmask
}

#[pyfunction]
fn validate_board(board: &PyBoard) -> PyResult<()> {
    board
        .inner
        .validate()
        .map_err(|_| pyo3::exceptions::PyValueError::new_err("invalid board bits"))
}

#[pyfunction]
fn legal_moves_list(board: &PyBoard) -> Vec<u32> {
    legal_moves_to_vec(generate_legal_moves(&board.inner))
        .into_iter()
        .map(|mv| mv.square as u32)
        .collect()
}

#[pyfunction(name = "is_legal_move")]
fn is_legal_move_py(board: &PyBoard, square: u8) -> bool {
    is_legal_move(&board.inner, Move { square })
}

#[pyfunction(name = "apply_move")]
fn apply_move_py(board: &PyBoard, square: u8) -> PyResult<PyBoard> {
    apply_move(&board.inner, Move { square })
        .map(|inner| PyBoard { inner })
        .map_err(|err| pyo3::exceptions::PyValueError::new_err(format!("{err:?}")))
}

#[pyfunction(name = "apply_forced_pass")]
fn apply_forced_pass_py(board: &PyBoard) -> PyResult<PyBoard> {
    apply_forced_pass(&board.inner)
        .map(|inner| PyBoard { inner })
        .map_err(|err| pyo3::exceptions::PyValueError::new_err(format!("{err:?}")))
}

#[pyfunction(name = "board_status")]
fn board_status_py(board: &PyBoard) -> &'static str {
    board_status_to_py_str(board_status(&board.inner))
}

#[pyfunction(name = "disc_count")]
fn disc_count_py(board: &PyBoard) -> (u8, u8, u8) {
    let counts = disc_count(&board.inner);
    (counts.black, counts.white, counts.empty)
}

#[pyfunction(name = "game_result")]
fn game_result_py(board: &PyBoard) -> &'static str {
    game_result_to_py_str(game_result(&board.inner))
}

#[pyfunction(name = "final_margin_from_black")]
fn final_margin_from_black_py(board: &PyBoard) -> i8 {
    final_margin_from_black(&board.inner)
}

#[pyfunction(name = "transform_board")]
fn transform_board_py(board: &PyBoard, sym: &str) -> PyResult<PyBoard> {
    Ok(PyBoard {
        inner: transform_board(&board.inner, py_str_to_symmetry(sym)?),
    })
}

#[pyfunction(name = "transform_square")]
fn transform_square_py(square: u8, sym: &str) -> PyResult<u8> {
    if square >= 64 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "square must be in 0..64",
        ));
    }
    Ok(transform_square(square, py_str_to_symmetry(sym)?))
}

#[pyfunction(name = "all_symmetries")]
fn all_symmetries_py() -> Vec<&'static str> {
    all_symmetries()
        .into_iter()
        .map(symmetry_to_py_str)
        .collect()
}

// Python 拡張モジュールのエントリポイント。
#[pymodule]
fn _core(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyBoard>()?;
    module.add_function(wrap_pyfunction!(initial_board, module)?)?;
    module.add_function(wrap_pyfunction!(board_from_bits, module)?)?;
    #[cfg(not(any(test, coverage)))]
    module.add_function(wrap_pyfunction!(pack_board_py, module)?)?;
    #[cfg(not(any(test, coverage)))]
    module.add_function(wrap_pyfunction!(unpack_board_parts_py, module)?)?;
    #[cfg(not(any(test, coverage)))]
    module.add_function(wrap_pyfunction!(play_random_game_parts_py, module)?)?;
    #[cfg(not(any(test, coverage)))]
    module.add_function(wrap_pyfunction!(
        sample_reachable_positions_parts_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(validate_board, module)?)?;
    module.add_function(wrap_pyfunction!(generate_legal_moves_py, module)?)?;
    module.add_function(wrap_pyfunction!(legal_moves_list, module)?)?;
    module.add_function(wrap_pyfunction!(is_legal_move_py, module)?)?;
    module.add_function(wrap_pyfunction!(apply_move_py, module)?)?;
    module.add_function(wrap_pyfunction!(apply_forced_pass_py, module)?)?;
    module.add_function(wrap_pyfunction!(board_status_py, module)?)?;
    module.add_function(wrap_pyfunction!(disc_count_py, module)?)?;
    module.add_function(wrap_pyfunction!(game_result_py, module)?)?;
    module.add_function(wrap_pyfunction!(final_margin_from_black_py, module)?)?;
    module.add_function(wrap_pyfunction!(transform_board_py, module)?)?;
    module.add_function(wrap_pyfunction!(transform_square_py, module)?)?;
    module.add_function(wrap_pyfunction!(all_symmetries_py, module)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        BB_H2VLINE, BB_SEED, Board, BoardBackend, BoardError, BoardStatus, Color, D4, D5,
        DiscCount, E4, E5, FlipBackend, GameResult, LegalMoves, Move, MoveError, MovegenBackend,
        OrientedBoard, PackedBoard, PerftError, PerftMode, PositionSamplingConfig,
        RandomPlayConfig, SimdPreference, Symmetry, XorShift64Star, all_symmetries,
        apply_forced_pass, apply_move, apply_move_unchecked, board_status, board_with_side_to_move,
        disc_count, final_margin_from_black, final_margin_from_side_to_move, flips_for_move,
        flips_for_move_bits_scan, game_result, generate_legal_moves, horizontal_seed,
        is_legal_move, legal_moves_bitmask_generic, legal_moves_to_vec, pack_board,
        parse_simd_preference, perft, perft_with_mode_oriented, play_random_game,
        play_random_game_from_board_with_rng, read_h2vline, sample_reachable_positions,
        selected_flip_backend, transform_board, transform_square, unpack_board,
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
        println!("flip backend={}", crate::selected_flip_backend());
        println!("board backend={}", crate::selected_board_backend());
    }

    #[test]
    fn parse_simd_preference_maps_known_and_unknown_values() {
        assert_eq!(parse_simd_preference(None), SimdPreference::Auto);
        assert_eq!(
            parse_simd_preference(Some("generic")),
            SimdPreference::Generic
        );
        assert_eq!(parse_simd_preference(Some("sse2")), SimdPreference::Sse2);
        assert_eq!(parse_simd_preference(Some("avx2")), SimdPreference::Avx2);
        assert_eq!(parse_simd_preference(Some("AUTO")), SimdPreference::Auto);
        assert_eq!(parse_simd_preference(Some("unknown")), SimdPreference::Auto);
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn resolve_backend_selection_matches_documented_matrix() {
        assert_eq!(
            crate::resolve_movegen_backend(SimdPreference::Generic, false),
            MovegenBackend::Generic
        );
        assert_eq!(
            crate::resolve_movegen_backend(SimdPreference::Sse2, false),
            MovegenBackend::Generic
        );
        assert_eq!(
            crate::resolve_movegen_backend(SimdPreference::Auto, false),
            MovegenBackend::Generic
        );
        assert_eq!(
            crate::resolve_movegen_backend(SimdPreference::Auto, true),
            MovegenBackend::Avx2
        );
        assert_eq!(
            crate::resolve_board_backend(SimdPreference::Generic),
            BoardBackend::Generic
        );
        assert_eq!(
            crate::resolve_board_backend(SimdPreference::Sse2),
            BoardBackend::Sse2
        );
        assert_eq!(
            crate::resolve_board_backend(SimdPreference::Avx2),
            BoardBackend::Sse2
        );
        assert_eq!(
            crate::resolve_board_backend(SimdPreference::Auto),
            BoardBackend::Sse2
        );
        assert_eq!(
            crate::resolve_flip_backend(SimdPreference::Generic, false),
            FlipBackend::Generic
        );
        assert_eq!(
            crate::resolve_flip_backend(SimdPreference::Sse2, false),
            FlipBackend::Generic
        );
        assert_eq!(
            crate::resolve_flip_backend(SimdPreference::Auto, false),
            FlipBackend::Generic
        );
        assert_eq!(
            crate::resolve_flip_backend(SimdPreference::Auto, true),
            FlipBackend::Avx2
        );
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn forcing_avx2_without_cpu_support_panics_in_backend_resolution() {
        let movegen = std::panic::catch_unwind(|| {
            crate::resolve_movegen_backend(SimdPreference::Avx2, false)
        });
        let flip =
            std::panic::catch_unwind(|| crate::resolve_flip_backend(SimdPreference::Avx2, false));
        assert!(movegen.is_err());
        assert!(flip.is_err());
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn avx2_flip_matches_generic_oracle_for_curated_positions() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }

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
                let (player_bits, opponent_bits) = match board.side_to_move {
                    Color::Black => (board.black_bits, board.white_bits),
                    Color::White => (board.white_bits, board.black_bits),
                };

                let generic = crate::flips_for_move_bits_unchecked_generic(
                    player_bits,
                    opponent_bits,
                    mv.square,
                );
                let avx2 = unsafe {
                    crate::flips_for_move_bits_avx2(player_bits, opponent_bits, mv.square)
                };
                assert_eq!(avx2, generic);
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn auto_or_forced_avx2_flip_backend_is_reported_consistently() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            assert_eq!(selected_flip_backend(), "generic");
            return;
        }

        let backend = selected_flip_backend();
        assert!(backend == "generic" || backend == "avx2");
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn selected_backends_match_runtime_resolution() {
        let preference = crate::simd_preference();
        let has_avx2 = std::arch::is_x86_feature_detected!("avx2");

        assert_eq!(
            crate::selected_movegen_backend_kind(),
            crate::resolve_movegen_backend(preference, has_avx2)
        );
        assert_eq!(
            crate::selected_board_backend_kind(),
            crate::resolve_board_backend(preference)
        );
        assert_eq!(
            crate::selected_flip_backend_kind(),
            crate::resolve_flip_backend(preference, has_avx2)
        );
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

        assert_eq!(result, Err(BoardError::OverlappingDiscs));
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
        assert_eq!(invalid.validate(), Err(BoardError::OverlappingDiscs));
    }

    #[test]
    fn packed_board_keeps_black_white_bits_and_side_to_move() {
        // PackedBoard が盤面の固定長表現として各値をそのまま保持できることを確認する。
        let packed = PackedBoard {
            black_bits: 0x12,
            white_bits: 0x34,
            side_to_move: Color::White,
        };

        assert_eq!(packed.black_bits, 0x12);
        assert_eq!(packed.white_bits, 0x34);
        assert_eq!(packed.side_to_move, Color::White);
    }

    #[test]
    fn pack_board_matches_initial_position_expected_values() {
        // 初期局面を pack_board した結果が固定値どおりであることを確認する。
        let packed = pack_board(&Board::new_initial());

        assert_eq!(
            packed,
            PackedBoard {
                black_bits: (1u64 << E4) | (1u64 << D5),
                white_bits: (1u64 << D4) | (1u64 << E5),
                side_to_move: Color::Black,
            }
        );
    }

    #[test]
    fn pack_then_unpack_restores_original_board() {
        // pack -> unpack の往復で元の盤面が保たれることを確認する。
        let board = Board {
            black_bits: 0x00FF_0000_0000_0000,
            white_bits: 0x0000_0000_00FF_0000,
            side_to_move: Color::White,
        };

        assert_eq!(unpack_board(pack_board(&board)), Ok(board));
    }

    #[test]
    fn unpack_then_pack_restores_packed_representation() {
        // unpack -> pack の往復で fixed-length 表現が保たれることを確認する。
        let packed = PackedBoard {
            black_bits: 0x0000_0018_1000_0000,
            white_bits: 0x0000_0000_0810_0000,
            side_to_move: Color::Black,
        };

        assert_eq!(
            unpack_board(packed).map(|board| pack_board(&board)),
            Ok(packed)
        );
    }

    #[test]
    fn unpack_board_rejects_overlapping_discs() {
        // 非合法 packed からの復元で BoardError を返すことを確認する。
        let packed = PackedBoard {
            black_bits: 1u64 << D4,
            white_bits: 1u64 << D4,
            side_to_move: Color::Black,
        };

        assert_eq!(unpack_board(packed), Err(BoardError::OverlappingDiscs));
    }

    #[test]
    fn random_generator_is_reproducible_for_same_seed() {
        // 同じ seed なら PRNG の出力列が一致することを確認する。
        let mut lhs = XorShift64Star::new(123);
        let mut rhs = XorShift64Star::new(123);

        for _ in 0..8 {
            assert_eq!(lhs.next_u64(), rhs.next_u64());
        }
    }

    #[test]
    fn play_random_game_is_reproducible_for_same_seed() {
        // 同じ seed と config なら同じランダム対局トレースが返ることを確認する。
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
        // トレース中の各遷移が合法手または強制パスだけで構成されることを確認する。
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
        // 強制パス局面から開始した場合、手順に None が記録されることを確認する。
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
        // 途中停止しても最終ラベルは終局まで進めた結果と一致することを確認する。
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
        // sampling した盤面が有効で、指定手数帯に収まる石数を持つことを確認する。
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
    fn generic_legal_moves_matches_naive_oracle_for_curated_positions() {
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
            let (player_bits, opponent_bits) = match board.side_to_move {
                Color::Black => (board.black_bits, board.white_bits),
                Color::White => (board.white_bits, board.black_bits),
            };
            assert_eq!(
                legal_moves_bitmask_generic(player_bits, opponent_bits),
                generate_legal_moves_naive(&board).bitmask
            );
        }
    }

    #[test]
    fn generic_legal_moves_matches_naive_oracle_for_deterministic_random_positions() {
        let mut seed = 0x4c95_2a61_f18d_730b_u64;

        for _ in 0..1_000 {
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
            let (player_bits, opponent_bits) = match board.side_to_move {
                Color::Black => (board.black_bits, board.white_bits),
                Color::White => (board.white_bits, board.black_bits),
            };

            assert_eq!(
                legal_moves_bitmask_generic(player_bits, opponent_bits),
                generate_legal_moves_naive(&board).bitmask
            );
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
    #[ignore = "api benchmark helper"]
    fn api_bench_generate_legal_moves_initial_position() {
        print_simd_status();
        let board = Board::new_initial();
        let start = std::time::Instant::now();
        let mut checksum = 0u64;
        for _ in 0..5_000_000 {
            checksum ^= generate_legal_moves(&board).bitmask;
        }
        println!(
            "rust generate_legal_moves elapsed={:.6}s checksum={}",
            start.elapsed().as_secs_f64(),
            checksum
        );
    }

    #[test]
    #[ignore = "api benchmark helper"]
    fn api_bench_apply_move_unchecked_initial_position() {
        print_simd_status();
        let board = Board::new_initial();
        let mv = Move { square: 19 };
        let start = std::time::Instant::now();
        let mut checksum = 0u64;
        for _ in 0..5_000_000 {
            checksum ^= apply_move_unchecked(&board, mv).black_bits;
        }
        println!(
            "rust apply_move_unchecked elapsed={:.6}s checksum={}",
            start.elapsed().as_secs_f64(),
            checksum
        );
    }

    #[test]
    #[ignore = "api benchmark helper"]
    fn api_bench_apply_move_initial_position() {
        print_simd_status();
        let board = Board::new_initial();
        let mv = Move { square: 19 };
        let start = std::time::Instant::now();
        let mut checksum = 0u64;
        for _ in 0..5_000_000 {
            checksum ^= apply_move(&board, mv).unwrap().white_bits;
        }
        println!(
            "rust apply_move elapsed={:.6}s checksum={}",
            start.elapsed().as_secs_f64(),
            checksum
        );
    }

    #[test]
    fn generate_legal_moves_returns_expected_moves_for_initial_position() {
        // 初期局面の黒番合法手 4 つが正しく列挙されることを確認する。
        let expected = (1u64 << 19) | (1u64 << 26) | (1u64 << 37) | (1u64 << 44);
        assert_legal_moves(&Board::new_initial(), expected);
    }

    #[test]
    fn is_legal_move_matches_generated_bitmask() {
        let board = Board::new_initial();
        let legal = generate_legal_moves(&board);

        for square in 0..64u8 {
            assert_eq!(
                is_legal_move(&board, Move { square }),
                legal.bitmask & bit(square) != 0
            );
        }
    }

    #[test]
    fn legal_moves_to_vec_returns_ascending_squares() {
        let legal = generate_legal_moves(&Board::new_initial());
        let moves = legal_moves_to_vec(legal);
        let squares: Vec<u8> = moves.into_iter().map(|mv| mv.square).collect();
        assert_eq!(squares, vec![19, 26, 37, 44]);
    }

    #[test]
    fn disc_count_and_margin_match_initial_position() {
        let board = Board::new_initial();
        assert_eq!(
            disc_count(&board),
            DiscCount {
                black: 2,
                white: 2,
                empty: 60,
            }
        );
        assert_eq!(final_margin_from_black(&board), 0);
        assert_eq!(final_margin_from_side_to_move(&board), 0);
        assert_eq!(game_result(&board), GameResult::Draw);
    }

    #[test]
    fn margin_and_result_follow_disc_difference_for_any_position() {
        let board = Board {
            black_bits: bit(0) | bit(1) | bit(2),
            white_bits: bit(8),
            side_to_move: Color::White,
        };
        assert_eq!(
            disc_count(&board),
            DiscCount {
                black: 3,
                white: 1,
                empty: 60
            }
        );
        assert_eq!(final_margin_from_black(&board), 2);
        assert_eq!(final_margin_from_side_to_move(&board), -2);
        assert_eq!(game_result(&board), GameResult::BlackWin);
    }

    #[test]
    fn all_symmetries_returns_fixed_order() {
        assert_eq!(
            all_symmetries(),
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
        );
    }

    #[test]
    fn transform_square_matches_fixed_coordinate_mapping() {
        let square = 19u8;
        assert_eq!(transform_square(square, Symmetry::Identity), 19);
        assert_eq!(transform_square(square, Symmetry::Rot90), 29);
        assert_eq!(transform_square(square, Symmetry::Rot180), 44);
        assert_eq!(transform_square(square, Symmetry::Rot270), 34);
        assert_eq!(transform_square(square, Symmetry::FlipHorizontal), 20);
        assert_eq!(transform_square(square, Symmetry::FlipVertical), 43);
        assert_eq!(transform_square(square, Symmetry::FlipDiag), 26);
        assert_eq!(transform_square(square, Symmetry::FlipAntiDiag), 37);
    }

    #[test]
    fn rotational_and_reflection_symmetries_round_trip() {
        let square = 19u8;
        let mut rotated = square;
        for _ in 0..4 {
            rotated = transform_square(rotated, Symmetry::Rot90);
        }
        assert_eq!(rotated, square);
        assert_eq!(
            transform_square(
                transform_square(square, Symmetry::FlipHorizontal),
                Symmetry::FlipHorizontal
            ),
            square
        );
        assert_eq!(
            transform_square(
                transform_square(square, Symmetry::FlipVertical),
                Symmetry::FlipVertical
            ),
            square
        );
        assert_eq!(
            transform_square(
                transform_square(square, Symmetry::FlipDiag),
                Symmetry::FlipDiag
            ),
            square
        );
        assert_eq!(
            transform_square(
                transform_square(square, Symmetry::FlipAntiDiag),
                Symmetry::FlipAntiDiag
            ),
            square
        );
    }

    #[test]
    fn transform_board_preserves_side_to_move_and_counts() {
        let board = Board {
            black_bits: bit(19) | bit(26) | bit(44),
            white_bits: bit(20) | bit(29),
            side_to_move: Color::White,
        };
        let transformed = transform_board(&board, Symmetry::Rot90);
        assert_eq!(transformed.side_to_move, Color::White);
        assert_eq!(disc_count(&transformed), disc_count(&board));
        assert_eq!(
            final_margin_from_black(&transformed),
            final_margin_from_black(&board)
        );
    }

    #[test]
    fn transform_board_matches_squarewise_mapping() {
        let board = Board {
            black_bits: bit(0) | bit(19) | bit(63),
            white_bits: bit(7) | bit(28),
            side_to_move: Color::Black,
        };
        let transformed = transform_board(&board, Symmetry::FlipDiag);
        let mut expected_black = 0u64;
        for sq in [0u8, 19, 63] {
            expected_black |= bit(transform_square(sq, Symmetry::FlipDiag));
        }
        let mut expected_white = 0u64;
        for sq in [7u8, 28] {
            expected_white |= bit(transform_square(sq, Symmetry::FlipDiag));
        }
        assert_eq!(transformed.black_bits, expected_black);
        assert_eq!(transformed.white_bits, expected_white);
    }

    #[test]
    fn transformed_legal_moves_match_transformed_squares() {
        let board = Board::new_initial();
        let transformed = transform_board(&board, Symmetry::Rot90);
        let transformed_legal = generate_legal_moves(&transformed);
        let expected = legal_moves_to_vec(generate_legal_moves(&board))
            .into_iter()
            .fold(0u64, |acc, mv| {
                acc | bit(transform_square(mv.square, Symmetry::Rot90))
            });
        assert_eq!(transformed_legal.bitmask, expected);
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
