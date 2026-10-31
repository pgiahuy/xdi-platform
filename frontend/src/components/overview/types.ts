// SPDX-License-Identifier: Apache-2.0
/* components/overview/types.ts - kiểu dữ liệu khung "Tổng quan dữ liệu" nhận từ trang (pages/overviewView.ts dựng từ
response của api/overview.ts). Component không biết API (AGENTS.md mục 3); biểu đồ dùng lại kiểu của trang hỏi để vẽ
bằng cùng ResultChart. */

import type { ChartView, ResultView } from '@/components/ask/types'

/** Vai trò cột trong hồ sơ; giao diện xếp cột theo nhóm của vai trò. */
export type OverviewRole = 'dimension' | 'measure' | 'time' | 'key' | 'text'

export type OverviewValueView = {
  value: string
  /** Bí danh đầu của giá trị; null khi không có. */
  label: string | null
  rows: number
}

export type OverviewColumnView = {
  name: string
  /** Nhãn tiếng Việt (bí danh đầu do lớp ngữ nghĩa viết); null khi chưa có. */
  label: string | null
  description: string | null
  /** database: comment trong database nguồn; model: lớp ngữ nghĩa viết; null khi không có mô tả. */
  descriptionSource: string | null
  role: OverviewRole
  unit: string | null
  sensitive: boolean
  /** % ô trống, 0 tới 100; null khi người xem bị lọc dòng. */
  nullPercent: number | null
  distinctCount: number | null
  /** Giá trị nhiều dòng nhất trước; rỗng với cột không phải chia nhóm, cột nhạy cảm, hay khi người xem bị lọc dòng. */
  values: OverviewValueView[]
}

export type OverviewMetricView = {
  name: string
  description: string
  unit: string | null
  confirmed: boolean
}

export type OverviewChartView = {
  /** measure_over_time: độ đo chính theo kỳ; rows_over_time: số dòng theo kỳ; rows_by_value: số dòng theo giá trị. */
  kind: 'measure_over_time' | 'rows_over_time' | 'rows_by_value'
  /** Cột của hồ sơ biểu đồ nói về: cột chia nhóm, hay cột thời gian. */
  column: string
  grain: 'day' | 'month' | null
  result: ResultView
  chart: ChartView
}

export type OverviewView = {
  name: string
  /** Mô tả bảng ghi trong database nguồn (COMMENT ON TABLE); null với file nạp tay hay bảng không có. */
  description: string | null
  /** Số phiên bản hiện hành; null khi dataset chưa có dữ liệu. */
  version: number | null
  /** Số dòng người xem đọc được; null khi chưa đếm. */
  rowCount: number | null
  timeColumn: string | null
  timeStart: string | null
  timeEnd: string | null
  /** Người xem bị lọc dòng theo vai dữ liệu: số đếm của hồ sơ không có. */
  restricted: boolean
  columns: OverviewColumnView[]
  metrics: OverviewMetricView[]
  /** Nhãn của độ đo chính (tên chỉ số hay nhãn cột); null khi không có. */
  mainMeasure: string | null
  charts: OverviewChartView[]
  /** Lời báo các phần chưa có, backend viết sẵn. */
  missing: string[]
}
