// SPDX-License-Identifier: Apache-2.0
/* components/ask/types.ts - kiểu dữ liệu các component của trang hỏi nhận từ trang. Component không biết API (AGENTS.md
mục 3); kiểu nào trùng hình dạng với hợp đồng ask (ví dụ ChoiceView với Choice) thì trang truyền thẳng dữ liệu của
response vào, không phải chép. */

export type DatasetOption = {
  id: string
  name: string
}

/** Hồ sơ của phiên bản hiện hành: profiling khi chưa xong lớp ngữ nghĩa (chưa có câu gợi ý, mô tả cột), failed khi lập
 * hồ sơ lỗi, ready khi xong hoặc không có gì để báo. */
export type ProfileProgress = 'profiling' | 'failed' | 'ready'

export type ChoiceOptionView = {
  id: string
  label: string
}

/** Một chỗ hiểu được nhiều cách: giả định đã chọn sẵn (selected có giá trị) hoặc câu hỏi lại (selected null). */
export type ChoiceView = {
  id: string
  label: string
  selected: string | null
  options: ChoiceOptionView[]
}

/** Thẻ "Đã hiểu là": nhãn tiếng Việt của từng mục, backend viết sẵn. */
export type UnderstandingView = {
  measures: string[]
  filters: string[]
  groupBy: string[]
  orderBy: string[]
  limit: string | null
  timeRange: string | null
  assumptions: ChoiceView[]
}

/** Cách đọc ô của một cột, như ValueKind của hợp đồng: number là số JavaScript giữ đúng; decimal là chuỗi chữ số
 * (BIGINT, DECIMAL...) để không làm tròn; date, timestamp là chuỗi ISO 8601. */
export type CellKind = 'text' | 'number' | 'decimal' | 'boolean' | 'date' | 'timestamp' | 'other'

export type Cell = number | string | boolean | null

export type ResultColumnView = {
  name: string
  kind: CellKind
}

/** Các dòng backend đã gửi (tối đa 1000) và tổng số dòng của câu SQL. */
export type ResultView = {
  columns: ResultColumnView[]
  rows: Cell[][]
  totalRows: number
  /** Gửi ít dòng hơn totalRows. */
  truncated: boolean
  /** totalRows chạm trần số dòng của guard: kết quả đầy đủ có thể còn nhiều hơn, nên ghi "ít nhất". */
  limitReached: boolean
}

/** Gợi ý biểu đồ của backend: x và y là tên cột trong ResultView.columns. */
export type ChartView = {
  kind: 'bar' | 'line'
  x: string
  y: string[]
}
