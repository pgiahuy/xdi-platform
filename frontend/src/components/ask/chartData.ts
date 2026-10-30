// SPDX-License-Identifier: Apache-2.0
/* components/ask/chartData.ts - dữ liệu vẽ của ResultChart, lấy từ chính các dòng của bảng kết quả theo gợi ý của
backend (ChartView, luật ở ARCHITECTURE mục 6b): xếp nhóm lớn trước, cắt số nhóm, gộp "Khác", và xoay dòng thành chuỗi
cho cột chồng hay mỗi nhóm một đường. Tách khỏi ResultChart.tsx để phần tính không lẫn với phần vẽ.

Mỗi điểm giữ cả số để vẽ (khóa của chuỗi) lẫn chữ đã định dạng của ô gốc (khóa kèm _text) cho tooltip: decimal đổi
sang số có thể làm tròn, tooltip phải hiện đúng con số trong bảng. Ô của mục "Khác" là tổng tính ở đây, nên chữ của nó
định dạng từ số. */

import { formatCell, toChartNumber } from './cellFormat'
import type { Cell, ChartView, ResultView } from './types'

/** Tên mục gộp các nhóm sau top, như IBCS gọi phần còn lại. */
export const OTHER_LABEL = 'Khác'

export type ChartPoint = Record<string, string | number | null>

export type ChartSeries = {
  /** Khóa của chuỗi trong mỗi điểm. */
  key: string
  /** Tên hiện ở chú giải và tooltip: tên số đo, hay giá trị nhóm khi mỗi nhóm một chuỗi. */
  name: string
  /** Số đo không cộng được: cột hẹp. */
  ratio: boolean
}

export type ChartData = {
  points: ChartPoint[]
  series: ChartSeries[]
  /** Số nhóm sau top đã gộp vào "Khác". */
  merged: number
  /** Số nhóm sau top bị bỏ vì không gộp được (số đo không cộng được, hay bảng chỉ có một phần các dòng). */
  leftOut: number
}

/** Dữ liệu vẽ cho các số đo measures (cả chart.y, hay một số đo khi mỗi số đo một biểu đồ nhỏ). */
export function buildChartData(result: ResultView, chart: ChartView, measures: string[]): ChartData {
  if (chart.series === null) {
    return buildOneKeyData(result, chart, measures)
  }
  return buildSeriesData(result, chart, chart.series, measures[0])
}

/** Mỗi dòng một điểm. Thanh ngang xếp lớn trước theo số đo đầu, rồi cắt top nhóm. Thời gian giữ thứ tự của bảng. */
function buildOneKeyData(result: ResultView, chart: ChartView, measures: string[]): ChartData {
  const xIndex = columnIndex(result, chart.x)
  const measureIndexes = measures.map((name) => columnIndex(result, name))
  const series = measures.map((name, i) => ({ key: `y${i}`, name, ratio: chart.ratio.includes(name) }))
  let points = result.rows.map((row) => {
    const point: ChartPoint = { x: axisLabel(formatCell(row[xIndex], result.columns[xIndex].kind)) }
    measureIndexes.forEach((columnIndex, i) => {
      point[`y${i}`] = toChartNumber(row[columnIndex])
      point[`y${i}_text`] = formatCell(row[columnIndex], result.columns[columnIndex].kind)
    })
    return point
  })
  if (chart.kind !== 'bar') {
    return { points, series, merged: 0, leftOut: 0 }
  }
  points = [...points].sort((a, b) => numberOf(b.y0) - numberOf(a.y0))
  if (chart.top === null || points.length <= chart.top) {
    return { points, series, merged: 0, leftOut: 0 }
  }
  const kept = points.slice(0, chart.top)
  const rest = points.slice(chart.top)
  if (!chart.otherBucket) {
    return { points: kept, series, merged: 0, leftOut: rest.length }
  }
  const other: ChartPoint = { x: OTHER_LABEL }
  series.forEach((s) => {
    const sum = rest.reduce((total, point) => total + numberOf(point[s.key]), 0)
    other[s.key] = sum
    other[`${s.key}_text`] = formatCell(sum, 'number')
  })
  return { points: [...kept, other], series, merged: rest.length, leftOut: 0 }
}

/**
 * Xoay dòng (kỳ, nhóm, số đo) thành mỗi kỳ một điểm, mỗi nhóm một chuỗi. Cột chồng xếp nhóm theo tổng lớn trước rồi
 * cắt top nhóm, phần còn lại gộp "Khác" hay bỏ; mỗi nhóm một đường giữ thứ tự gặp trong bảng (backend đã giới hạn số
 * nhóm). Kỳ giữ thứ tự của bảng.
 */
function buildSeriesData(result: ResultView, chart: ChartView, seriesColumn: string, measure: string): ChartData {
  const xIndex = columnIndex(result, chart.x)
  const groupIndex = columnIndex(result, seriesColumn)
  const valueIndex = columnIndex(result, measure)
  const valueKind = result.columns[valueIndex].kind
  const periods: string[] = []
  const groups: string[] = []
  const totals = new Map<string, number>()
  const cells = new Map<string, Cell>()
  for (const row of result.rows) {
    const period = axisLabel(formatCell(row[xIndex], result.columns[xIndex].kind))
    const group = formatCell(row[groupIndex], result.columns[groupIndex].kind)
    if (!periods.includes(period)) periods.push(period)
    if (!groups.includes(group)) groups.push(group)
    totals.set(group, (totals.get(group) ?? 0) + numberOf(toChartNumber(row[valueIndex])))
    cells.set(`${period}\u0000${group}`, row[valueIndex])
  }
  let drawn = groups
  let rest: string[] = []
  if (chart.kind === 'stacked_column') {
    const byTotal = [...groups].sort((a, b) => (totals.get(b) ?? 0) - (totals.get(a) ?? 0))
    const top = chart.top ?? byTotal.length
    drawn = byTotal.slice(0, top)
    rest = byTotal.slice(top)
  }
  const merge = rest.length > 0 && chart.otherBucket
  const series: ChartSeries[] = drawn.map((group, i) => ({ key: `s${i}`, name: group, ratio: false }))
  if (merge) series.push({ key: 'other', name: OTHER_LABEL, ratio: false })
  const points = periods.map((period) => {
    const point: ChartPoint = { x: period }
    drawn.forEach((group, i) => {
      const cell = cells.get(`${period}\u0000${group}`) ?? null
      point[`s${i}`] = toChartNumber(cell)
      point[`s${i}_text`] = formatCell(cell, valueKind)
    })
    if (merge) {
      let sum = 0
      for (const group of rest) {
        sum += numberOf(toChartNumber(cells.get(`${period}\u0000${group}`) ?? null))
      }
      point.other = sum
      point.other_text = formatCell(sum, 'number')
    }
    return point
  })
  return {
    points,
    series,
    merged: merge ? rest.length : 0,
    leftOut: merge ? 0 : rest.length,
  }
}

function columnIndex(result: ResultView, name: string): number {
  return result.columns.findIndex((column) => column.name === name)
}

/** Ô trống hay không đọc được số thì tính là 0 khi xếp và cộng; khi vẽ vẫn để trống (null). */
function numberOf(value: string | number | null | undefined): number {
  return typeof value === 'number' ? value : 0
}

// Mốc thời gian gộp theo ngày, tháng, quý đều rơi vào nửa đêm; bỏ " 00:00:00" trên trục cho nhãn đủ chỗ ở màn hẹp.
// Bảng kết quả cũng bỏ giờ khi cả cột đều là nửa đêm (buildDisplayKind trong cellFormat.ts).
function axisLabel(text: string): string {
  return text.replace(/ 00:00:00$/, '')
}
