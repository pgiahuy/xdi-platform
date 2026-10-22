// SPDX-License-Identifier: Apache-2.0
/* components/ask/types.ts - kiểu dữ liệu các component của trang hỏi nhận từ trang. Component không biết API (AGENTS.md
mục 3); kiểu nào trùng hình dạng với hợp đồng ask (ví dụ ChoiceView với Choice) thì trang truyền thẳng dữ liệu của
response vào, không phải chép. */

export type DatasetOption = {
  id: string
  name: string
}

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
