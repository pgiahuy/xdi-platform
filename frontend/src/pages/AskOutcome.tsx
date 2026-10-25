// SPDX-License-Identifier: Apache-2.0
/* pages/AskOutcome.tsx - phần dưới ô hỏi của trang hỏi: hiện một AskResponse theo status (hợp đồng ở
backend/src/service/ask_contract.rs). answered: thẻ "Đã hiểu là", lời giải thích của câu "tại sao", biểu đồ nếu
backend gợi ý, bảng kết quả; needs_choice: màn câu hỏi lại; rejected, failed: lời báo của backend. Cảnh báo hiện với mọi
status. Cả khối nằm trong một thẻ, tiêu đề là chính câu đã hỏi, để đọc kết quả mà không phải nhìn lại ô hỏi (câu trong ô
có thể đã sửa). Dựng dữ liệu cho component từ response; việc gửi câu hỏi do AskPage.
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
import { AnswerSummary } from '@/components/ask/AnswerSummary'
import { AskSection } from '@/components/ask/AskSection'
import { AskWarnings } from '@/components/ask/AskWarnings'
import { ClarificationForm } from '@/components/ask/ClarificationForm'
import { SampleBadge } from '@/components/ask/SampleModeSwitch'
import { ResultTable } from '@/components/ask/ResultTable'
import type { ChartView, ResultView, UnderstandingView } from '@/components/ask/types'
import { UnderstandingCard } from '@/components/ask/UnderstandingCard'
import { ErrorBoundary } from '@/components/ErrorBoundary'
import { FormMessage } from '@/components/forms/FormMessage'
import { cn } from '@/lib/utils'
import { buildResultView } from './resultView'

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
  onChangeAssumption: (choiceId: string, optionId: string) => void
  onAnswerClarifications: (picks: { choiceId: string; optionId: string }[]) => void
}

// Hợp đồng luôn có message ở ba status này; câu dự phòng chỉ để không hiện khối trống nếu backend thiếu.
const fallbackMessage = {
  needs_choice: 'Câu hỏi hiểu được theo nhiều cách. Chọn một cách rồi hỏi lại.',
  rejected: 'Câu SQL bị từ chối.',
  failed: 'Không chạy được câu SQL.',
}

export function AskOutcome({ answer, pending, onChangeAssumption, onAnswerClarifications }: Props) {
  const { response, variables } = answer
  return (
    <AskSection title={variables.question} prominent>
      <div aria-busy={pending} className={cn('flex flex-col gap-5', pending && 'pointer-events-none opacity-60')}>
        {import.meta.env.DEV && variables.fromSamples && <SampleBadge />}
        <AskWarnings messages={response.warnings.map((warning) => warning.message)} />
        <OutcomeBody
          answer={answer}
          pending={pending}
          onChangeAssumption={onChangeAssumption}
          onAnswerClarifications={onAnswerClarifications}
        />
      </div>
    </AskSection>
  )
}

function OutcomeBody({ answer, pending, onChangeAssumption, onAnswerClarifications }: Props) {
  const { response, variables } = answer
  switch (response.status) {
    case 'answered':
      return <AnsweredBody response={response} pending={pending} onChangeAssumption={onChangeAssumption} />
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
  onChangeAssumption: (choiceId: string, optionId: string) => void
}

// Lời giải thích của câu "tại sao" là câu trả lời, nên đứng ngay sau thẻ "Đã hiểu là", trên bảng kết quả; bảng phân
// rã và SQL đứng sau từng con số thuộc pane bằng chứng (#52).
function AnsweredBody({ response, pending, onChangeAssumption }: AnsweredProps) {
  const result = response.result && buildResultView(response.result)
  const chart = result && response.chart && buildChartView(response.chart, result)
  return (
    <>
      {response.understanding && (
        <UnderstandingCard
          understanding={buildUnderstandingView(response.understanding)}
          onChangeAssumption={onChangeAssumption}
          disabled={pending}
        />
      )}
      {response.explanation && (
        <AnswerSummary
          summary={response.explanation.summary}
          byModel={response.explanation.summary_by_model}
        />
      )}
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
  if (!names.has(chart.x) || chart.y.length === 0 || !chart.y.every((name) => names.has(name))) return null
  return { kind: chart.kind, x: chart.x, y: chart.y }
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
  }
}
