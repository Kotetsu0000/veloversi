use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::engine::{
    Board, BoardStatus, GameResult, Move, apply_forced_pass, apply_move, board_status, disc_count,
    final_margin_from_black, generate_legal_moves, legal_moves_to_vec,
};
use crate::serialize::{PackedBoard, pack_board, unpack_board};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameRecording {
    pub start_board: Board,
    pub current_board: Board,
    pub moves: Vec<Option<Move>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameRecord {
    pub start_board: PackedBoard,
    pub moves: Vec<Option<u8>>,
    pub final_result: GameResult,
    pub final_black_discs: u8,
    pub final_white_discs: u8,
    pub final_empty_discs: u8,
    pub final_margin_from_black: i8,
}

#[derive(Debug)]
pub enum RecordingError {
    InvalidMove,
    InvalidPass,
    NotTerminal,
    InvalidFormat(String),
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for RecordingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecordingError::InvalidMove => write!(f, "invalid move for current recording state"),
            RecordingError::InvalidPass => write!(f, "invalid pass for current recording state"),
            RecordingError::NotTerminal => write!(f, "recording is not terminal"),
            RecordingError::InvalidFormat(message) => write!(f, "{message}"),
            RecordingError::Io(err) => write!(f, "{err}"),
            RecordingError::Json(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for RecordingError {}

impl From<std::io::Error> for RecordingError {
    fn from(value: std::io::Error) -> Self {
        RecordingError::Io(value)
    }
}

impl From<serde_json::Error> for RecordingError {
    fn from(value: serde_json::Error) -> Self {
        RecordingError::Json(value)
    }
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct JsonPackedBoard {
    black_bits: u64,
    white_bits: u64,
    side_to_move: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct JsonGameRecord {
    start_board: JsonPackedBoard,
    moves: Vec<Option<u8>>,
    final_result: String,
    final_black_discs: u8,
    final_white_discs: u8,
    final_empty_discs: u8,
    final_margin_from_black: i8,
}

fn game_result_to_record_str(result: GameResult) -> &'static str {
    match result {
        GameResult::BlackWin => "black",
        GameResult::WhiteWin => "white",
        GameResult::Draw => "draw",
    }
}

fn record_str_to_game_result(result: &str) -> Result<GameResult, RecordingError> {
    match result {
        "black" => Ok(GameResult::BlackWin),
        "white" => Ok(GameResult::WhiteWin),
        "draw" => Ok(GameResult::Draw),
        _ => Err(RecordingError::InvalidFormat(
            "final_result must be 'black', 'white', or 'draw'".to_string(),
        )),
    }
}

fn packed_board_to_json(board: PackedBoard) -> JsonPackedBoard {
    JsonPackedBoard {
        black_bits: board.black_bits,
        white_bits: board.white_bits,
        side_to_move: match board.side_to_move {
            crate::engine::Color::Black => "black".to_string(),
            crate::engine::Color::White => "white".to_string(),
        },
    }
}

fn json_to_packed_board(board: JsonPackedBoard) -> Result<PackedBoard, RecordingError> {
    let side_to_move = match board.side_to_move.as_str() {
        "black" => crate::engine::Color::Black,
        "white" => crate::engine::Color::White,
        _ => {
            return Err(RecordingError::InvalidFormat(
                "side_to_move must be 'black' or 'white'".to_string(),
            ));
        }
    };
    let packed = PackedBoard {
        black_bits: board.black_bits,
        white_bits: board.white_bits,
        side_to_move,
    };
    unpack_board(packed).map_err(|err| RecordingError::InvalidFormat(format!("{err:?}")))?;
    Ok(packed)
}

fn game_record_to_json(record: &GameRecord) -> JsonGameRecord {
    JsonGameRecord {
        start_board: packed_board_to_json(record.start_board),
        moves: record.moves.clone(),
        final_result: game_result_to_record_str(record.final_result).to_string(),
        final_black_discs: record.final_black_discs,
        final_white_discs: record.final_white_discs,
        final_empty_discs: record.final_empty_discs,
        final_margin_from_black: record.final_margin_from_black,
    }
}

fn json_to_game_record(record: JsonGameRecord) -> Result<GameRecord, RecordingError> {
    let start_board = json_to_packed_board(record.start_board)?;
    Ok(GameRecord {
        start_board,
        moves: record.moves,
        final_result: record_str_to_game_result(&record.final_result)?,
        final_black_discs: record.final_black_discs,
        final_white_discs: record.final_white_discs,
        final_empty_discs: record.final_empty_discs,
        final_margin_from_black: record.final_margin_from_black,
    })
}

pub fn random_start_board(plies: u16, seed: u64) -> Board {
    let mut board = Board::new_initial();
    let mut rng = XorShift64Star::new(seed);

    for _ in 0..plies {
        match board_status(&board) {
            BoardStatus::Terminal => break,
            BoardStatus::ForcedPass => {
                board = apply_forced_pass(&board).expect("forced pass must succeed");
            }
            BoardStatus::Ongoing => {
                let legal_moves = legal_moves_to_vec(generate_legal_moves(&board));
                let mv = legal_moves[rng.choose_index(legal_moves.len())];
                board = apply_move(&board, mv).expect("chosen move must be legal");
            }
        }
    }

    board
}

pub fn start_game_recording(start_board: &Board) -> GameRecording {
    GameRecording {
        start_board: *start_board,
        current_board: *start_board,
        moves: Vec::new(),
    }
}

pub fn current_board(recording: &GameRecording) -> Board {
    recording.current_board
}

pub fn record_move(recording: &GameRecording, mv: Move) -> Result<GameRecording, RecordingError> {
    let next_board =
        apply_move(&recording.current_board, mv).map_err(|_| RecordingError::InvalidMove)?;
    let mut moves = recording.moves.clone();
    moves.push(Some(mv));
    Ok(GameRecording {
        start_board: recording.start_board,
        current_board: next_board,
        moves,
    })
}

pub fn record_pass(recording: &GameRecording) -> Result<GameRecording, RecordingError> {
    let next_board =
        apply_forced_pass(&recording.current_board).map_err(|_| RecordingError::InvalidPass)?;
    let mut moves = recording.moves.clone();
    moves.push(None);
    Ok(GameRecording {
        start_board: recording.start_board,
        current_board: next_board,
        moves,
    })
}

pub fn finish_game_recording(recording: &GameRecording) -> Result<GameRecord, RecordingError> {
    if board_status(&recording.current_board) != BoardStatus::Terminal {
        return Err(RecordingError::NotTerminal);
    }
    let counts = disc_count(&recording.current_board);
    Ok(GameRecord {
        start_board: pack_board(&recording.start_board),
        moves: recording
            .moves
            .iter()
            .map(|mv| mv.map(|mv| mv.square))
            .collect(),
        final_result: crate::engine::game_result(&recording.current_board),
        final_black_discs: counts.black,
        final_white_discs: counts.white,
        final_empty_discs: counts.empty,
        final_margin_from_black: final_margin_from_black(&recording.current_board),
    })
}

pub fn append_game_record(path: &Path, record: &GameRecord) -> Result<(), RecordingError> {
    if path.exists() {
        let file = fs::File::open(path)?;
        let reader = BufReader::new(file);
        for (line_no, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let parsed: JsonGameRecord = serde_json::from_str(&line).map_err(|err| {
                RecordingError::InvalidFormat(format!(
                    "invalid JSONL at line {}: {err}",
                    line_no + 1
                ))
            })?;
            json_to_game_record(parsed)?;
        }
    }

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let line = serde_json::to_string(&game_record_to_json(record))?;
    writeln!(file, "{line}")?;
    Ok(())
}

pub fn load_game_records(path: &Path) -> Result<Vec<GameRecord>, RecordingError> {
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut records = Vec::new();
    for (line_no, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let parsed: JsonGameRecord = serde_json::from_str(&line).map_err(|err| {
            RecordingError::InvalidFormat(format!("invalid JSONL at line {}: {err}", line_no + 1))
        })?;
        records.push(json_to_game_record(parsed)?);
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        RecordingError, XorShift64Star, append_game_record, current_board, finish_game_recording,
        game_result_to_record_str, json_to_packed_board, load_game_records, random_start_board,
        record_move, record_pass, record_str_to_game_result, start_game_recording,
    };
    use crate::engine::{
        Board, BoardStatus, Color, GameResult, Move, apply_forced_pass, board_status,
    };

    fn unique_temp_path() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("veloversi-recording-{nanos}.jsonl"))
    }

    #[test]
    fn random_start_board_is_reproducible_for_same_seed() {
        assert_eq!(random_start_board(5, 7), random_start_board(5, 7));
    }

    #[test]
    fn random_start_board_rng_matches_fixed_sequence() {
        let mut rng = XorShift64Star::new(1);
        assert_eq!(rng.next_u64(), 5_180_492_295_206_395_165);
        assert_eq!(rng.next_u64(), 12_380_297_144_915_551_517);
        assert_eq!(rng.choose_index(7), 4);
    }

    #[test]
    fn start_game_recording_keeps_start_and_current_board() {
        let board = Board::new_initial();
        let recording = start_game_recording(&board);

        assert_eq!(recording.start_board, board);
        assert_eq!(recording.current_board, board);
        assert!(recording.moves.is_empty());
    }

    #[test]
    fn record_move_updates_current_board_and_moves() {
        let recording = start_game_recording(&Board::new_initial());
        let next = record_move(&recording, Move { square: 19 }).expect("legal");

        assert_eq!(next.moves, vec![Some(Move { square: 19 })]);
        assert_ne!(current_board(&next), current_board(&recording));
    }

    #[test]
    fn record_pass_updates_current_board_and_moves() {
        let board = Board::from_bits(0xFFFF_FFFF_FFFF_FF7E, 0x0000_0000_0000_0080, Color::Black)
            .expect("valid");
        let recording = start_game_recording(&board);
        let next = record_pass(&recording).expect("forced pass");

        assert_eq!(next.moves, vec![None]);
        assert_eq!(board_status(&current_board(&next)), BoardStatus::Ongoing);
        assert_eq!(
            apply_forced_pass(&board).expect("forced"),
            current_board(&next)
        );
    }

    #[test]
    fn finish_game_recording_requires_terminal_board() {
        let recording = start_game_recording(&Board::new_initial());
        assert!(matches!(
            finish_game_recording(&recording),
            Err(RecordingError::NotTerminal)
        ));
    }

    #[test]
    fn finish_game_recording_returns_counts_and_result() {
        let terminal = Board::from_bits(u64::MAX, 0, Color::Black).expect("valid");
        let recording = start_game_recording(&terminal);
        let record = finish_game_recording(&recording).expect("terminal");

        assert_eq!(record.final_result, crate::engine::GameResult::BlackWin);
        assert_eq!(record.final_black_discs, 64);
        assert_eq!(record.final_white_discs, 0);
        assert_eq!(record.final_empty_discs, 0);
        assert_eq!(record.final_margin_from_black, 64);
    }

    #[test]
    fn append_and_load_game_records_round_trip_jsonl() {
        let path = unique_temp_path();
        let terminal = Board::from_bits(u64::MAX, 0, Color::Black).expect("valid");
        let record = finish_game_recording(&start_game_recording(&terminal)).expect("terminal");

        append_game_record(&path, &record).expect("append first");
        append_game_record(&path, &record).expect("append second");
        let loaded = load_game_records(&path).expect("load");
        assert_eq!(loaded, vec![record.clone(), record]);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn record_result_strings_round_trip_all_variants() {
        assert_eq!(game_result_to_record_str(GameResult::BlackWin), "black");
        assert_eq!(game_result_to_record_str(GameResult::WhiteWin), "white");
        assert_eq!(game_result_to_record_str(GameResult::Draw), "draw");
        assert!(matches!(
            record_str_to_game_result("black"),
            Ok(GameResult::BlackWin)
        ));
        assert!(matches!(
            record_str_to_game_result("white"),
            Ok(GameResult::WhiteWin)
        ));
        assert!(matches!(
            record_str_to_game_result("draw"),
            Ok(GameResult::Draw)
        ));
    }

    #[test]
    fn json_to_packed_board_rejects_invalid_side_to_move() {
        let err = json_to_packed_board(super::JsonPackedBoard {
            black_bits: 0,
            white_bits: 0,
            side_to_move: "bad".to_string(),
        });
        assert!(matches!(err, Err(RecordingError::InvalidFormat(_))));
    }

    #[test]
    fn json_to_packed_board_accepts_white_side_to_move() {
        let packed = json_to_packed_board(super::JsonPackedBoard {
            black_bits: 0x0000_0000_1000_0000,
            white_bits: 0x0000_0008_0000_0000,
            side_to_move: "white".to_string(),
        })
        .expect("valid white board");

        assert_eq!(packed.side_to_move, Color::White);
    }

    #[test]
    fn recording_error_display_includes_message() {
        assert_eq!(
            RecordingError::InvalidMove.to_string(),
            "invalid move for current recording state"
        );
        assert_eq!(
            RecordingError::InvalidPass.to_string(),
            "invalid pass for current recording state"
        );
        assert_eq!(
            RecordingError::NotTerminal.to_string(),
            "recording is not terminal"
        );
    }
}
