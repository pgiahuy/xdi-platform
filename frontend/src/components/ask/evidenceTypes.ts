// SPDX-License-Identifier: Apache-2.0
/* components/ask/evidenceTypes.ts - kiểu dữ liệu pane bằng chứng (EvidencePane.tsx) nhận từ trang
(pages/AskEvidence.tsx). Mỗi tab nhận dữ liệu của mình, hoặc null kèm lý do không có, để tab trống vẫn nói vì sao.
Số của phân rã giữ nguyên chuỗi chữ số backend gửi; component chỉ nhóm nghìn trên chuỗi (cellFormat.formatDecimal),
không đổi qua Number(), vì Number làm tròn số lớn và số thập phân. */

import type { Cell, CellKind, ResultView } from './types'

export type EvidenceTab = 'sql' | 'breakdown' | 'rows' | 'steps'

/** Dữ liệu của một tab, hoặc lý do tab không có dữ liệu (chữ tiếng Việt cho người dùng). */
export type TabData<T> = { data: T; reason?: never } | { data: null; reason: string }

export type SqlView = {
  /** SQL mô hình viết; null khi mô hình chưa viết. */
  generated: string | null
  /** Có bước sửa: generated là bản mô hình đã sửa sau lần chạy lỗi. */
  repaired: boolean
  /** SQL đã chạy, sau guard; null kèm guardedMissing khi không có. */
  guarded: string | null
  /** Vì sao không có bản sau guard, ví dụ lý do guard từ chối. */
  guardedMissing: string | null
  /** Tên các bảng câu đã chạy đọc, như SQL viết; rỗng khi câu chưa có kết quả. */
  tablesRead: string[]
}

export type GroupView = {
  value: Cell
  baseValue: string
  compareValue: string
  contribution: string
  /** Chỉ có với độ đo tỷ lệ: phần do tỷ trọng của nhóm đổi (cơ cấu) và phần do tỷ lệ trong nhóm đổi (hiệu suất). */
  mixEffect: string | null
  rateEffect: string | null
  /** Có ở cả hai bên, chỉ ở bên so sánh (mới) hay chỉ ở bên gốc (không còn). */
  presence: GroupPresence
  /** % tác động do backend tính, một chữ số lẻ ("41.5"); null khi chênh lệch tổng gần 0. */
  shareOfChange: string | null
  /** % thay đổi trong nhóm; null khi nhóm mới, không còn, đổi dấu hay bên gốc không dương. */
  percentChange: string | null
  /** Hai bên khác 0 và trái dấu (lãi thành lỗ). */
  signFlip: boolean
  /** Nhóm khác thường theo luật của backend (explain/annotate.rs): in đậm. */
  outlier: boolean
}

export type GroupPresence = 'both' | 'new' | 'gone'

export type DimensionView = {
  column: string
  /** Nghĩa tiếng Việt của cột từ hồ sơ cột; null khi hồ sơ không có. */
  label: string | null
  kind: CellKind
  groups: GroupView[]
  /** Chỉ có với độ đo tỷ lệ: tổng hai phần của mọi nhóm, cộng lại bằng chênh lệch. */
  mixEffect: string | null
  rateEffect: string | null
  /** Có ít nhất một nhóm khác thường; bảng không có thì là bảng "dàn đều", backend xếp sau. */
  hasOutliers: boolean
}

/** Cách chia backend bỏ khỏi bảng phân rã hay khỏi lời tóm tắt, kèm lý do. */
export type DroppedView = {
  column: string
  label: string | null
  /** separates: mọi giá trị chỉ có ở một bên (tên khác của hai bên đang so); duplicates: chia y hệt cột duplicateOf;
   * follows: mỗi giá trị của cột đang so (duplicateOf) chỉ có một giá trị của cột này. */
  reason: 'separates_slices' | 'duplicates' | 'follows_slices'
  duplicateOf: string | null
  baseOnly: string[]
  compareOnly: string[]
}

export type VolumePriceView = {
  volumeLabel: string
  priceLabel: string
  baseVolume: string
  compareVolume: string
  basePrice: string
  comparePrice: string
  volumeEffect: string
  priceEffect: string
  ifVolumeUnchanged: string
}

export type BreakdownView = {
  measure: string
  /** Hai kỳ hay hai nhóm: quyết định chữ chiều (tăng, giảm hay cao hơn, thấp hơn). */
  comparison: Comparison
  baseLabel: string
  compareLabel: string
  baseValue: string
  compareValue: string
  /** Số dòng của từng lát cắt, chuỗi chữ số. */
  baseRows: string
  compareRows: string
  /** Độ đo đếm dòng: số đo của lát cắt chính là quy mô của nó. */
  countsRows: boolean
  change: string
  /** Chênh lệch so với lát gốc, phần trăm; null khi lát gốc không dương. */
  percentChange: string | null
  /** Cột có hai giá trị đang so (so hai nhóm), kèm nghĩa tiếng Việt; null khi so hai kỳ. */
  sliceColumn: string | null
  sliceColumnLabel: string | null
  /** Chiều giải thích chênh lệch tốt nhất đứng đầu, đúng thứ tự backend gửi. */
  dimensions: DimensionView[]
  /** Cách chia đã bỏ, để tab Phân rã nói vì sao không có bảng của chúng. */
  dropped: DroppedView[]
  volumePrice: VolumePriceView | null
  /** Câu trả lời chính và các ý ngắn: do mô hình viết khi byModel (đã qua hai cổng kiểm của backend), không thì câu
   * khuôn do code viết từ số đã tính. */
  summaryTitle: string
  summaryPoints: SummaryPointView[]
  byModel: boolean
  /** Chỗ mô hình chấm thấy chưa rõ nghĩa (không chắc sai) trong insight của mô hình; rỗng khi không có. */
  warnings: string[]
}

export type SummaryPointView = {
  /** Đầu ý, in đậm, ví dụ "Lý do chính". */
  lead: string
  text: string
}

export type Comparison = 'periods' | 'groups'

export type Slice = 'base' | 'compare'

/** Một khóa của ô được bấm: giá trị gốc của một cột dimension hay time trong dòng đó, đúng như backend gửi. */
export type DrillKeyView = {
  column: string
  value: Cell
  /** Kind dùng để hiện giá trị, có thể là 'date' với cột timestamp toàn nửa đêm; không gửi lên backend. */
  kind: CellKind
}

// Một kiểu chung cho hai chỗ bấm để tab Dòng nguồn, lần gọi drill và chữ "Đang xem" đi cùng một đường; from cho biết
// request gửi breakdown hay cell (hợp đồng ở drill/contract.rs).
/** Chỗ được bấm: một dòng phân rã (lát cắt, cột, giá trị nhóm) hoặc một ô số đo của bảng kết quả (khóa của dòng, tên
 * cột số đo, giá trị ô để so với tổng các dòng nguồn; không có khóa là toàn bộ dòng nguồn của câu hỏi). */
export type DrillTarget =
  | { from: 'breakdown'; slice: Slice; column: string; value: Cell; kind: CellKind }
  | { from: 'cell'; keys: DrillKeyView[]; measure: string; value: Cell; kind: CellKind }

/** Ô nào của bảng kết quả bấm được: chỉ số các cột số đo, chỉ số các cột khóa (dimension, time) của dòng. */
export type CellDrill = {
  measures: number[]
  keys: number[]
  onDrill: (target: DrillTarget) => void
}

/** Một trang dòng nguồn: vị trí dòng đầu, cỡ trang, tổng số dòng nguồn của chỗ được bấm. */
export type RowsPage = {
  offset: number
  limit: number
  total: number
}

/** Độ đo của chỗ được bấm, backend tính lại trên mọi dòng nguồn (DrillMeasure): sum là tổng cột rowColumn của các
 * dòng, count là số dòng, other là hàm tổng hợp khác (trung bình, tỷ lệ) tính lại. clicked là ô đã bấm khi so được
 * (ô của bảng kết quả); null với dòng phân rã, vì số của phân rã có thể đã chia theo số nhóm. */
export type RowsMeasure = {
  column: string
  rule: 'sum' | 'count' | 'other'
  value: Cell
  kind: CellKind
  rowColumn: string | null
  clicked: Cell
  compared: boolean
}

/** Dòng nguồn đến từ bảng nào và cột nào lấy từ bảng nối (câu nối bảng), kèm độ đo tính lại. */
export type RowsSource = {
  table: string | null
  joinedColumns: string[]
  measure: RowsMeasure | null
}

/** Tab dòng nguồn theo trạng thái lần truy gần nhất; idle kèm lời nhắc bấm ở đâu. */
export type RowsView =
  | { state: 'idle'; hint: string }
  | { state: 'loading'; target: DrillTarget }
  | { state: 'error'; target: DrillTarget; message: string }
  | { state: 'ready'; target: DrillTarget; sql: string; result: ResultView; page: RowsPage; source: RowsSource }

/** ok: bước đạt; fallback: bước viết lời không dùng được lời của mô hình nên câu trả lời dùng câu của hệ thống (câu hỏi
 * vẫn có trả lời, không phải lỗi); error: bước lỗi. */
export type StepState = 'ok' | 'fallback' | 'error'

export type StepView = {
  /** Nhãn tiếng Việt do giao diện đặt cho tên bước. */
  label: string
  durationMs: number
  state: StepState
  detail: string | null
  /** Bước có gọi mô hình ngôn ngữ. */
  byModel: boolean
  /** Cờ kết quả của bước, thay cho nhãn "có mô hình": loại câu ở bước phân loại (quantitative, causal), kết luận hai
   * cổng kiểm ở bước viết lời (match, warning, mismatch: lời không hiện, thay bằng câu khuôn); null ở bước khác hay khi
   * backend không gửi. */
  flag: StepFlag | null
}

export type StepFlag = 'quantitative' | 'causal' | 'match' | 'warning' | 'mismatch'

/** Một lần gọi mô hình ngôn ngữ của câu hỏi. */
export type ModelCallView = {
  /** Nhãn tiếng Việt do giao diện đặt cho loại việc. */
  label: string
  durationMs: number
  ok: boolean
  /** null khi nhà cung cấp không báo số token (mock, lần gọi lỗi). */
  inputTokens: number | null
  outputTokens: number | null
}

/** Tab Các bước: các bước theo thứ tự, rồi các lần gọi mô hình. */
export type StepsView = {
  steps: StepView[]
  calls: ModelCallView[]
}
