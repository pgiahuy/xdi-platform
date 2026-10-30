// SPDX-License-Identifier: Apache-2.0
/* components/overview/questions.ts - câu hỏi điền sẵn vào ô hỏi khi bấm một cột, một giá trị, một chỉ số hay một biểu
đồ của khung tổng quan (#198), ví dụ bấm Market thì "Doanh thu theo Market". Chỉ điền, không gửi: người dùng sửa rồi
mới hỏi, nên câu chỉ cần là điểm bắt đầu hợp lý, không cần đúng mọi ý.
Câu dựng từ độ đo chính (backend chọn: chỉ số đã xác nhận, hay cột tiền, số lượng) và kỳ của biểu đồ theo thời gian;
không có độ đo chính thì hỏi số dòng. Cột số đo hỏi tổng khi cộng được (units.ts), không thì trung bình. Cột gọi bằng nhãn tiếng Việt khi có, vì mô hình nhận cả bí danh lẫn tên cột. */

import type { OverviewChartView, OverviewColumnView, OverviewMetricView, OverviewView } from './types'
import { isAdditiveUnit } from './units'

const ROWS = 'Số dòng'

export function nameOf(column: OverviewColumnView): string {
  return column.label ?? column.name
}

/** "tháng" hay "ngày", theo kỳ backend chọn cho biểu đồ thời gian; mặc định tháng. */
function periodOf(overview: OverviewView): string {
  const grain = overview.charts.find((chart) => chart.grain !== null)?.grain
  return grain === 'day' ? 'ngày' : 'tháng'
}

function measureOf(overview: OverviewView): string {
  return overview.mainMeasure ?? ROWS
}

export function buildColumnQuestion(column: OverviewColumnView, overview: OverviewView): string {
  const name = nameOf(column)
  const period = periodOf(overview)
  switch (column.role) {
    case 'dimension':
      return `${measureOf(overview)} theo ${name}`
    case 'measure': {
      const total = `${isAdditiveUnit(column.unit) ? 'Tổng' : 'Trung bình'} ${name}`
      return overview.timeColumn ? `${total} theo ${period}` : total
    }
    case 'time':
      return column.name === overview.timeColumn
        ? `${measureOf(overview)} theo ${period}`
        : `${measureOf(overview)} theo ${period} của ${name}`
    case 'key':
      return `Có bao nhiêu ${name} khác nhau`
    case 'text':
      return `10 ${name} có nhiều dòng nhất`
  }
}

export function buildValueQuestion(column: OverviewColumnView, value: string, overview: OverviewView): string {
  const subject = `${nameOf(column)} ${value}`
  return overview.timeColumn
    ? `${measureOf(overview)} theo ${periodOf(overview)} của ${subject}`
    : `${measureOf(overview)} của ${subject}`
}

export function buildMetricQuestion(metric: OverviewMetricView, overview: OverviewView): string {
  return overview.timeColumn ? `${metric.name} theo ${periodOf(overview)}` : metric.name
}

export function buildChartQuestion(chart: OverviewChartView, overview: OverviewView): string {
  const period = chart.grain === 'day' ? 'ngày' : 'tháng'
  if (chart.kind === 'measure_over_time') return `${measureOf(overview)} theo ${period}`
  if (chart.kind === 'rows_over_time') return `${ROWS} theo ${period}`
  const column = overview.columns.find((c) => c.name === chart.column)
  return `${ROWS} theo ${column ? nameOf(column) : chart.column}`
}
