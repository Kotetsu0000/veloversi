# オセロ Rust エンジン仕様書

## 1. 目的

本仕様書は、オセロ AI 学習基盤および最終実行系から利用される Rust エンジンの外部仕様と内部仕様を定義する。
本書ではアルゴリズムの詳細は扱わず、以下を定義対象とする。

- 盤面表現
- 公開 API
- Python 連携 API
- 内部専用 API
- 探索・終盤完全読みの入出力契約
- WASM 共有を前提としたデータ型制約
- SIMD 最適化を前提とした実装上の制約

本エンジンは次の 2 クレート構成を前提とする。

- `engine-core`
- `engine-search`

`engine-core` は純粋なゲームロジックを扱う。
`engine-search` は探索、ソルバ、評価呼び出しラッパを扱う。

## 2. 設計原則

### 2.1 主要方針

- 盤面は 64 マス固定とする
- 盤面表現は 2 本の 64bit ビットボードを基本表現とする
- 公開 API はコピーコストの低い値型中心で構成する
- Python 公開 API は PyO3 経由でシリアライズしやすい型へ変換する
- WASM 公開 API は JS から扱いやすい数値配列とバイト列を優先する
- パスは合法手が 0 個のときのみ許可される強制処理とする
- policy 用行動空間は 64 とし、パスは行動として学習対象に含めない
- SIMD 最適化は実装上許容し、関数仕様は SIMD 非依存に保つ

### 2.2 色表現と手番表現

内部表現では絶対色を持つ。

- `black_bits: u64`
- `white_bits: u64`
- `side_to_move: Color`

学習系へ露出する特徴生成時のみ、必要に応じて「自分 / 相手」視点へ変換する。

### 2.3 盤面インデックス規約

盤面インデックスは 0..63 とする。
具体的なマス順は次を標準とする。

- `0 = A1`
- `7 = H1`
- `8 = A2`
- `63 = H8`

すべての公開関数はこのインデックス規約を共有しなければならない。

## 3. モジュール構成

### 3.1 engine-core

- `board`
- `movegen`
- `apply`
- `game`
- `symmetry`
- `feature`
- `serialize`
- `random_play`
- `ffi_python`
- `ffi_wasm`

### 3.2 engine-search

- `search`
- `solver`
- `ttable`
- `ordering`
- `endgame`
- `bench`

## 4. 基本データ型仕様

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    Black,
    White,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Board {
    pub black_bits: u64,
    pub white_bits: u64,
    pub side_to_move: Color,
}

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
pub struct LegalMoves {
    pub bitmask: u64,
    pub count: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoardStatus {
    Ongoing,
    ForcedPass,
    Terminal,
}
```

### 4.1 Board の不変条件

- `black_bits & white_bits == 0`
- 盤面外ビットは常に 0
- `side_to_move` は次に打つ側を表す
- `Board` は常に合法到達局面であることが望ましいが、検証用に非合法状態も保持可能とする

### 4.2 Move の不変条件

- `square` は 0..63
- 合法性は `apply_move` 実行前に別途判定する

## 5. 公開 API 仕様

### 5.1 board モジュール

#### 5.1.1 `Board::new_initial`

```rust
pub fn new_initial() -> Board
```

説明:
- 標準初期配置の盤面を返す
- 手番は黒とする

戻り値:
- 正常な初期局面を保持した `Board`

公開範囲:
- Rust 公開
- Python 公開
- WASM 公開

#### 5.1.2 `Board::from_bits`

```rust
pub fn from_bits(black_bits: u64, white_bits: u64, side_to_move: Color) -> Result<Board, BoardError>
```

説明:
- 外部から与えられたビットボードと手番から `Board` を生成する
- 基本整合性を検証する

検証対象:
- 黒白ビットの重なりがないこと
- 盤面外ビットが 0 であること

戻り値:
- 妥当な場合は `Board`
- 不正な場合は `BoardError`

公開範囲:
- Rust 公開
- Python 公開
- WASM 非公開

#### 5.1.3 `Board::to_bits`

```rust
pub fn to_bits(&self) -> (u64, u64, Color)
```

説明:
- 盤面の内部表現をそのまま返す

公開範囲:
- Rust 公開
- Python 公開
- WASM 非公開

#### 5.1.4 `Board::occupied_bits`

```rust
pub fn occupied_bits(&self) -> u64
```

説明:
- 黒石と白石の OR を返す

公開範囲:
- Rust 公開
- Python 非公開
- WASM 非公開

#### 5.1.5 `Board::empty_bits`

```rust
pub fn empty_bits(&self) -> u64
```

説明:
- 空きマスのビットマスクを返す

公開範囲:
- Rust 公開
- Python 非公開
- WASM 非公開

#### 5.1.6 `Board::validate`

```rust
pub fn validate(&self) -> Result<(), BoardError>
```

説明:
- 内部整合性を検査する
- 到達可能性の判定は含めない

公開範囲:
- Rust 公開
- Python 公開
- WASM 非公開

### 5.2 movegen モジュール

#### 5.2.1 `generate_legal_moves`

```rust
pub fn generate_legal_moves(board: &Board) -> LegalMoves
```

説明:
- 現手番が打てる合法手をすべて列挙し、ビットマスクで返す
- パスは含めない
- `count == 0` のときは合法手なしを意味する

戻り値:
- `bitmask`: 合法手位置に 1 が立った 64bit 値
- `count`: 合法手数

期待結果:
- 合法手が存在する場合、各ビットはオセロ公式ルール上の合法着手位置を表す
- 合法手が存在しない場合、`bitmask == 0` かつ `count == 0`

公開範囲:
- Rust 公開
- Python 公開
- WASM 公開

#### 5.2.2 `is_legal_move`

```rust
pub fn is_legal_move(board: &Board, mv: Move) -> bool
```

説明:
- 指定手が合法かを判定する

公開範囲:
- Rust 公開
- Python 公開
- WASM 公開

#### 5.2.3 `legal_moves_to_vec`

```rust
pub fn legal_moves_to_vec(legal: LegalMoves) -> SmallVec<[Move; 32]>
```

説明:
- 合法手ビットマスクを `Move` 配列へ変換する
- 列挙順は固定であること
- 盤面インデックス昇順を標準とする

公開範囲:
- Rust 公開
- Python 公開
- WASM 非公開

### 5.3 apply モジュール

#### 5.3.1 `apply_move`

```rust
pub fn apply_move(board: &Board, mv: Move) -> Result<Board, MoveError>
```

説明:
- 合法手を 1 手適用した後の新しい盤面を返す
- 反転処理、石配置更新、手番反転を行う

期待結果:
- 指定着手地点に現手番側の石が置かれる
- ルールに従い挟まれた相手石がすべて反転する
- 次手番へ切り替わる
- 次手番側に合法手がないかどうかはこの関数では判定しない

戻り値:
- 成功時は次局面 `Board`
- 非合法手なら `MoveError`

公開範囲:
- Rust 公開
- Python 公開
- WASM 公開

#### 5.3.2 `apply_move_unchecked`

```rust
pub fn apply_move_unchecked(board: &Board, mv: Move) -> Board
```

説明:
- 合法手である前提で着手を適用する内部高速関数
- 非合法入力時の挙動は未定義とする

公開範囲:
- Rust 公開
- Python 非公開
- WASM 非公開

#### 5.3.3 `apply_forced_pass`

```rust
pub fn apply_forced_pass(board: &Board) -> Result<Board, MoveError>
```

説明:
- 合法手が 0 個の局面でのみ手番を相手へ移す
- 石配置は変更しない

期待結果:
- 合法手が 0 個のときのみ `side_to_move` を反転した `Board` を返す
- 合法手が存在する局面ではエラー

公開範囲:
- Rust 公開
- Python 公開
- WASM 公開

### 5.4 game モジュール

#### 5.4.1 `board_status`

```rust
pub fn board_status(board: &Board) -> BoardStatus
```

説明:
- 現局面が継続局面、強制パス局面、終局局面のいずれかを返す

判定基準:
- 現手番に合法手あり -> `Ongoing`
- 現手番に合法手なし、相手番に合法手あり -> `ForcedPass`
- 両者に合法手なし -> `Terminal`

公開範囲:
- Rust 公開
- Python 公開
- WASM 公開

#### 5.4.2 `disc_count`

```rust
pub fn disc_count(board: &Board) -> DiscCount
```

説明:
- 黒石数、白石数、空きマス数を返す

公開範囲:
- Rust 公開
- Python 公開
- WASM 公開

#### 5.4.3 `final_margin_from_black`

```rust
pub fn final_margin_from_black(board: &Board) -> i8
```

説明:
- 常に現在局面の `black_count - white_count` を返す
- 終局局面では最終石差として解釈できる

公開範囲:
- Rust 公開
- Python 公開
- WASM 公開

#### 5.4.4 `final_margin_from_side_to_move`

```rust
pub fn final_margin_from_side_to_move(board: &Board) -> i8
```

説明:
- 常に現在局面の `side_to_move` 視点の石差を返す
- 終局局面では最終石差として解釈できる

公開範囲:
- Rust 公開
- Python 公開
- WASM 非公開

#### 5.4.5 `game_result`

```rust
pub fn game_result(board: &Board) -> GameResult
```

説明:
- 常に現在局面の石数比較に基づく勝敗を返す
- 終局局面では最終結果として解釈できる

公開範囲:
- Rust 公開
- Python 公開
- WASM 公開

### 5.5 symmetry モジュール

#### 5.5.1 `transform_board`

```rust
pub fn transform_board(board: &Board, sym: Symmetry) -> Board
```

説明:
- 盤面に対して対称変換を適用した盤面を返す
- 手番は保持する

対象変換:
- 恒等
- 90 度回転
- 180 度回転
- 270 度回転
- 水平反転
- 垂直反転
- 主対角反転
- 副対角反転

公開範囲:
- Rust 公開
- Python 公開
- WASM 非公開

#### 5.5.2 `transform_square`

```rust
pub fn transform_square(square: u8, sym: Symmetry) -> u8
```

説明:
- 対称変換下での着手マス対応を返す

公開範囲:
- Rust 公開
- Python 公開
- WASM 非公開

#### 5.5.3 `all_symmetries`

```rust
pub fn all_symmetries() -> [Symmetry; 8]
```

説明:
- 利用可能な対称変換一覧を返す

公開範囲:
- Rust 公開
- Python 公開
- WASM 非公開

### 5.6 feature モジュール

本モジュールは学習向け特徴面生成を担う。

#### 5.6.1 `FeatureConfig`

```rust
pub struct FeatureConfig {
    pub history_len: usize,
    pub include_legal_mask: bool,
    pub include_phase_plane: bool,
    pub include_turn_plane: bool,
    pub perspective: FeaturePerspective,
}
```

説明:
- 学習向け特徴生成設定

`perspective`:
- `AbsoluteColor`
- `SideToMove`

#### 5.6.2 `encode_planes`

```rust
pub struct EncodedPlanes {
    pub channels: usize,
    pub width: usize,
    pub height: usize,
    pub data_f32: Vec<f32>,
}

pub struct EncodedPlanesBatch {
    pub batch: usize,
    pub channels: usize,
    pub width: usize,
    pub height: usize,
    pub data_f32: Vec<f32>,
}

pub fn encode_planes(current: &Board, history: &[Board], config: &FeatureConfig) -> EncodedPlanes

pub fn encode_planes_batch(
    boards: &[Board],
    histories: &[Vec<Board>],
    config: &FeatureConfig,
) -> EncodedPlanesBatch
```

説明:
- 指定設定に従い 2D 平面特徴を生成する
- `history` は新しい順で受け取る
- `history` 長が不足する場合の埋め方は 0 埋めを標準とする
- planes は `channels_first` で返す
- dtype は `float32` とする
- Step 14 の dense feature では、current と history 各局面について 2 plane を持つ
- `include_legal_mask` / `include_phase_plane` / `include_turn_plane` は current 局面に対する追加 plane とする

戻り値:
- `EncodedPlanes { channels, width, height, data_f32 }`
- batch 版は `EncodedPlanesBatch { batch, channels, width, height, data_f32 }`

期待結果:
- `data_f32` は `[C, H, W]` 順に並ぶ連続配列
- batch 版は `[B, C, H, W]` 順に並ぶ連続配列
- Python 側で `torch.float32` に変換しやすい配置であること

公開範囲:
- Rust 公開
- Python 公開
- WASM 非公開

#### 5.6.3 `encode_flat_features`

```rust
pub struct EncodedFlatFeatures {
    pub len: usize,
    pub data_f32: Vec<f32>,
}

pub struct EncodedFlatFeaturesBatch {
    pub batch: usize,
    pub len: usize,
    pub data_f32: Vec<f32>,
}

pub fn encode_flat_features(
    current: &Board,
    history: &[Board],
    config: &FeatureConfig,
) -> EncodedFlatFeatures

pub fn encode_flat_features_batch(
    boards: &[Board],
    histories: &[Vec<Board>],
    config: &FeatureConfig,
) -> EncodedFlatFeaturesBatch
```

説明:
- MLP 向けの固定長 split-flat 特徴を生成する
- 各 frame ごとに 2 本の 64 要素 occupancy を持つ
- `include_legal_mask` は 64 要素、`include_phase_plane` と `include_turn_plane` は 1 要素を追加する
- dtype は `float32` とする

公開範囲:
- Rust 公開
- Python 公開
- WASM 非公開

#### 5.6.4 `encode_nnue_features`

```rust
pub fn encode_nnue_features(board: &Board) -> NnueSparseFeatures
```

説明:
- NNUE 学習・推論用の疎特徴を生成する
- 具体的な特徴設計は別仕様とし、本関数は特徴インデックス列を返す

戻り値:
- 活性特徴インデックス列
- 各特徴値
- 視点情報

公開範囲:
- Rust 公開
- Python 公開
- WASM 公開

### 5.7 serialize モジュール

```rust
pub struct PackedBoard {
    pub black_bits: u64,
    pub white_bits: u64,
    pub side_to_move: Color,
}
```

説明:
- `Board` と 1 対 1 対応する固定長表現
- `Copy` / `Clone` / `Eq` / `Debug` を持つ軽量値オブジェクトとして扱う

#### 5.7.1 `pack_board`

```rust
pub fn pack_board(board: &Board) -> PackedBoard
```

説明:
- 盤面を固定長シリアライズ形式へ変換する

用途:
- データセット保存
- Python 受け渡し
- ハッシュキー生成補助

公開範囲:
- Rust 公開
- Python 公開
- WASM 非公開

#### 5.7.2 `unpack_board`

```rust
pub fn unpack_board(packed: PackedBoard) -> Result<Board, BoardError>
```

説明:
- 固定長シリアライズ形式から `Board` を復元する

公開範囲:
- Rust 公開
- Python 公開
- WASM 非公開

### 5.8 random_play モジュール

```rust
pub struct RandomPlayConfig {
    pub max_plies: Option<u16>,
}

pub struct RandomGameTrace {
    pub boards: Vec<Board>,
    pub moves: Vec<Option<Move>>,
    pub final_result: GameResult,
    pub final_margin_from_black: i8,
    pub plies_played: u16,
    pub reached_terminal: bool,
}

pub struct PositionSamplingConfig {
    pub num_positions: u32,
    pub min_plies: u16,
    pub max_plies: u16,
}
```

説明:
- `RandomGameTrace` はランダム対局の途中局面列と着手列、および終局ラベルを保持する
- `moves` ではパスを `None` で表す
- `max_plies = None` は終局まで進める
- `max_plies = Some(n)` はトレース記録を最大 `n` plies で止めるが、`final_result` と `final_margin_from_black` は終局まで進めた結果を返す

#### 5.8.1 `play_random_game`

```rust
pub fn play_random_game(seed: u64, config: &RandomPlayConfig) -> RandomGameTrace
```

説明:
- 初期局面から合法手の中でランダムに対局を進め、終局までの履歴を返す
- 非合法盤面は生成しない

戻り値:
- 各局面履歴
- 各着手
- 終局結果
- 最終石差

公開範囲:
- Rust 公開
- Python 公開
- WASM 非公開

#### 5.8.2 `sample_reachable_positions`

```rust
pub fn sample_reachable_positions(seed: u64, config: &PositionSamplingConfig) -> Vec<Board>
```

説明:
- 合法対局列から到達可能局面のみをサンプリングする
- 指定分布に従い複数手数帯から抽出する

公開範囲:
- Rust 公開
- Python 公開
- WASM 非公開

## 6. 探索・終盤完全読み API 仕様

### 6.1 SearchConfig

```rust
pub struct SearchConfig {
    pub max_depth: Option<u8>,
    pub max_nodes: Option<u64>,
    pub time_limit_ms: Option<u64>,
    pub exact_solver_empty_threshold: Option<u8>,
    pub use_transposition_table: bool,
    pub multi_pv: u8,
}
```

説明:
- 探索停止条件と補助機能の設定

### 6.2 SearchResult

```rust
pub struct SearchResult {
    pub best_move: Option<Move>,
    pub best_score: i16,
    pub score_kind: ScoreKind,
    pub pv: Vec<Move>,
    pub searched_nodes: u64,
    pub reached_depth: u8,
    pub is_exact: bool,
}

pub enum ScoreKind {
    MarginFromSideToMove,
    MarginFromBlack,
}
```

### 6.3 `search_best_move`

```rust
pub fn search_best_move(board: &Board, config: &SearchConfig) -> SearchResult
```

説明:
- 与えられた局面に対し探索を実行し、最善手と評価を返す
- 合法手が 0 個なら `best_move == None`
- `best_score` は仕様で固定した視点に従う

公開範囲:
- Rust 公開
- Python 公開
- WASM 条件付き公開

### 6.4 `solve_exact`

```rust
pub fn solve_exact(board: &Board, config: &SolveConfig) -> SolveResult
```

説明:
- 局面が完全読みに適した条件下で、終局までの正確な石差を返す
- 探索打ち切りではなく、必ず exact 結果を返すか、対応不可エラーを返す

戻り値:
- 最善手
- exact margin
- PV
- 探索統計

公開範囲:
- Rust 公開
- Python 公開
- WASM 非公開

### 6.5 `can_solve_exact`

```rust
pub fn can_solve_exact(board: &Board, config: &SolveConfig) -> bool
```

説明:
- 現局面が完全読み対象条件を満たすかを返す

公開範囲:
- Rust 公開
- Python 公開
- WASM 非公開

## 7. Python 公開 API 仕様

PyO3 モジュール名は `veloversi._core` とする。

### 7.1 Python 公開クラス

```python
class Board:
    black_bits: int
    white_bits: int
    side_to_move: str
```

### 7.2 Python 公開関数一覧

```python
def initial_board() -> Board: ...
def board_from_bits(black_bits: int, white_bits: int, side_to_move: str) -> Board: ...
def pack_board(board: Board) -> tuple[int, int, str]: ...
def unpack_board(packed: tuple[int, int, str]) -> Board: ...
def validate_board(board: Board) -> None: ...
def generate_legal_moves(board: Board) -> int: ...
def legal_moves_list(board: Board) -> list[int]: ...
def is_legal_move(board: Board, square: int) -> bool: ...
def apply_move(board: Board, square: int) -> Board: ...
def apply_forced_pass(board: Board) -> Board: ...
def board_status(board: Board) -> str: ...
def disc_count(board: Board) -> tuple[int, int, int]: ...
def game_result(board: Board) -> str: ...
def final_margin_from_black(board: Board) -> int: ...
def encode_planes(board: Board, history: list[Board], config: dict) -> numpy.ndarray: ...
def encode_planes_batch(boards: list[Board], histories: list[list[Board]], config: dict) -> numpy.ndarray: ...
def encode_flat_features(board: Board, history: list[Board], config: dict) -> numpy.ndarray: ...
def encode_flat_features_batch(boards: list[Board], histories: list[list[Board]], config: dict) -> numpy.ndarray: ...
def encode_nnue_features(board: Board) -> tuple[numpy.ndarray, numpy.ndarray]: ...
def transform_board(board: Board, sym: str) -> Board: ...
def transform_square(square: int, sym: str) -> int: ...
def play_random_game(seed: int, config: dict) -> dict: ...
def sample_reachable_positions(seed: int, config: dict) -> list[Board]: ...
def sample_reachable_positions(seed: int, config: dict) -> list[PyBoard]: ...
def play_random_game(seed: int, config: dict) -> dict: ...
def search_best_move(board: PyBoard, config: dict) -> dict: ...
def can_solve_exact(board: PyBoard, config: dict) -> bool: ...
def solve_exact(board: PyBoard, config: dict) -> dict: ...
```

### 7.3 Python 非公開関数

以下は内部最適化用であり Python へ露出しない。

- `apply_move_unchecked`
- ビット走査補助関数
- SIMD 特化 movegen カーネル
- transposition table 内部操作
- move ordering 専用スコア関数
- feature バッファ再利用関数
- zobrist ハッシュ内部更新関数

## 8. WASM 公開 API 前提仕様

WASM 側の詳細は別仕様書に委ねるが、Rust エンジン側は以下の制約を満たす。

- 公開型は JS 互換のプリミティブ型、配列、構造体シリアライズに限る
- `u64` の直接公開は避け、必要時は 2 本の `u32` または文字列へ変換できること
- NNUE 特徴は疎配列または密配列のどちらでも取り出せること
- パス局面判定と強制パス処理を JS から呼べること

## 9. エラー仕様

```rust
pub enum BoardError {
    OverlappingDiscs,
}

pub enum MoveError {
    IllegalMove,
    PassNotAllowed,
    TerminalBoard,
}

pub enum FeatureError {
    InvalidHistoryLength,
    PerspectiveMismatch,
    UnsupportedConfig,
}

pub enum SolveError {
    NotEligible,
    ResourceLimit,
    InternalFailure,
}
```

要件:
- Python では `ValueError` または専用例外へ変換する
- WASM ではエラーコードとメッセージへ変換する

## 10. 性能要件

### 10.1 共通要件

- `Board` は `Copy` 可能な固定長小構造体であること
- movegen と apply は分配アロケーションを行わないこと
- 主要パスでヒープ確保を避けること

### 10.2 SIMD 前提要件

- movegen 実装は SIMD 最適化可能な内部関数へ分割してよい
- SIMD 非対応環境向けフォールバックを持つこと
- SIMD 使用有無で結果が変わらないこと
- feature エンコードは連続メモリ書き込みを前提とすること

### 10.3 Python 連携要件

- バッチ特徴生成 API を将来追加しやすい構造であること
- 単局面 API は学習用データ生成で 1 秒あたり十分な件数を処理できること

## 11. テスト仕様

### 11.1 必須ユニットテスト

- 初期局面の合法手数が正しいこと
- 合法手生成と `is_legal_move` が一致すること
- 合法着手後の石数が整合すること
- 強制パス局面でのみ `apply_forced_pass` が成功すること
- 終局局面で石差が正しいこと
- 対称変換後の合法手・石差の整合性が保たれること

### 11.2 必須プロパティテスト

- `transform_board` の閉包性
- `apply_move` 後も黒白ビットが重ならないこと
- ランダム合法対局から生成した局面が常に `validate` を満たすこと

### 11.3 クロス言語整合テスト

- Rust と Python で legal move 出力が一致すること
- Rust と WASM で apply / board_status が一致すること

## 12. 将来拡張を見据えた予約項目

- 逐次差分特徴更新 API
- NNUE 推論直結の増分更新 API
- 自己対局用並列実行 API
- GPU 連携用バッチ feature API
