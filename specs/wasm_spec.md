# オセロ WASM 実行系仕様書

## 1. 目的

本仕様書は、ブラウザまたは JavaScript 実行環境でオセロエンジンと NNUE 推論を利用するための WASM 実行系仕様を定義する。

本書は以下を対象とする。

- JS へ公開する API
- Rust から WASM への公開境界
- 盤面・合法手・推論結果の受け渡し形式
- NNUE 重み読み込み形式
- UI 接続の契約

アルゴリズムの詳細、UI デザイン、通信プロトコルの詳細は対象外とする。

## 2. 構成

推奨構成は以下とする。

```text
crates/
  engine-core/
  engine-search/
  engine-wasm/
web/
  pkg/
  src/
```

- `engine-core` を基盤ロジックとして利用する
- `engine-search` は必要最小限のみ取り込む
- `engine-wasm` は JS 公開専用ラッパとする

## 3. 対象ユースケース

### 3.1 必須ユースケース

- ブラウザで人間対 AI 対戦を行う
- 合法手表示を行う
- AI の推奨手と評価値を表示する
- 学習済み NNUE 重みを埋め込んで利用する

### 3.2 任意ユースケース

- ローカル対戦棋譜の再生
- Web Worker 上での探索
- 端末 SIMD 対応時の高速化

## 4. JS 公開 API 方針

- 呼び出しは同期 API とする
- 重い探索は Worker 利用を前提に非同期ラッパを上位層で提供してよい
- 盤面情報はシリアライズ済み構造体または TypedArray で返す
- bitboard の `u64` は JS へ直接露出しない

## 5. 公開型仕様

### 5.1 WasmBoard

```rust
#[wasm_bindgen]
pub struct WasmBoard {
    black_lo: u32,
    black_hi: u32,
    white_lo: u32,
    white_hi: u32,
    side_to_move: u8,
}
```

説明:
- JS 互換のため `u64` を上下 32bit へ分割して保持する
- `side_to_move`: `0 = black`, `1 = white`

### 5.2 WasmDiscCount

```rust
#[wasm_bindgen]
pub struct WasmDiscCount {
    pub black: u8,
    pub white: u8,
    pub empty: u8,
}
```

### 5.3 WasmSearchResult

```rust
#[wasm_bindgen]
pub struct WasmSearchResult {
    pub best_move: i16,
    pub best_score: i16,
    pub is_exact: bool,
    pub searched_nodes: u32,
    pub reached_depth: u8,
}
```

仕様:
- `best_move == -1` は合法手なしを表す
- `best_score` は side-to-move 視点の石差スコアとする

### 5.4 WasmInferenceResult

```rust
#[wasm_bindgen]
pub struct WasmInferenceResult {
    pub margin: f32,
    pub win_prob: f32,
    pub draw_prob: f32,
    pub loss_prob: f32,
}
```

## 6. 公開 API 仕様

### 6.1 `new_initial_board`

```rust
#[wasm_bindgen]
pub fn new_initial_board() -> WasmBoard
```

説明:
- 初期局面を生成する

### 6.2 `board_from_parts`

```rust
#[wasm_bindgen]
pub fn board_from_parts(
    black_lo: u32,
    black_hi: u32,
    white_lo: u32,
    white_hi: u32,
    side_to_move: u8,
) -> Result<WasmBoard, JsValue>
```

説明:
- JS 側で保持している盤面を WASM 盤面へ変換する
- 入力整合性を検査する

### 6.3 `board_to_array`

```rust
#[wasm_bindgen]
pub fn board_to_array(board: &WasmBoard) -> js_sys::Uint32Array
```

説明:
- 盤面を `[black_lo, black_hi, white_lo, white_hi, side_to_move]` 形式の配列へ変換する

### 6.4 `generate_legal_moves_mask`

```rust
#[wasm_bindgen]
pub fn generate_legal_moves_mask(board: &WasmBoard) -> js_sys::Uint32Array
```

説明:
- 合法手マスクを `u64` 相当の 2 要素 `Uint32Array` として返す
- `[mask_lo, mask_hi]`

### 6.5 `generate_legal_moves_list`

```rust
#[wasm_bindgen]
pub fn generate_legal_moves_list(board: &WasmBoard) -> js_sys::Uint8Array
```

説明:
- 合法手一覧を 0..63 の配列で返す
- パスは含めない

### 6.6 `is_legal_move`

```rust
#[wasm_bindgen]
pub fn is_legal_move(board: &WasmBoard, square: u8) -> bool
```

説明:
- 指定着手が合法かを返す

### 6.7 `apply_move`

```rust
#[wasm_bindgen]
pub fn apply_move(board: &WasmBoard, square: u8) -> Result<WasmBoard, JsValue>
```

説明:
- 合法手を 1 手適用した新盤面を返す

### 6.8 `board_status`

```rust
#[wasm_bindgen]
pub fn board_status(board: &WasmBoard) -> u8
```

返却値:
- `0 = ongoing`
- `1 = forced_pass`
- `2 = terminal`

### 6.9 `apply_forced_pass`

```rust
#[wasm_bindgen]
pub fn apply_forced_pass(board: &WasmBoard) -> Result<WasmBoard, JsValue>
```

説明:
- 合法手 0 の場合のみ手番を交代する

### 6.10 `disc_count`

```rust
#[wasm_bindgen]
pub fn disc_count(board: &WasmBoard) -> WasmDiscCount
```

### 6.11 `game_result`

```rust
#[wasm_bindgen]
pub fn game_result(board: &WasmBoard) -> i8
```

返却値:
- `1 = black win`
- `0 = draw`
- `-1 = white win`

### 6.12 `final_margin_from_black`

```rust
#[wasm_bindgen]
pub fn final_margin_from_black(board: &WasmBoard) -> i8
```

### 6.13 `infer_nnue`

```rust
#[wasm_bindgen]
pub fn infer_nnue(board: &WasmBoard) -> WasmInferenceResult
```

説明:
- 盤面から NNUE 特徴を生成し、埋め込み済み重みで推論する
- 結果を margin と WDL 確率で返す

契約:
- `margin` は side-to-move 視点の正規化石差予測
- WDL 確率の総和は 1 に近いこと

### 6.14 `search_best_move`

```rust
#[wasm_bindgen]
pub fn search_best_move(board: &WasmBoard, depth: u8, node_limit: u32) -> WasmSearchResult
```

説明:
- ブラウザ利用向け簡易探索 API
- 重い終盤完全読みは含めてもよいが、標準では軽量構成とする

### 6.15 `load_nnue_weights`

```rust
#[wasm_bindgen]
pub fn load_nnue_weights(bytes: &[u8]) -> Result<(), JsValue>
```

説明:
- 外部バイト列から NNUE 重みを読み込む
- 初期実装では埋め込み専用でもよいが、動的差し替え拡張のため API を予約する

### 6.16 `current_model_info`

```rust
#[wasm_bindgen]
pub fn current_model_info() -> JsValue
```

説明:
- 現在読み込まれているモデルのバージョン、重み形式、量子化情報を返す

## 7. Rust 内部モジュール仕様

### 7.1 `ffi`

役割:
- `engine-core` の `Board` と `WasmBoard` の相互変換
- JS 向けエラー変換

### 7.2 `nnue_runtime`

役割:
- NNUE 特徴生成
- 量子化重み管理
- SIMD 対応推論
- フォールバック推論

内部専用関数例:

```rust
fn board_to_internal(board: &WasmBoard) -> Result<Board, JsValue>
fn board_from_internal(board: &Board) -> WasmBoard
fn decode_weights(bytes: &[u8]) -> Result<QuantizedWeights, WeightError>
fn infer_quantized(features: &NnueSparseFeatures, weights: &QuantizedWeights) -> WasmInferenceResult
```

これらは JS に公開しない。

### 7.3 `search_runtime`

役割:
- WASM から使う簡易探索設定の変換
- 探索結果の JS 互換変換

### 7.4 `panic_hook`

役割:
- デバッグ時に Rust panic をブラウザコンソールへ出す

## 8. NNUE 重み仕様

### 8.1 重み形式

最低限以下を保持する。

- モデル識別子
- 特徴数
- 中間層サイズ
- 量子化スケール
- 層重み
- 層バイアス

### 8.2 埋め込み方式

次の 2 方式を許容する。

1. Rust 静的埋め込み
   - `include_bytes!` でバイナリ埋め込み
2. JS から動的ロード
   - 初期化後に `load_nnue_weights` で読み込み

標準は 1 とする。

### 8.3 推論精度一致要件

- Python 推論と Rust/WASM 推論で所定誤差以内に一致すること
- 量子化前後で勝敗符号が大きく崩れないこと

## 9. JS 利用仕様

### 9.1 標準利用例

```javascript
import init, {
  new_initial_board,
  generate_legal_moves_list,
  apply_move,
  board_status,
  apply_forced_pass,
  infer_nnue,
  search_best_move,
} from "./pkg/engine_wasm.js";

await init();
let board = new_initial_board();
const legal = generate_legal_moves_list(board);
board = apply_move(board, legal[0]);
const evalResult = infer_nnue(board);
```

### 9.2 UI 側責務

- 盤面表示
- 合法手ハイライト
- 強制パス時の自動進行または通知
- 推論結果の表示
- 探索 API 呼び出し制御

## 10. Worker 連携仕様

探索をメインスレッドから分離する場合、以下のメッセージ形式を標準とする。

### 10.1 request

```json
{
  "type": "search",
  "board": [0, 0, 0, 0, 0],
  "depth": 8,
  "nodeLimit": 50000
}
```

### 10.2 response

```json
{
  "type": "search_result",
  "bestMove": 19,
  "bestScore": 12,
  "isExact": false,
  "searchedNodes": 50000,
  "reachedDepth": 8
}
```

## 11. エラー仕様

JS へ返すエラーは `JsValue` とし、内容は以下を基本とする。

```json
{
  "code": "ILLEGAL_MOVE",
  "message": "specified move is not legal"
}
```

代表コード:
- `INVALID_BOARD`
- `ILLEGAL_MOVE`
- `PASS_NOT_ALLOWED`
- `MODEL_NOT_LOADED`
- `INVALID_WEIGHTS`
- `INTERNAL_ERROR`

## 12. SIMD と互換性仕様

### 12.1 SIMD 方針

- ブラウザ SIMD 対応時は NNUE 内部演算で SIMD を利用してよい
- 非対応環境ではスカラー実装へ自動フォールバックする

### 12.2 結果整合性

- SIMD と非 SIMD で推論結果の誤差は許容範囲内で一致すること
- 合法手生成、着手、終局判定は完全一致であること

## 13. パフォーマンス要件

- 1 手ごとの合法手生成は UI 応答に十分な速度であること
- NNUE 推論はブラウザ上で対話的な待ち時間を超えないこと
- 初期ロード時の重みロードは現実的サイズに収まること

## 14. テスト仕様

### 14.1 API テスト

- 初期局面生成
- 合法手一覧一致
- 非合法手拒否
- 強制パス局面処理
- 終局石差取得
- NNUE 推論出力 shape と範囲

### 14.2 整合テスト

- Rust ネイティブと WASM で legal moves 一致
- Rust ネイティブと WASM で apply 結果一致
- Python 量子化前推論と WASM 推論の近似一致

### 14.3 ブラウザ結合テスト

- JS から API 呼び出し可能
- Worker 経由探索が動作
- UI で一連の対局が成立

## 15. 将来拡張予約

- 複数 NNUE モデル切り替え
- 教師モデル ONNX 推論のブラウザ持ち込み
- GPU Web API との連携
- 棋譜入出力対応
