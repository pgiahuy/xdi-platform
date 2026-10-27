// SPDX-License-Identifier: Apache-2.0
/* pages/AskOutcome.tsx - phần dưới ô hỏi của trang hỏi: hiện một AskResponse theo status (hợp đồng ở
backend/src/service/ask/contract.rs). answered: thẻ "Đã hiểu là", lời giải thích của câu "tại sao", biểu đồ nếu
backend gợi ý, bảng kết quả; needs_choice: màn câu hỏi lại; rejected, failed: lời báo của backend. Cảnh báo hiện với mọi
status. Khối này là phần trả lời của một lượt trong khung trò chuyện; bong bóng câu hỏi và thẻ bọc ngoài do
ChatTurn (AskConversation.tsx). Dựng dữ liệu cho component từ response; việc gửi câu hỏi do AskPage.
Biểu đồ tải riêng (lazy) vì Recharts nặng, lý do ở components/ask/ResultChart.tsx; nó nằm trong ErrorBoundary riêng
để chunk tải lỗi hay lỗi khi vẽ chỉ làm mất biểu đồ, bảng kết quả vẫn còn. */

import { lazy, Suspense } from 'react'
import {
  isRepeatedClarification,
  type AskResponse,
  type AskVariables,
  type ChartSuggestion,
  type Understanding,
} from '@/api/ask'
import { AskWarnings } from '@/components/ask/AskWarnings'
import { ClarificationForm } from '@/components/ask/ClarificationForm'
import { SampleBadge } from '@/components/ask/SampleModeSwitch'
import { ResultTable } from '@/components/ask/ResultTable'
import type { DrillTarget } from '@/components/ask/evidenceTypes'
import type { ChartView, ResultView, UnderstandingView } from '@/components/ask/types'
import { UnderstandingCard } from '@/components/ask/UnderstandingCard'
import { WhyOverview } from '@/components/ask/WhyOverview'
import { ErrorBoundary } from '@/components/ErrorBoundary'
import { FormMessage } from '@/components/forms/FormMessage'
import { cn } from '@/lib/utils'
import { buildBreakdown } from './breakdownView'
import { buildCellDrill, buildResultView } from './resultView'

const ResultChart = lazy(() => import('@/components/ask/ResultChart').then((m) => ({ default: m.ResultChart })))

/** Một lần hỏi đã có response, kèm đúng request đã gửi: hỏi lại cần cả hai. */
export type Answer = {
  variables: AskVariables
  response: AskResponse
}

type Props = {
  answer: Answer
  /** Đang hỏi lại: câu trả lời cũ vẫn hiện, mờ đi, không bấm được. */
  pending: boolean
  /** Bấm một ô số đo của bảng kết quả: trang mở tab Dòng nguồn của ô đó. */
  onCellDrill: (target: DrillTarget) => void
  onChangeAssumption: (choiceId: string, optionId: string) => void
  onAnswerClarifications: (picks: { choiceId: string; optionId: string }[]) => void
}

// Hợp đồng luôn có message ở ba status này; câu dự phòng chỉ để không hiện khối trống nếu backend thiếu.
const fallbackMessage = {
  needs_choice: 'Câu hỏi hiểu được theo nhiều cách. Chọn một cách rồi hỏi lại.',
  rejected: 'Câu SQL bị từ chối.',
  failed: 'Không chạy được câu SQL.',
}

export function AskOutcome({ answer, pending, onCellDrill, onChangeAssumption, onAnswerClarifications }: Props) {
  const { response, variables } = answer
  return (
    <div aria-busy={pending} className={cn('flex min-w-0 flex-col gap-5', pending && 'pointer-events-none opacity-60')}>
      {import.meta.env.DEV && variables.fromSamples && <SampleBadge />}
      <AskWarnings messages={response.warnings.map((warning) => warning.message)} />
      <OutcomeBody
        answer={answer}
        pending={pending}
        onCellDrill={onCellDrill}
        onChangeAssumption={onChangeAssumption}
        onAnswerClarifications={onAnswerClarifications}
      />
    </div>
  )
}

function OutcomeBody({ answer, pending, onCellDrill, onChangeAssumption, onAnswerClarifications }: Props) {
  const { response, variables } = answer
  switch (response.status) {
    case 'answered':
      return (
        <AnsweredBody
          response={response}
          pending={pending}
          onCellDrill={onCellDrill}
          onChangeAssumption={onChangeAssumption}
        />
      )
    case 'needs_choice': {
      const message = response.message ?? fallbackMessage.needs_choice
      if (isRepeatedClarification(variables.answered ?? [], response)) {
        const hint = 'Lựa chọn vừa gửi không áp dụng được; sửa câu hỏi rồi hỏi lại.'
        return <FormMessage kind="error">{`${message} ${hint}`}</FormMessage>
      }
      return (
        <ClarificationForm
          // Mỗi response một form mới: lựa chọn đang đánh dấu của lần trước không sót sang.
          key={JSON.stringify(variables)}
          message={message}
          clarifications={response.clarifications}
          onSubmit={onAnswerClarifications}
          disabled={pending}
        />
      )
    }
    case 'rejected':
    case 'failed':
      return <FormMessage kind="error">{response.message ?? fallbackMessage[response.status]}</FormMessage>
  }
}

type AnsweredProps = {
  response: AskResponse
  pending: boolean
  onCellDrill: (target: DrillTarget) => void
  onChangeAssumption: (choiceId: string, optionId: string) => void
}

// Câu "tại sao": ba số lớn, lời giải thích và đóng góp theo chiều đầu tiên là câu trả lời, nên đứng ngay sau thẻ "Đã
// hiểu là", trên bảng kết quả; bảng phân rã đủ mọi chiều và SQL đứng sau từng con số thuộc pane bằng chứng (#52).
function AnsweredBody({ response, pending, onCellDrill, onChangeAssumption }: AnsweredProps) {
  const result = response.result && buildResultView(response.result)
  const chart = result && response.chart && buildChartView(response.chart, result)
  const breakdown = buildBreakdown(response).data
  return (
    <>
      {response.understanding && (
        <UnderstandingCard
          understanding={buildUnderstandingView(response.understanding)}
          onChangeAssumption={onChangeAssumption}
          disabled={pending}
        />
      )}
      {breakdown && <WhyOverview breakdown={breakdown} />}
      {result && chart && (
        // key theo câu hỏi: câu trả lời mới thì thử vẽ lại. lazy() nhớ lần tải chunk đã lỗi, nên chunk lỗi chỉ hết
        // khi tải lại trang; bảng vẫn dùng được trong lúc đó.
        <ErrorBoundary
          key={`chart-${response.query_id}`}
          fallback={<p className="text-sm text-muted-foreground">Không vẽ được biểu đồ; số liệu ở bảng bên dưới.</p>}
        >
          <Suspense fallback={<p className="text-sm text-muted-foreground">Đang tải biểu đồ...</p>}>
            <ResultChart result={result} chart={chart} />
          </Suspense>
        </ErrorBoundary>
      )}
      {result && (
        // key theo câu hỏi trong lịch sử: kết quả mới thì bảng về trang 1.
        <ResultTable
          key={`table-${response.query_id}`}
          result={result}
          fileName={`ket-qua-${(response.query_id ?? 'cau-hoi').slice(0, 8)}.csv`}
          cellDrill={buildCellDrill(response, onCellDrill)}
        />
      )}
    </>
  )
}

// Gợi ý trỏ tới cột không có trong kết quả thì bỏ biểu đồ, vẫn hiện bảng: vẽ sai cột còn tệ hơn không vẽ. Không có
// dòng nào thì cũng bỏ, khỏi vẽ một trục trống.
function buildChartView(chart: ChartSuggestion, result: ResultView): ChartView | null {
  if (result.rows.length === 0) return null
  const names = new Set(result.columns.map((column) => column.name))
  const columnsExist =
    names.has(chart.x) &&
    chart.y.length > 0 &&
    chart.y.every((name) => names.has(name)) &&
    (chart.series === null || names.has(chart.series))
  if (!columnsExist) return null
  return {
    kind: chart.kind,
    x: chart.x,
    y: chart.y,
    layout: chart.layout,
    series: chart.series,
    ratio: chart.ratio,
    top: chart.top,
    otherBucket: chart.other_bucket,
  }
}

function buildUnderstandingView(understanding: Understanding): UnderstandingView {
  const labels = (items: { label: string }[]) => items.map((item) => item.label)
  return {
    measures: labels(understanding.measures),
    filters: labels(understanding.filters),
    groupBy: labels(understanding.group_by),
    orderBy: labels(understanding.order_by),
    limit: understanding.limit?.label ?? null,
    timeRange: understanding.time_range?.label ?? null,
    assumptions: understanding.assumptions,
    fromPrevious: [
      ...understanding.measures,
      ...understanding.filters,
      ...understanding.group_by,
      ...(understanding.time_range ? [understanding.time_range] : []),
    ]
      .filter((item) => item.from_previous)
      .map((item) => item.label),
  }
}
