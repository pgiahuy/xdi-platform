// SPDX-License-Identifier: Apache-2.0
/* components/overview/OverviewCharts.tsx - biểu đồ dựng sẵn của khung tổng quan: độ đo chính theo kỳ, số dòng theo kỳ,
rồi số dòng theo giá trị của từng cột chia nhóm. Backend đã chọn loại biểu đồ theo cùng luật IBCS với câu trả lời, nên
vẽ bằng chính ResultChart của trang hỏi: tải riêng (lazy) vì Recharts nặng, trong ErrorBoundary để chunk lỗi chỉ mất
biểu đồ (lý do ở components/ask/ResultChart.tsx).
Dataset nhiều cột chia nhóm thì khung rất dài, nên chỉ hiện sẵn vài biểu đồ số dòng theo giá trị, còn lại mở bằng nút.
Bấm tiêu đề một biểu đồ thì điền câu hỏi của biểu đồ đó vào ô hỏi. */

import { lazy, Suspense, useState } from 'react'
import { ErrorBoundary } from '@/components/ErrorBoundary'
import { Button } from '@/components/ui/button'
import { buildChartQuestion } from './questions'
import type { OverviewChartView, OverviewView } from './types'

const ResultChart = lazy(() => import('@/components/ask/ResultChart').then((m) => ({ default: m.ResultChart })))

/** Số biểu đồ số dòng theo giá trị hiện sẵn. 4 chọn ở đây để khung không quá dài, chưa đo. */
const VALUE_CHARTS_SHOWN = 4

type Props = {
  overview: OverviewView
  onPick: (question: string) => void
}

export function OverviewCharts({ overview, onPick }: Props) {
  const [allCharts, setAllCharts] = useState(false)
  const timeCharts = overview.charts.filter((chart) => chart.kind !== 'rows_by_value')
  const valueCharts = overview.charts.filter((chart) => chart.kind === 'rows_by_value')
  const shownValues = allCharts ? valueCharts : valueCharts.slice(0, VALUE_CHARTS_SHOWN)
  const hidden = valueCharts.length - shownValues.length
  if (overview.charts.length === 0) return null
  return (
    <div className="flex flex-col gap-3">
      <div className="grid gap-3 xl:grid-cols-2">
        {[...timeCharts, ...shownValues].map((chart) => (
          <ChartCard
            key={`${chart.kind}:${chart.column}`}
            chart={chart}
            question={buildChartQuestion(chart, overview)}
            onPick={onPick}
          />
        ))}
      </div>
      {valueCharts.length > VALUE_CHARTS_SHOWN && (
        <Button variant="outline" size="sm" className="self-start" onClick={() => setAllCharts((all) => !all)}>
          {allCharts ? 'Thu gọn biểu đồ' : `Hiện thêm ${hidden} biểu đồ số dòng theo giá trị`}
        </Button>
      )}
    </div>
  )
}

type CardProps = {
  chart: OverviewChartView
  question: string
  onPick: (question: string) => void
}

function ChartCard({ chart, question, onPick }: CardProps) {
  return (
    <section className="flex min-w-0 flex-col gap-2 rounded-lg border bg-background p-3">
      <button
        type="button"
        onClick={() => onPick(question)}
        className="self-start text-left text-sm font-medium text-label underline-offset-2 hover:underline"
        aria-label={`${question}: điền câu hỏi này vào ô hỏi`}
      >
        {question}
      </button>
      <ErrorBoundary fallback={<p className="text-sm text-muted-foreground">Không vẽ được biểu đồ này.</p>}>
        <Suspense fallback={<p className="text-sm text-muted-foreground">Đang tải biểu đồ...</p>}>
          <ResultChart result={chart.result} chart={chart.chart} />
        </Suspense>
      </ErrorBoundary>
    </section>
  )
}
