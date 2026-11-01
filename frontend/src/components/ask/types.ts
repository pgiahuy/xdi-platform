// SPDX-License-Identifier: Apache-2.0
/* components/ask/types.ts - kiểu dữ liệu các component của trang hỏi nhận từ trang. Component không biết API (AGENTS.md
mục 3); kiểu nào trùng hình dạng với hợp đồng ask (ví dụ ChoiceView với Choice) thì trang truyền thẳng dữ liệu của
response vào, không phải chép. */

/** Một bảng trong ô chọn dataset. */
export type ScopeTableOption = {
  id: string
  name: string
  /** Schema trong database nguồn; null với file. */
  schema: string | null
}

/** Một dataset trong ô chọn: database (mở ra chọn được một bảng) hay file (#257). */
export type ScopeDatasetOption = {
  id: string
  name: string
  kind: 'database' | 'file'
  tables: ScopeTableOption[]
}

/** Hồ sơ của phiên bản hiện hành: profiling khi chưa xong lớp ngữ nghĩa (chưa có câu gợi ý, mô tả cột), failed khi lập
 * hồ sơ lỗi, no_suggestions khi lớp cấu trúc xong mà mô hình đang chạy không lập được lớp ngữ nghĩa (vẫn hỏi được),
 * ready khi xong hoặc không có gì để báo. */
export type ProfileProgress = 'profiling' | 'failed' | 'no_suggestions' | 'ready'

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

/** Thẻ "Cách tính": nhãn tiếng Việt của từng mục, backend viết sẵn. */
export type UnderstandingView = {
  measures: string[]
  filters: string[]
  groupBy: string[]
  orderBy: string[]
  limit: string | null
  timeRange: string | null
  assumptions: ChoiceView[]
  /** Nhãn của các mục lấy từ lượt trước của cuộc trò chuyện; thẻ ghi "từ câu trước" cạnh chúng. */
  fromPrevious?: string[]
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

/** Gợi ý biểu đồ của backend theo bộ luật ở ARCHITECTURE mục 6b; mọi tên là tên cột trong ResultView.columns. */
export type ChartView = {
  /** bar: thanh ngang so sánh nhóm; column: cột đứng theo thời gian; line: đường có điểm ở từng kỳ; stacked_column: cột
   * chồng chia theo các nhóm của series. */
  kind: 'bar' | 'column' | 'line' | 'stacked_column'
  /** Trục nhóm (bar) hay trục thời gian (các loại khác). */
  x: string
  /** Các số đo cần vẽ. */
  y: string[]
  /** single: một số đo; grouped: cạnh nhau trên một thang đo vì cùng đơn vị; panels: mỗi số đo một biểu đồ nhỏ. */
  layout: 'single' | 'grouped' | 'panels'
  /** Cột nhóm thành các phần của cột chồng, hay các đường khi mỗi nhóm một đường; null với loại khác. */
  series: string | null
  /** Số đo không cộng được (trung bình, tỷ lệ): vẽ cột hẹp, không gộp "Khác". */
  ratio: string[]
  /** Số nhóm tối đa được vẽ, lớn trước; null là vẽ hết. */
  top: number | null
  /** Các nhóm sau top được cộng thành một mục "Khác"; false thì bỏ và ghi số nhóm đã bỏ. */
  otherBucket: boolean
}
