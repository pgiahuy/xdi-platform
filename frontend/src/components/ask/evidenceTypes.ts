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
  /** Những gì guard ghi là đã thêm, ví dụ "Thêm LIMIT 100000". */
  guardNotes: string[]
}

export type GroupView = {
  value: Cell
  baseValue: string
  compareValue: string
  contribution: string
  /** Chỉ có với độ đo tỷ lệ: phần do tỷ trọng của nhóm đổi (cơ cấu) và phần do tỷ lệ trong nhóm đổi (hiệu suất). */
  mixEffect: string | null
  rateEffect: string | null
}

export type DimensionView = {
  column: string
  kind: CellKind
  groups: GroupView[]
  /** Chỉ có với độ đo tỷ lệ: tổng hai phần của mọi nhóm, cộng lại bằng chênh lệch. */
  mixEffect: string | null
  rateEffect: string | null
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
  change: string
  /** Chiều giải thích chênh lệch tốt nhất đứng đầu, đúng thứ tự backend gửi. */
  dimensions: DimensionView[]
  volumePrice: VolumePriceView | null
  summary: string
  /** False: câu viết theo khuôn, không phải lời của mô hình. */
  summaryByModel: boolean
}

export type Comparison = 'periods' | 'groups'

export type Slice = 'base' | 'compare'

/** Một khóa của ô được bấm: giá trị gốc của một cột dimension hay time trong dòng đó, đúng như backend gửi. */
export type DrillKeyView = {
  column: string
  value: Cell
  kind: CellKind
}

// Một kiểu chung cho hai chỗ bấm để tab Dòng nguồn, lần gọi drill và chữ "Đang xem" đi cùng một đường; from cho biết
// request gửi breakdown hay cell (hợp đồng ở drill/contract.rs).
/** Chỗ được bấm: một dòng phân rã (lát cắt, cột, giá trị nhóm) hoặc một ô số đo của bảng kết quả (khóa của dòng và tên
 * cột số đo; không có khóa là toàn bộ dòng nguồn của câu hỏi). */
export type DrillTarget =
  | { from: 'breakdown'; slice: Slice; column: string; value: Cell; kind: CellKind }
  | { from: 'cell'; keys: DrillKeyView[]; measure: string }

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

/** Tab dòng nguồn theo trạng thái lần truy gần nhất; idle kèm lời nhắc bấm ở đâu. */
export type RowsView =
  | { state: 'idle'; hint: string }
  | { state: 'loading'; target: DrillTarget }
  | { state: 'error'; target: DrillTarget; message: string }
  | { state: 'ready'; target: DrillTarget; sql: string; result: ResultView; page: RowsPage; sample: boolean }

export type StepView = {
  /** Nhãn tiếng Việt do giao diện đặt cho tên bước. */
  label: string
  durationMs: number
  ok: boolean
  detail: string | null
  /** Bước có gọi mô hình ngôn ngữ. */
  byModel: boolean
}
