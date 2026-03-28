#[cfg(not(any(test, coverage)))]
use ndarray::{Array1, Array2, Array3, Array4};
#[cfg(not(any(test, coverage)))]
use numpy::{IntoPyArray, PyArray1, PyArray2, PyArray3, PyArray4};
use pyo3::prelude::*;

use crate::{
    Board, BoardStatus, Color, Symmetry, all_symmetries, apply_forced_pass, apply_move,
    board_status, disc_count, final_margin_from_black, game_result, generate_legal_moves,
    is_legal_move, legal_moves_to_vec, transform_board, transform_square,
};
#[cfg(not(any(test, coverage)))]
use crate::{
    FeatureConfig, FeaturePerspective, Move, PackedBoard, PackedSupervisedExample, RandomGameTrace,
    RandomPlayConfig, SupervisedExample, encode_flat_features, encode_flat_features_batch,
    encode_planes, encode_planes_batch, pack_board, packed_supervised_examples_from_trace,
    packed_supervised_examples_from_traces, play_random_game, prepare_flat_learning_batch,
    prepare_planes_learning_batch, sample_reachable_positions, supervised_examples_from_trace,
    supervised_examples_from_traces, unpack_board,
};

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

fn game_result_to_py_str(result: crate::GameResult) -> &'static str {
    match result {
        crate::GameResult::BlackWin => "black_win",
        crate::GameResult::WhiteWin => "white_win",
        crate::GameResult::Draw => "draw",
    }
}

#[cfg(not(any(test, coverage)))]
fn py_str_to_feature_perspective(value: &str) -> PyResult<FeaturePerspective> {
    match value {
        "absolute_color" => Ok(FeaturePerspective::AbsoluteColor),
        "side_to_move" => Ok(FeaturePerspective::SideToMove),
        _ => Err(pyo3::exceptions::PyValueError::new_err(
            "perspective must be 'absolute_color' or 'side_to_move'",
        )),
    }
}

#[cfg(not(any(test, coverage)))]
fn feature_config_from_parts(
    history_len: usize,
    include_legal_mask: bool,
    include_phase_plane: bool,
    include_turn_plane: bool,
    perspective: &str,
) -> PyResult<FeatureConfig> {
    Ok(FeatureConfig {
        history_len,
        include_legal_mask,
        include_phase_plane,
        include_turn_plane,
        perspective: py_str_to_feature_perspective(perspective)?,
    })
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
fn move_to_py_option_square(mv: Option<crate::Move>) -> Option<u8> {
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

#[cfg(not(any(test, coverage)))]
type PySupervisedExampleParts = (PyBoardBits, u16, Vec<Option<u8>>, &'static str, i8);

#[cfg(not(any(test, coverage)))]
type PyRandomGameTraceParts = (
    Vec<(u64, u64, String)>,
    Vec<Option<u8>>,
    String,
    i8,
    u16,
    bool,
);

#[cfg(not(any(test, coverage)))]
type PyPackedSupervisedExampleParts = (PyBoardBits, u16, Vec<Option<u8>>, &'static str, i8, i8);

#[cfg(not(any(test, coverage)))]
type PyPackedSupervisedExampleInput = ((u64, u64, String), u16, Vec<Option<u8>>, String, i8, i8);

#[cfg(not(any(test, coverage)))]
type PyPreparedPlanesBatch<'py> = (
    Bound<'py, PyArray4<f32>>,
    Bound<'py, PyArray1<f32>>,
    Bound<'py, PyArray1<i16>>,
    Bound<'py, PyArray2<f32>>,
);

#[cfg(not(any(test, coverage)))]
type PyPreparedFlatBatch<'py> = (
    Bound<'py, PyArray2<f32>>,
    Bound<'py, PyArray1<f32>>,
    Bound<'py, PyArray1<i16>>,
    Bound<'py, PyArray2<f32>>,
);

#[cfg(not(any(test, coverage)))]
fn supervised_example_to_py_parts(example: &SupervisedExample) -> PySupervisedExampleParts {
    (
        board_to_py_tuple(&example.board),
        example.ply,
        example
            .moves_until_here
            .iter()
            .copied()
            .map(move_to_py_option_square)
            .collect(),
        game_result_to_py_str(example.final_result),
        example.final_margin_from_black,
    )
}

#[cfg(not(any(test, coverage)))]
fn packed_supervised_example_to_py_parts(
    example: &PackedSupervisedExample,
) -> PyPackedSupervisedExampleParts {
    (
        packed_board_to_py_tuple(example.board),
        example.ply,
        example.moves_until_here.clone(),
        game_result_to_py_str(example.final_result),
        example.final_margin_from_black,
        example.policy_target_index,
    )
}

#[cfg(not(any(test, coverage)))]
fn packed_supervised_example_from_py_parts(
    board_bits: (u64, u64, String),
    ply: u16,
    moves_until_here: Vec<Option<u8>>,
    final_result: &str,
    final_margin_from_black: i8,
    policy_target_index: i8,
) -> PyResult<PackedSupervisedExample> {
    let board = PackedBoard {
        black_bits: board_bits.0,
        white_bits: board_bits.1,
        side_to_move: py_str_to_color(&board_bits.2)?,
    };
    let final_result = match final_result {
        "black_win" => crate::GameResult::BlackWin,
        "white_win" => crate::GameResult::WhiteWin,
        "draw" => crate::GameResult::Draw,
        _ => {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "final_result must be 'black_win', 'white_win', or 'draw'",
            ));
        }
    };
    Ok(PackedSupervisedExample {
        board,
        ply,
        moves_until_here,
        final_result,
        final_margin_from_black,
        policy_target_index,
    })
}

#[cfg(not(any(test, coverage)))]
fn random_game_trace_from_py_parts(
    boards_bits: Vec<(u64, u64, String)>,
    moves: Vec<Option<u8>>,
    final_result: &str,
    final_margin_from_black: i8,
    plies_played: u16,
    reached_terminal: bool,
) -> PyResult<RandomGameTrace> {
    let boards = boards_bits
        .into_iter()
        .map(|(black_bits, white_bits, side_to_move)| {
            Board::from_bits(black_bits, white_bits, py_str_to_color(&side_to_move)?)
                .map_err(|_| pyo3::exceptions::PyValueError::new_err("invalid board bits"))
        })
        .collect::<PyResult<Vec<_>>>()?;
    let rust_moves = moves
        .into_iter()
        .map(|mv| mv.map(|square| Move { square }))
        .collect();
    let final_result = match final_result {
        "black_win" => crate::GameResult::BlackWin,
        "white_win" => crate::GameResult::WhiteWin,
        "draw" => crate::GameResult::Draw,
        _ => {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "final_result must be 'black_win', 'white_win', or 'draw'",
            ));
        }
    };
    Ok(RandomGameTrace {
        boards,
        moves: rust_moves,
        final_result,
        final_margin_from_black,
        plies_played,
        reached_terminal,
    })
}

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
    let color = py_str_to_color(side_to_move)?;
    Board::from_bits(black_bits, white_bits, color)
        .map(|inner| PyBoard { inner })
        .map_err(|_| pyo3::exceptions::PyValueError::new_err("invalid board bits"))
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
        &crate::PositionSamplingConfig {
            num_positions,
            min_plies,
            max_plies,
        },
    )
    .iter()
    .map(board_to_py_tuple)
    .collect()
}

#[pyfunction(name = "_supervised_examples_from_trace_parts")]
#[cfg(not(any(test, coverage)))]
fn supervised_examples_from_trace_parts_py(
    boards_bits: Vec<(u64, u64, String)>,
    moves: Vec<Option<u8>>,
    final_result: &str,
    final_margin_from_black: i8,
    plies_played: u16,
    reached_terminal: bool,
) -> PyResult<Vec<PySupervisedExampleParts>> {
    let trace = random_game_trace_from_py_parts(
        boards_bits,
        moves,
        final_result,
        final_margin_from_black,
        plies_played,
        reached_terminal,
    )?;
    Ok(supervised_examples_from_trace(&trace)
        .iter()
        .map(supervised_example_to_py_parts)
        .collect())
}

#[pyfunction(name = "_supervised_examples_from_traces_parts")]
#[cfg(not(any(test, coverage)))]
fn supervised_examples_from_traces_parts_py(
    traces: Vec<PyRandomGameTraceParts>,
) -> PyResult<Vec<PySupervisedExampleParts>> {
    let rust_traces = traces
        .into_iter()
        .map(
            |(
                boards_bits,
                moves,
                final_result,
                final_margin_from_black,
                plies_played,
                reached_terminal,
            )| {
                random_game_trace_from_py_parts(
                    boards_bits,
                    moves,
                    &final_result,
                    final_margin_from_black,
                    plies_played,
                    reached_terminal,
                )
            },
        )
        .collect::<PyResult<Vec<_>>>()?;
    Ok(supervised_examples_from_traces(&rust_traces)
        .iter()
        .map(supervised_example_to_py_parts)
        .collect())
}

#[pyfunction(name = "_packed_supervised_examples_from_trace_parts")]
#[cfg(not(any(test, coverage)))]
fn packed_supervised_examples_from_trace_parts_py(
    boards_bits: Vec<(u64, u64, String)>,
    moves: Vec<Option<u8>>,
    final_result: &str,
    final_margin_from_black: i8,
    plies_played: u16,
    reached_terminal: bool,
) -> PyResult<Vec<PyPackedSupervisedExampleParts>> {
    let trace = random_game_trace_from_py_parts(
        boards_bits,
        moves,
        final_result,
        final_margin_from_black,
        plies_played,
        reached_terminal,
    )?;
    Ok(packed_supervised_examples_from_trace(&trace)
        .iter()
        .map(packed_supervised_example_to_py_parts)
        .collect())
}

#[pyfunction(name = "_packed_supervised_examples_from_traces_parts")]
#[cfg(not(any(test, coverage)))]
fn packed_supervised_examples_from_traces_parts_py(
    traces: Vec<PyRandomGameTraceParts>,
) -> PyResult<Vec<PyPackedSupervisedExampleParts>> {
    let rust_traces = traces
        .into_iter()
        .map(
            |(
                boards_bits,
                moves,
                final_result,
                final_margin_from_black,
                plies_played,
                reached_terminal,
            )| {
                random_game_trace_from_py_parts(
                    boards_bits,
                    moves,
                    &final_result,
                    final_margin_from_black,
                    plies_played,
                    reached_terminal,
                )
            },
        )
        .collect::<PyResult<Vec<_>>>()?;
    Ok(packed_supervised_examples_from_traces(&rust_traces)
        .iter()
        .map(packed_supervised_example_to_py_parts)
        .collect())
}

#[pyfunction(name = "_prepare_planes_learning_batch_parts")]
#[cfg(not(any(test, coverage)))]
#[allow(clippy::too_many_arguments)]
fn prepare_planes_learning_batch_parts_py<'py>(
    py: Python<'py>,
    examples: Vec<PyPackedSupervisedExampleInput>,
    history_len: usize,
    include_legal_mask: bool,
    include_phase_plane: bool,
    include_turn_plane: bool,
    perspective: &str,
) -> PyResult<PyPreparedPlanesBatch<'py>> {
    let config = feature_config_from_parts(
        history_len,
        include_legal_mask,
        include_phase_plane,
        include_turn_plane,
        perspective,
    )?;
    let rust_examples = examples
        .into_iter()
        .map(
            |(
                board_bits,
                ply,
                moves_until_here,
                final_result,
                final_margin_from_black,
                policy_target_index,
            )| {
                packed_supervised_example_from_py_parts(
                    board_bits,
                    ply,
                    moves_until_here,
                    &final_result,
                    final_margin_from_black,
                    policy_target_index,
                )
            },
        )
        .collect::<PyResult<Vec<_>>>()?;
    let batch = prepare_planes_learning_batch(&rust_examples, &config)
        .map_err(|err| pyo3::exceptions::PyValueError::new_err(format!("{err:?}")))?;

    let features = Array4::from_shape_vec(
        (
            batch.features.batch,
            batch.features.channels,
            batch.features.height,
            batch.features.width,
        ),
        batch.features.data_f32,
    )
    .expect("planes batch shape must be valid")
    .into_pyarray(py);
    let value_targets = Array1::from_shape_vec(batch.value_targets.len(), batch.value_targets)
        .expect("value target shape must be valid")
        .into_pyarray(py);
    let policy_targets = Array1::from_shape_vec(batch.policy_targets.len(), batch.policy_targets)
        .expect("policy target shape must be valid")
        .into_pyarray(py);
    let legal_move_masks =
        Array2::from_shape_vec((rust_examples.len(), 64), batch.legal_move_masks)
            .expect("legal move mask shape must be valid")
            .into_pyarray(py);
    Ok((features, value_targets, policy_targets, legal_move_masks))
}

#[pyfunction(name = "_prepare_flat_learning_batch_parts")]
#[cfg(not(any(test, coverage)))]
#[allow(clippy::too_many_arguments)]
fn prepare_flat_learning_batch_parts_py<'py>(
    py: Python<'py>,
    examples: Vec<PyPackedSupervisedExampleInput>,
    history_len: usize,
    include_legal_mask: bool,
    include_phase_plane: bool,
    include_turn_plane: bool,
    perspective: &str,
) -> PyResult<PyPreparedFlatBatch<'py>> {
    let config = feature_config_from_parts(
        history_len,
        include_legal_mask,
        include_phase_plane,
        include_turn_plane,
        perspective,
    )?;
    let rust_examples = examples
        .into_iter()
        .map(
            |(
                board_bits,
                ply,
                moves_until_here,
                final_result,
                final_margin_from_black,
                policy_target_index,
            )| {
                packed_supervised_example_from_py_parts(
                    board_bits,
                    ply,
                    moves_until_here,
                    &final_result,
                    final_margin_from_black,
                    policy_target_index,
                )
            },
        )
        .collect::<PyResult<Vec<_>>>()?;
    let batch = prepare_flat_learning_batch(&rust_examples, &config)
        .map_err(|err| pyo3::exceptions::PyValueError::new_err(format!("{err:?}")))?;

    let features = Array2::from_shape_vec(
        (batch.features.batch, batch.features.len),
        batch.features.data_f32,
    )
    .expect("flat batch shape must be valid")
    .into_pyarray(py);
    let value_targets = Array1::from_shape_vec(batch.value_targets.len(), batch.value_targets)
        .expect("value target shape must be valid")
        .into_pyarray(py);
    let policy_targets = Array1::from_shape_vec(batch.policy_targets.len(), batch.policy_targets)
        .expect("policy target shape must be valid")
        .into_pyarray(py);
    let legal_move_masks =
        Array2::from_shape_vec((rust_examples.len(), 64), batch.legal_move_masks)
            .expect("legal move mask shape must be valid")
            .into_pyarray(py);
    Ok((features, value_targets, policy_targets, legal_move_masks))
}

#[pyfunction(name = "_encode_planes_parts")]
#[cfg(not(any(test, coverage)))]
#[allow(clippy::too_many_arguments)]
fn encode_planes_parts_py<'py>(
    py: Python<'py>,
    board: &PyBoard,
    history: Vec<PyBoard>,
    history_len: usize,
    include_legal_mask: bool,
    include_phase_plane: bool,
    include_turn_plane: bool,
    perspective: &str,
) -> PyResult<Bound<'py, PyArray3<f32>>> {
    let config = feature_config_from_parts(
        history_len,
        include_legal_mask,
        include_phase_plane,
        include_turn_plane,
        perspective,
    )?;
    let history_boards: Vec<Board> = history.into_iter().map(|board| board.inner).collect();
    let encoded = encode_planes(&board.inner, &history_boards, &config);
    let array = Array3::from_shape_vec(
        (encoded.channels, encoded.height, encoded.width),
        encoded.data_f32,
    )
    .expect("feature planes shape must be valid");
    Ok(array.into_pyarray(py))
}

#[pyfunction(name = "_encode_planes_batch_parts")]
#[cfg(not(any(test, coverage)))]
#[allow(clippy::too_many_arguments)]
fn encode_planes_batch_parts_py<'py>(
    py: Python<'py>,
    boards: Vec<PyBoard>,
    histories: Vec<Vec<PyBoard>>,
    history_len: usize,
    include_legal_mask: bool,
    include_phase_plane: bool,
    include_turn_plane: bool,
    perspective: &str,
) -> PyResult<Bound<'py, PyArray4<f32>>> {
    let config = feature_config_from_parts(
        history_len,
        include_legal_mask,
        include_phase_plane,
        include_turn_plane,
        perspective,
    )?;
    let rust_boards: Vec<Board> = boards.into_iter().map(|board| board.inner).collect();
    let rust_histories: Vec<Vec<Board>> = histories
        .into_iter()
        .map(|history| history.into_iter().map(|board| board.inner).collect())
        .collect();
    let encoded = encode_planes_batch(&rust_boards, &rust_histories, &config);
    let array = Array4::from_shape_vec(
        (
            encoded.batch,
            encoded.channels,
            encoded.height,
            encoded.width,
        ),
        encoded.data_f32,
    )
    .expect("feature planes batch shape must be valid");
    Ok(array.into_pyarray(py))
}

#[pyfunction(name = "_encode_flat_features_parts")]
#[cfg(not(any(test, coverage)))]
#[allow(clippy::too_many_arguments)]
fn encode_flat_features_parts_py<'py>(
    py: Python<'py>,
    board: &PyBoard,
    history: Vec<PyBoard>,
    history_len: usize,
    include_legal_mask: bool,
    include_phase_plane: bool,
    include_turn_plane: bool,
    perspective: &str,
) -> PyResult<Bound<'py, PyArray1<f32>>> {
    let config = feature_config_from_parts(
        history_len,
        include_legal_mask,
        include_phase_plane,
        include_turn_plane,
        perspective,
    )?;
    let history_boards: Vec<Board> = history.into_iter().map(|board| board.inner).collect();
    let encoded = encode_flat_features(&board.inner, &history_boards, &config);
    let array = Array1::from_shape_vec(encoded.len, encoded.data_f32)
        .expect("flat feature shape must be valid");
    Ok(array.into_pyarray(py))
}

#[pyfunction(name = "_encode_flat_features_batch_parts")]
#[cfg(not(any(test, coverage)))]
#[allow(clippy::too_many_arguments)]
fn encode_flat_features_batch_parts_py<'py>(
    py: Python<'py>,
    boards: Vec<PyBoard>,
    histories: Vec<Vec<PyBoard>>,
    history_len: usize,
    include_legal_mask: bool,
    include_phase_plane: bool,
    include_turn_plane: bool,
    perspective: &str,
) -> PyResult<Bound<'py, PyArray2<f32>>> {
    let config = feature_config_from_parts(
        history_len,
        include_legal_mask,
        include_phase_plane,
        include_turn_plane,
        perspective,
    )?;
    let rust_boards: Vec<Board> = boards.into_iter().map(|board| board.inner).collect();
    let rust_histories: Vec<Vec<Board>> = histories
        .into_iter()
        .map(|history| history.into_iter().map(|board| board.inner).collect())
        .collect();
    let encoded = encode_flat_features_batch(&rust_boards, &rust_histories, &config);
    let array = Array2::from_shape_vec((encoded.batch, encoded.len), encoded.data_f32)
        .expect("flat feature batch shape must be valid");
    Ok(array.into_pyarray(py))
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
    is_legal_move(&board.inner, crate::Move { square })
}

#[pyfunction(name = "apply_move")]
fn apply_move_py(board: &PyBoard, square: u8) -> PyResult<PyBoard> {
    apply_move(&board.inner, crate::Move { square })
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
    #[cfg(not(any(test, coverage)))]
    module.add_function(wrap_pyfunction!(
        supervised_examples_from_trace_parts_py,
        module
    )?)?;
    #[cfg(not(any(test, coverage)))]
    module.add_function(wrap_pyfunction!(
        supervised_examples_from_traces_parts_py,
        module
    )?)?;
    #[cfg(not(any(test, coverage)))]
    module.add_function(wrap_pyfunction!(
        packed_supervised_examples_from_trace_parts_py,
        module
    )?)?;
    #[cfg(not(any(test, coverage)))]
    module.add_function(wrap_pyfunction!(
        packed_supervised_examples_from_traces_parts_py,
        module
    )?)?;
    #[cfg(not(any(test, coverage)))]
    module.add_function(wrap_pyfunction!(
        prepare_planes_learning_batch_parts_py,
        module
    )?)?;
    #[cfg(not(any(test, coverage)))]
    module.add_function(wrap_pyfunction!(
        prepare_flat_learning_batch_parts_py,
        module
    )?)?;
    #[cfg(not(any(test, coverage)))]
    module.add_function(wrap_pyfunction!(encode_planes_parts_py, module)?)?;
    #[cfg(not(any(test, coverage)))]
    module.add_function(wrap_pyfunction!(encode_planes_batch_parts_py, module)?)?;
    #[cfg(not(any(test, coverage)))]
    module.add_function(wrap_pyfunction!(encode_flat_features_parts_py, module)?)?;
    #[cfg(not(any(test, coverage)))]
    module.add_function(wrap_pyfunction!(
        encode_flat_features_batch_parts_py,
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
