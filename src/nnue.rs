use crate::engine::{Board, Color, DiscCount, disc_count, generate_legal_moves};
use serde::Deserialize;
use smallvec::SmallVec;
use std::array::from_fn;
use std::fmt;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

pub const NNUE_PATTERN_FAMILIES: usize = 16;
pub const NNUE_PATTERN_SLOTS: usize = 64;
pub const NNUE_SCALAR_SLOTS: usize = 3;
pub const NNUE_INPUT_LEN: usize = NNUE_PATTERN_SLOTS + NNUE_SCALAR_SLOTS;
pub const NNUE_ACCUMULATOR_DIM: usize = 32;
pub const NNUE_HIDDEN_DIM: usize = 16;
pub const NNUE_FORMAT: &str = "veloversi-vvm";
pub const NNUE_ARCHITECTURE: &str = "nnue-v1";
pub const NNUE_VERSION: u32 = 1;
pub const NNUE_SCALAR_BUCKET_SIZES: [usize; NNUE_SCALAR_SLOTS] = [65, 65, 65];

const MAX_PATTERN_CELLS: usize = 10;
const UNUSED_CELL: u8 = 0xFF;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedNnueInput {
    pub len: usize,
    pub data_i32: Vec<i32>,
}

#[derive(Debug)]
pub enum NnueModelError {
    Io(std::io::Error),
    Parse(serde_json::Error),
    InvalidFormat(&'static str),
    InvalidField(String),
}

impl fmt::Display for NnueModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "failed to read model file: {err}"),
            Self::Parse(err) => write!(f, "failed to parse model file: {err}"),
            Self::InvalidFormat(message) => write!(f, "{message}"),
            Self::InvalidField(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for NnueModelError {}

impl From<std::io::Error> for NnueModelError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for NnueModelError {
    fn from(value: serde_json::Error) -> Self {
        Self::Parse(value)
    }
}

#[derive(Clone, Debug)]
pub struct NnueValueModel {
    accumulator_dim: usize,
    hidden_dim: usize,
    pattern_family_sizes: [u8; NNUE_PATTERN_FAMILIES],
    scalar_bucket_sizes: [u16; NNUE_SCALAR_SLOTS],
    pattern_tables: [QuantizedTable; NNUE_PATTERN_FAMILIES],
    scalar_tables: [QuantizedTable; NNUE_SCALAR_SLOTS],
    accumulator_bias: Vec<f32>,
    fc1: QuantizedLinear,
    fc2: QuantizedLinear,
}

#[derive(Clone, Debug)]
struct QuantizedTable {
    rows: usize,
    cols: usize,
    scale: f32,
    values: Vec<i8>,
}

#[derive(Clone, Debug)]
struct QuantizedLinear {
    out_dim: usize,
    in_dim: usize,
    scale: f32,
    weights: Vec<i8>,
    bias: Vec<f32>,
}

#[derive(Clone, Debug)]
struct PatternMetadata {
    cells: [[u8; MAX_PATTERN_CELLS]; NNUE_PATTERN_SLOTS],
    sizes: [u8; NNUE_PATTERN_SLOTS],
    #[allow(dead_code)]
    affected: [SmallVec<[u8; 16]>; 64],
}

#[derive(Deserialize)]
struct StoredNnueModel {
    format: String,
    version: u32,
    architecture: String,
    input_len: usize,
    accumulator_dim: usize,
    hidden_dim: usize,
    pattern_family_sizes: Vec<u8>,
    scalar_bucket_sizes: Vec<u16>,
    pattern_tables: Vec<StoredQuantizedTable>,
    scalar_tables: Vec<StoredQuantizedTable>,
    accumulator_bias: Vec<f32>,
    fc1: StoredQuantizedLinear,
    fc2: StoredQuantizedLinear,
}

#[derive(Deserialize)]
struct StoredQuantizedTable {
    rows: usize,
    cols: usize,
    scale: f32,
    values: Vec<i8>,
}

#[derive(Deserialize)]
struct StoredQuantizedLinear {
    out_dim: usize,
    in_dim: usize,
    scale: f32,
    weights: Vec<i8>,
    bias: Vec<f32>,
}

const fn sq(file: u8, rank: u8) -> u8 {
    rank * 8 + file
}

const fn rot90_cw(square: u8) -> u8 {
    let file = square % 8;
    let rank = square / 8;
    (7 - file) * 8 + rank
}

const fn apply_rotations(
    mut cells: [u8; MAX_PATTERN_CELLS],
    rotation: u8,
) -> [u8; MAX_PATTERN_CELLS] {
    let mut idx = 0;
    while idx < MAX_PATTERN_CELLS {
        if cells[idx] != UNUSED_CELL {
            let mut current = cells[idx];
            let mut step = 0;
            while step < rotation {
                current = rot90_cw(current);
                step += 1;
            }
            cells[idx] = current;
        }
        idx += 1;
    }
    cells
}

const BASE_PATTERN_SIZES: [u8; NNUE_PATTERN_FAMILIES] =
    [8, 9, 8, 9, 8, 9, 7, 10, 10, 10, 10, 10, 10, 10, 10, 10];

const BASE_PATTERNS: [[u8; MAX_PATTERN_CELLS]; 16] = [
    [
        sq(0, 1),
        sq(1, 1),
        sq(2, 1),
        sq(3, 1),
        sq(4, 1),
        sq(5, 1),
        sq(6, 1),
        sq(7, 1),
        UNUSED_CELL,
        UNUSED_CELL,
    ], // hv2
    [
        sq(1, 0),
        sq(2, 0),
        sq(3, 1),
        sq(4, 2),
        sq(6, 1),
        sq(5, 3),
        sq(6, 4),
        sq(7, 5),
        sq(7, 6),
        UNUSED_CELL,
    ], // d6 + 2C + X
    [
        sq(0, 2),
        sq(1, 2),
        sq(2, 2),
        sq(3, 2),
        sq(4, 2),
        sq(5, 2),
        sq(6, 2),
        sq(7, 2),
        UNUSED_CELL,
        UNUSED_CELL,
    ], // hv3
    [
        sq(0, 0),
        sq(1, 0),
        sq(2, 1),
        sq(3, 2),
        sq(4, 3),
        sq(5, 4),
        sq(6, 5),
        sq(7, 6),
        sq(7, 7),
        UNUSED_CELL,
    ], // d7 + 2corner
    [
        sq(0, 3),
        sq(1, 3),
        sq(2, 3),
        sq(3, 3),
        sq(4, 3),
        sq(5, 3),
        sq(6, 3),
        sq(7, 3),
        UNUSED_CELL,
        UNUSED_CELL,
    ], // hv4
    [
        sq(0, 0),
        sq(1, 0),
        sq(2, 0),
        sq(0, 1),
        sq(1, 1),
        sq(2, 1),
        sq(0, 2),
        sq(1, 2),
        sq(2, 2),
        UNUSED_CELL,
    ], // corner9
    [
        sq(1, 1),
        sq(3, 0),
        sq(4, 1),
        sq(5, 2),
        sq(6, 3),
        sq(7, 4),
        sq(6, 6),
        UNUSED_CELL,
        UNUSED_CELL,
        UNUSED_CELL,
    ], // d5 + 2X
    [
        sq(0, 0),
        sq(1, 1),
        sq(2, 2),
        sq(3, 3),
        sq(4, 4),
        sq(5, 5),
        sq(6, 6),
        sq(7, 7),
        sq(0, 1),
        sq(1, 0),
    ], // d8 + 2C
    [
        sq(1, 1),
        sq(0, 0),
        sq(1, 0),
        sq(2, 0),
        sq(3, 0),
        sq(4, 0),
        sq(5, 0),
        sq(6, 0),
        sq(7, 0),
        sq(6, 1),
    ], // edge + 2x
    [
        sq(0, 0),
        sq(1, 0),
        sq(2, 0),
        sq(3, 0),
        sq(0, 1),
        sq(1, 1),
        sq(2, 1),
        sq(0, 2),
        sq(1, 2),
        sq(0, 3),
    ], // triangle
    [
        sq(0, 0),
        sq(2, 0),
        sq(3, 0),
        sq(4, 0),
        sq(5, 0),
        sq(7, 0),
        sq(2, 1),
        sq(3, 1),
        sq(4, 1),
        sq(5, 1),
    ], // corner + block
    [
        sq(0, 0),
        sq(1, 1),
        sq(2, 2),
        sq(3, 3),
        sq(1, 0),
        sq(2, 1),
        sq(3, 2),
        sq(0, 1),
        sq(1, 2),
        sq(2, 3),
    ], // cross
    [
        sq(2, 1),
        sq(0, 0),
        sq(1, 0),
        sq(2, 0),
        sq(3, 0),
        sq(4, 0),
        sq(5, 0),
        sq(6, 0),
        sq(7, 0),
        sq(5, 1),
    ], // edge + y
    [
        sq(0, 0),
        sq(1, 0),
        sq(2, 0),
        sq(3, 0),
        sq(4, 0),
        sq(0, 1),
        sq(1, 1),
        sq(0, 2),
        sq(0, 3),
        sq(0, 4),
    ], // narrow triangle
    [
        sq(0, 0),
        sq(1, 0),
        sq(0, 1),
        sq(1, 1),
        sq(2, 1),
        sq(3, 1),
        sq(1, 2),
        sq(2, 2),
        sq(1, 3),
        sq(3, 3),
    ], // fish
    [
        sq(2, 5),
        sq(3, 5),
        sq(3, 6),
        sq(3, 7),
        sq(2, 7),
        sq(5, 7),
        sq(4, 7),
        sq(4, 6),
        sq(4, 5),
        sq(5, 5),
    ], // anvil
];

static PATTERN_METADATA: OnceLock<PatternMetadata> = OnceLock::new();

fn pattern_metadata() -> &'static PatternMetadata {
    PATTERN_METADATA.get_or_init(|| {
        let mut cells = [[UNUSED_CELL; MAX_PATTERN_CELLS]; NNUE_PATTERN_SLOTS];
        let mut sizes = [0u8; NNUE_PATTERN_SLOTS];
        let mut affected: [SmallVec<[u8; 16]>; 64] = from_fn(|_| SmallVec::new());
        for family_idx in 0..NNUE_PATTERN_FAMILIES {
            for rotation in 0..4u8 {
                let slot = family_idx * 4 + usize::from(rotation);
                let rotated = apply_rotations(BASE_PATTERNS[family_idx], rotation);
                cells[slot] = rotated;
                sizes[slot] = BASE_PATTERN_SIZES[family_idx];
                for &square in rotated.iter().take(BASE_PATTERN_SIZES[family_idx] as usize) {
                    affected[square as usize].push(slot as u8);
                }
            }
        }
        PatternMetadata {
            cells,
            sizes,
            affected,
        }
    })
}

fn current_side_bits(board: &Board) -> (u64, u64) {
    match board.side_to_move {
        Color::Black => (board.black_bits, board.white_bits),
        Color::White => (board.white_bits, board.black_bits),
    }
}

fn square_state(board: &Board, square: u8) -> i32 {
    let bit = 1u64 << square;
    let (self_bits, opp_bits) = current_side_bits(board);
    if self_bits & bit != 0 {
        1
    } else if opp_bits & bit != 0 {
        2
    } else {
        0
    }
}

fn opponent_to_move_board(board: &Board) -> Board {
    Board {
        black_bits: board.black_bits,
        white_bits: board.white_bits,
        side_to_move: match board.side_to_move {
            Color::Black => Color::White,
            Color::White => Color::Black,
        },
    }
}

fn pattern_index(board: &Board, slot: usize) -> i32 {
    let meta = pattern_metadata();
    let mut value = 0i32;
    let mut factor = 1i32;
    for &square in meta.cells[slot].iter().take(meta.sizes[slot] as usize) {
        value += square_state(board, square) * factor;
        factor *= 3;
    }
    value
}

pub fn prepare_nnue_model_input(board: &Board) -> EncodedNnueInput {
    let mut data = Vec::with_capacity(NNUE_INPUT_LEN);
    for slot in 0..NNUE_PATTERN_SLOTS {
        data.push(pattern_index(board, slot));
    }
    let DiscCount { empty, .. } = disc_count(board);
    let self_mobility = generate_legal_moves(board).count;
    let opp_mobility = generate_legal_moves(&opponent_to_move_board(board)).count;
    data.push(i32::from(empty));
    data.push(i32::from(self_mobility));
    data.push(i32::from(opp_mobility));
    EncodedNnueInput {
        len: NNUE_INPUT_LEN,
        data_i32: data,
    }
}

#[allow(dead_code)]
pub fn affected_pattern_slots(square: u8) -> SmallVec<[u8; 16]> {
    pattern_metadata().affected[square as usize].clone()
}

fn validate_quantized_table(
    table: StoredQuantizedTable,
    expected_cols: usize,
) -> Result<QuantizedTable, NnueModelError> {
    if table.cols != expected_cols {
        return Err(NnueModelError::InvalidField(format!(
            "table cols must be {expected_cols}, got {}",
            table.cols
        )));
    }
    if table.scale <= 0.0 || !table.scale.is_finite() {
        return Err(NnueModelError::InvalidField(
            "table scale must be a finite positive float".to_string(),
        ));
    }
    if table.values.len() != table.rows * table.cols {
        return Err(NnueModelError::InvalidField(format!(
            "table values length must be {}",
            table.rows * table.cols
        )));
    }
    Ok(QuantizedTable {
        rows: table.rows,
        cols: table.cols,
        scale: table.scale,
        values: table.values,
    })
}

fn validate_quantized_linear(
    linear: StoredQuantizedLinear,
    expected_in_dim: usize,
    expected_out_dim: usize,
) -> Result<QuantizedLinear, NnueModelError> {
    if linear.in_dim != expected_in_dim {
        return Err(NnueModelError::InvalidField(format!(
            "linear in_dim must be {expected_in_dim}, got {}",
            linear.in_dim
        )));
    }
    if linear.out_dim != expected_out_dim {
        return Err(NnueModelError::InvalidField(format!(
            "linear out_dim must be {expected_out_dim}, got {}",
            linear.out_dim
        )));
    }
    if linear.scale <= 0.0 || !linear.scale.is_finite() {
        return Err(NnueModelError::InvalidField(
            "linear scale must be a finite positive float".to_string(),
        ));
    }
    if linear.weights.len() != linear.in_dim * linear.out_dim {
        return Err(NnueModelError::InvalidField(format!(
            "linear weights length must be {}",
            linear.in_dim * linear.out_dim
        )));
    }
    if linear.bias.len() != linear.out_dim {
        return Err(NnueModelError::InvalidField(format!(
            "linear bias length must be {}",
            linear.out_dim
        )));
    }
    Ok(QuantizedLinear {
        out_dim: linear.out_dim,
        in_dim: linear.in_dim,
        scale: linear.scale,
        weights: linear.weights,
        bias: linear.bias,
    })
}

pub fn load_rust_value_model(path: impl AsRef<Path>) -> Result<NnueValueModel, NnueModelError> {
    let text = fs::read_to_string(path)?;
    let stored: StoredNnueModel = serde_json::from_str(&text)?;

    if stored.format != NNUE_FORMAT {
        return Err(NnueModelError::InvalidFormat("unexpected .vvm format"));
    }
    if stored.version != NNUE_VERSION {
        return Err(NnueModelError::InvalidFormat("unexpected .vvm version"));
    }
    if stored.architecture != NNUE_ARCHITECTURE {
        return Err(NnueModelError::InvalidFormat(
            "unexpected .vvm architecture",
        ));
    }
    if stored.input_len != NNUE_INPUT_LEN {
        return Err(NnueModelError::InvalidField(format!(
            "input_len must be {NNUE_INPUT_LEN}, got {}",
            stored.input_len
        )));
    }
    if stored.accumulator_dim != NNUE_ACCUMULATOR_DIM {
        return Err(NnueModelError::InvalidField(format!(
            "accumulator_dim must be {NNUE_ACCUMULATOR_DIM}, got {}",
            stored.accumulator_dim
        )));
    }
    if stored.hidden_dim != NNUE_HIDDEN_DIM {
        return Err(NnueModelError::InvalidField(format!(
            "hidden_dim must be {NNUE_HIDDEN_DIM}, got {}",
            stored.hidden_dim
        )));
    }
    if stored.pattern_family_sizes.len() != NNUE_PATTERN_FAMILIES {
        return Err(NnueModelError::InvalidField(format!(
            "pattern_family_sizes length must be {NNUE_PATTERN_FAMILIES}"
        )));
    }
    if stored.scalar_bucket_sizes.len() != NNUE_SCALAR_SLOTS {
        return Err(NnueModelError::InvalidField(format!(
            "scalar_bucket_sizes length must be {NNUE_SCALAR_SLOTS}"
        )));
    }
    if stored.pattern_tables.len() != NNUE_PATTERN_FAMILIES {
        return Err(NnueModelError::InvalidField(format!(
            "pattern_tables length must be {NNUE_PATTERN_FAMILIES}"
        )));
    }
    if stored.scalar_tables.len() != NNUE_SCALAR_SLOTS {
        return Err(NnueModelError::InvalidField(format!(
            "scalar_tables length must be {NNUE_SCALAR_SLOTS}"
        )));
    }
    if stored.accumulator_bias.len() != NNUE_ACCUMULATOR_DIM {
        return Err(NnueModelError::InvalidField(format!(
            "accumulator_bias length must be {NNUE_ACCUMULATOR_DIM}"
        )));
    }

    let mut pattern_family_sizes = [0u8; NNUE_PATTERN_FAMILIES];
    pattern_family_sizes.copy_from_slice(&stored.pattern_family_sizes);
    if pattern_family_sizes != BASE_PATTERN_SIZES {
        return Err(NnueModelError::InvalidField(
            "pattern_family_sizes do not match nnue feature specification".to_string(),
        ));
    }

    let mut scalar_bucket_sizes = [0u16; NNUE_SCALAR_SLOTS];
    scalar_bucket_sizes.copy_from_slice(&stored.scalar_bucket_sizes);
    for (idx, &size) in scalar_bucket_sizes.iter().enumerate() {
        if usize::from(size) != NNUE_SCALAR_BUCKET_SIZES[idx] {
            return Err(NnueModelError::InvalidField(format!(
                "scalar bucket size at index {idx} must be {}",
                NNUE_SCALAR_BUCKET_SIZES[idx]
            )));
        }
    }

    let pattern_tables_vec = stored
        .pattern_tables
        .into_iter()
        .enumerate()
        .map(|(family, table)| {
            let expected_rows = 3usize.pow(u32::from(pattern_family_sizes[family]));
            if table.rows != expected_rows {
                return Err(NnueModelError::InvalidField(format!(
                    "pattern table {family} rows must be {expected_rows}, got {}",
                    table.rows
                )));
            }
            validate_quantized_table(table, NNUE_ACCUMULATOR_DIM)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let pattern_tables: [QuantizedTable; NNUE_PATTERN_FAMILIES] = pattern_tables_vec
        .try_into()
        .map_err(|_| NnueModelError::InvalidFormat("failed to load pattern tables"))?;

    let scalar_tables_vec = stored
        .scalar_tables
        .into_iter()
        .enumerate()
        .map(|(slot, table)| {
            let expected_rows = usize::from(scalar_bucket_sizes[slot]);
            if table.rows != expected_rows {
                return Err(NnueModelError::InvalidField(format!(
                    "scalar table {slot} rows must be {expected_rows}, got {}",
                    table.rows
                )));
            }
            validate_quantized_table(table, NNUE_ACCUMULATOR_DIM)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let scalar_tables: [QuantizedTable; NNUE_SCALAR_SLOTS] = scalar_tables_vec
        .try_into()
        .map_err(|_| NnueModelError::InvalidFormat("failed to load scalar tables"))?;

    let fc1 = validate_quantized_linear(stored.fc1, NNUE_ACCUMULATOR_DIM, NNUE_HIDDEN_DIM)?;
    let fc2 = validate_quantized_linear(stored.fc2, NNUE_HIDDEN_DIM, 1)?;

    Ok(NnueValueModel {
        accumulator_dim: NNUE_ACCUMULATOR_DIM,
        hidden_dim: NNUE_HIDDEN_DIM,
        pattern_family_sizes,
        scalar_bucket_sizes,
        pattern_tables,
        scalar_tables,
        accumulator_bias: stored.accumulator_bias,
        fc1,
        fc2,
    })
}

fn apply_table(table: &QuantizedTable, row: usize, out: &mut [f32]) {
    let start = row * table.cols;
    let values = &table.values[start..start + table.cols];
    for (dst, &weight) in out.iter_mut().zip(values.iter()) {
        *dst += f32::from(weight) * table.scale;
    }
}

fn apply_linear(linear: &QuantizedLinear, input: &[f32], output: &mut [f32]) {
    for (out_idx, dst) in output.iter_mut().enumerate().take(linear.out_dim) {
        let mut value = linear.bias[out_idx];
        let row = &linear.weights[out_idx * linear.in_dim..(out_idx + 1) * linear.in_dim];
        for (input_value, &weight) in input.iter().zip(row.iter()) {
            value += input_value * (f32::from(weight) * linear.scale);
        }
        *dst = value;
    }
}

fn clipped_relu(values: &mut [f32]) {
    for value in values.iter_mut() {
        *value = value.clamp(0.0, 127.0);
    }
}

impl NnueValueModel {
    pub fn predict_encoded(&self, encoded: &[i32]) -> Result<f32, NnueModelError> {
        if encoded.len() != NNUE_INPUT_LEN {
            return Err(NnueModelError::InvalidField(format!(
                "encoded nnue input length must be {NNUE_INPUT_LEN}"
            )));
        }
        let mut accumulator = self.accumulator_bias.clone();
        for (slot, &encoded_value) in encoded.iter().enumerate().take(NNUE_PATTERN_SLOTS) {
            let family = slot / 4;
            let table = &self.pattern_tables[family];
            let row = usize::try_from(encoded_value).map_err(|_| {
                NnueModelError::InvalidField(format!("pattern index at slot {slot} must be >= 0"))
            })?;
            if row >= table.rows {
                return Err(NnueModelError::InvalidField(format!(
                    "pattern index at slot {slot} out of range"
                )));
            }
            apply_table(table, row, &mut accumulator);
        }
        for (scalar_slot, table) in self.scalar_tables.iter().enumerate() {
            let input_slot = NNUE_PATTERN_SLOTS + scalar_slot;
            let row = usize::try_from(encoded[input_slot]).map_err(|_| {
                NnueModelError::InvalidField(format!(
                    "scalar bucket at slot {scalar_slot} must be >= 0"
                ))
            })?;
            if row >= table.rows {
                return Err(NnueModelError::InvalidField(format!(
                    "scalar bucket at slot {scalar_slot} out of range"
                )));
            }
            apply_table(table, row, &mut accumulator);
        }
        clipped_relu(&mut accumulator);
        let mut hidden = vec![0.0; self.hidden_dim];
        apply_linear(&self.fc1, &accumulator, &mut hidden);
        clipped_relu(&mut hidden);
        let mut output = [0.0f32; 1];
        apply_linear(&self.fc2, &hidden, &mut output);
        Ok(output[0])
    }

    pub fn predict_board(&self, board: &Board) -> Result<f32, NnueModelError> {
        let encoded = prepare_nnue_model_input(board);
        self.predict_encoded(&encoded.data_i32)
    }

    pub fn accumulator_dim(&self) -> usize {
        self.accumulator_dim
    }

    pub fn hidden_dim(&self) -> usize {
        self.hidden_dim
    }

    pub fn pattern_family_sizes(&self) -> &[u8; NNUE_PATTERN_FAMILIES] {
        &self.pattern_family_sizes
    }

    pub fn scalar_bucket_sizes(&self) -> &[u16; NNUE_SCALAR_SLOTS] {
        &self.scalar_bucket_sizes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Board;

    #[test]
    fn prepare_nnue_model_input_reports_expected_shape() {
        let encoded = prepare_nnue_model_input(&Board::new_initial());
        assert_eq!(encoded.len, NNUE_INPUT_LEN);
        assert_eq!(encoded.data_i32.len(), NNUE_INPUT_LEN);
    }

    #[test]
    fn affected_pattern_slots_contains_played_square_dependencies() {
        let affected = affected_pattern_slots(19);
        assert!(!affected.is_empty());
        assert!(
            affected
                .iter()
                .all(|slot| usize::from(*slot) < NNUE_PATTERN_SLOTS)
        );
    }

    #[test]
    fn nnue_input_changes_after_move() {
        let board = Board::new_initial();
        let next = crate::engine::apply_move(&board, crate::engine::Move { square: 19 }).unwrap();
        let current = prepare_nnue_model_input(&board);
        let after = prepare_nnue_model_input(&next);
        assert_ne!(current.data_i32, after.data_i32);
    }
}
