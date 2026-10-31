// SPDX-License-Identifier: Apache-2.0
/* pages/reportView.ts - đổi hợp đồng báo cáo (api/reports.ts) sang dữ liệu component của components/report. Bảng kết
quả và gợi ý biểu đồ dùng lại bộ đổi của trang hỏi (resultView.ts), nên một mục vẽ bảng, biểu đồ đúng như câu trả lời. */

import type { ReportItem, ReportSummary } from '@/api/reports'
import type { ChartView } from '@/components/ask/types'
import type { ReportItemView, ReportListEntry } from '@/components/report/types'
import { buildChartView, buildResultView } from './resultView'

export function buildReportEntry(report: ReportSummary, href: string): ReportListEntry {
  return {
    id: report.id,
    title: report.title,
    automatic: report.kind === 'automatic',
    itemCount: report.item_count,
    updatedAt: report.updated_at,
    href,
  }
}

export function buildReportItemView(item: ReportItem): ReportItemView {
  const u = item.understanding
  return {
    id: item.id,
    question: item.question,
    askedAt: item.asked_at,
    causal: item.question_type === 'causal',
    tables: item.tables.map((t) => ({
      name: t.name,
      version: t.version,
      capturedAt: t.captured_at,
      isCurrent: t.is_current,
    })),
    answer: item.answer && { text: item.answer.text, byModel: item.answer.by_model, warnings: item.answer.warnings },
    numberCheck: item.number_check,
    meaningCheck: item.meaning_check,
    facts: item.facts,
    method: u && {
      measures: u.measures.map((m) => m.label),
      filters: u.filters.map((f) => f.label),
      groupBy: u.group_by.map((g) => g.label),
      timeRange: u.time_range?.label ?? null,
      order: [...u.order_by.map((o) => o.label), ...(u.limit ? [u.limit.label] : [])],
    },
    sql: item.sql,
    result: item.result && buildResultView(item.result),
    breakdown: item.breakdown && {
      measure: item.breakdown.measure,
      baseLabel: item.breakdown.base_label,
      compareLabel: item.breakdown.compare_label,
      baseValue: item.breakdown.base_value,
      compareValue: item.breakdown.compare_value,
      change: item.breakdown.change,
      dimension: item.breakdown.dimension,
      groups: item.breakdown.groups.map((g) => ({
        name: g.name,
        baseValue: g.base_value,
        compareValue: g.compare_value,
        contribution: g.contribution,
        shareOfChange: g.share_of_change,
      })),
    },
    notes: item.notes,
    error: item.error,
  }
}

/** Gợi ý biểu đồ của mục, null khi không vẽ được (luật ở resultView.ts). */
export function buildReportChart(item: ReportItem): ChartView | null {
  if (!item.result || !item.chart) return null
  return buildChartView(item.chart, buildResultView(item.result))
}
