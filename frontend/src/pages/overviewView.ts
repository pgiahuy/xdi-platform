// SPDX-License-Identifier: Apache-2.0
/* pages/overviewView.ts - đổi DatasetOverviewResponse (api/overview.ts) sang OverviewView của components/overview. Bảng và
gợi ý biểu đồ đổi bằng đúng hàm của câu trả lời (resultView.ts), nên biểu đồ dựng sẵn vẽ như biểu đồ của câu hỏi. Biểu
đồ trỏ tới cột không có trong bảng thì bỏ, như ở trang hỏi. */

import type { DatasetOverview } from '@/api/overview'
import type { OverviewChartView, OverviewView } from '@/components/overview/types'
import { buildChartView, buildResultView } from './resultView'

export function buildOverviewView(overview: DatasetOverview): OverviewView {
  return {
    name: overview.name,
    version: overview.version?.number ?? null,
    rowCount: overview.summary.row_count,
    timeColumn: overview.summary.time_column,
    timeStart: overview.summary.time_start,
    timeEnd: overview.summary.time_end,
    restricted: overview.summary.restricted,
    columns: overview.columns.map((column) => ({
      name: column.name,
      label: column.label,
      description: column.description,
      role: column.role,
      unit: column.unit,
      sensitive: column.sensitive,
      nullPercent: column.null_percent,
      distinctCount: column.distinct_count,
      values: column.values,
    })),
    metrics: overview.metrics.map((metric) => ({
      name: metric.name,
      description: metric.description,
      unit: metric.unit,
      confirmed: metric.confirmed,
    })),
    mainMeasure: overview.main_measure?.label ?? null,
    charts: overview.charts.flatMap((chart): OverviewChartView[] => {
      const result = buildResultView(chart.result)
      const view = buildChartView(chart.chart, result)
      return view ? [{ kind: chart.kind, column: chart.column, grain: chart.grain, result, chart: view }] : []
    }),
    missing: overview.missing.map((part) => part.message),
  }
}
