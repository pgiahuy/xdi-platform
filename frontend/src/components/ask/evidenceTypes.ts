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
}

export type DimensionView = {
  column: string
  kind: CellKind
  groups: GroupView[]
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

export type Slice = 'base' | 'compare'

/** Chỗ được bấm trong bảng phân rã: một lát cắt, một cột, một giá trị nhóm. */
export type DrillTarget = {
  slice: Slice
  column: string
  value: Cell
  kind: CellKind
}

/** Tab dòng nguồn theo trạng thái lần truy gần nhất. */
export type RowsView =
  | { state: 'idle' }
  | { state: 'loading'; target: DrillTarget }
  | { state: 'error'; target: DrillTarget; message: string }
  | { state: 'ready'; target: DrillTarget; sql: string; result: ResultView; sample: boolean }

export type StepView = {
  /** Nhãn tiếng Việt do giao diện đặt cho tên bước. */
  label: string
  durationMs: number
  ok: boolean
  detail: string | null
  /** Bước có gọi mô hình ngôn ngữ. */
  byModel: boolean
}
