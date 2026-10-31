// SPDX-License-Identifier: Apache-2.0
/* components/report/types.ts - dữ liệu các component của trang Báo cáo nhận qua props. Trang dựng chúng từ hợp đồng
API (pages/reportView.ts), nên component không biết gì về backend. */

import type { ResultView } from '@/components/ask/types'

/** Một báo cáo trong danh sách. */
export type ReportListEntry = {
  id: string
  title: string
  automatic: boolean
  itemCount: number
  /** ISO 8601. */
  updatedAt: string
  href: string
}

/** Kiểm số: lời của mô hình đã qua, bị thay bằng câu của hệ thống, hay câu do hệ thống viết nên không cần kiểm. */
export type NumberCheckView = 'passed' | 'replaced' | 'not_needed'

/** Kiểm nghĩa: kết luận của lần gọi mô hình riêng so nghĩa của lời với các ý đã tính. */
export type MeaningCheckView = 'match' | 'warning' | 'mismatch'

/** Phần "Cách tính" của một mục: nhãn tiếng Việt backend viết sẵn. */
export type MethodView = {
  measures: string[]
  filters: string[]
  groupBy: string[]
  timeRange: string | null
  order: string[]
}

export type BreakdownGroupView = {
  name: string
  baseValue: string
  compareValue: string
  contribution: string
  shareOfChange: string | null
}

export type BreakdownView = {
  measure: string
  baseLabel: string
  compareLabel: string
  baseValue: string
  compareValue: string
  change: string
  dimension: string
  groups: BreakdownGroupView[]
}

export type TableVersionView = {
  name: string
  version: number
  /** ISO 8601. */
  capturedAt: string
  isCurrent: boolean
}

export type ReportItemView = {
  id: string
  question: string
  /** ISO 8601. */
  askedAt: string
  causal: boolean
  tables: TableVersionView[]
  answer: { text: string; byModel: boolean; warnings: string[] } | null
  numberCheck: NumberCheckView | null
  meaningCheck: MeaningCheckView | null
  facts: { title: string; points: string[] } | null
  method: MethodView | null
  sql: string | null
  result: ResultView | null
  breakdown: BreakdownView | null
  notes: string[]
  error: string | null
}
