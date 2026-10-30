// SPDX-License-Identifier: Apache-2.0
/* components/metrics/types.ts - kiểu dữ liệu các component của màn Chỉ số nhận từ trang (pages/MetricsPage.tsx). Trang
dựng các kiểu này từ dữ liệu backend (src/api/metrics.ts), để component không phụ thuộc dạng JSON của API. */

export type MetricItem = {
  id: string
  name: string
  description: string
  /** Biểu thức tổng hợp SQL, ví dụ SUM("Order Item Total"). */
  expression: string
  filter: string | null
  timeColumn: string | null
  unit: string | null
  version: number
}

export type Aggregation = 'sum' | 'avg'

export type ColumnItem = {
  name: string
  dataType: string
  /** dimension, measure, time, key, text, ignore. */
  role: string
  description: string | null
  /** Chỉ cột measure có; cột khác là null. */
  aggregation: Aggregation | null
}

/** Chỉ số admin đang soạn, mọi trường là chữ như trong ô nhập; trang đổi chữ rỗng thành null trước khi gửi. */
export type MetricDraft = {
  name: string
  description: string
  expression: string
  filter: string
  timeColumn: string
  unit: string
}

/** Tên trường backend trả trong lỗi (ApiError.field), trùng tên trường JSON của request. */
export type MetricField = 'name' | 'description' | 'expression' | 'filter' | 'time_column' | 'unit'
