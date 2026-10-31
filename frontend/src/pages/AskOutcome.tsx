// SPDX-License-Identifier: Apache-2.0
/* pages/AskOutcome.tsx - phần dưới ô hỏi của trang hỏi: hiện một AskResponse theo status (hợp đồng ở
backend/src/service/ask/contract.rs). answered: lời trả lời và phần giải thích (insight của câu thường, hay lời giải
thích của câu "tại sao"), biểu đồ nếu backend gợi ý, bảng kết quả, thẻ "Cách tính"; needs_choice: màn câu hỏi lại; rejected, failed: lời báo của backend. Cảnh báo hiện với mọi
status. Khối này là phần trả lời của một lượt trong khung trò chuyện; bong bóng câu hỏi và thẻ bọc ngoài do
ChatTurn (AskConversation.tsx). Dựng dữ liệu cho component từ response; việc gửi câu hỏi do AskPage.
Biểu đồ tải riêng (lazy) vì Recharts nặng, lý do ở components/ask/ResultChart.tsx; nó nằm trong ErrorBoundary riêng
để chunk tải lỗi hay lỗi khi vẽ chỉ làm mất biểu đồ, bảng kết quả vẫn còn. */

import { lazy, Suspense } from 'react'
import {
  isRepeatedClarification,
  type AskResponse,
  type AskVariables,
  type Insight,
  type Understanding,
} from '@/api/ask'
import { AnswerSummary } from '@/components/ask/AnswerSummary'
import { AnswerText } from '@/components/ask/AnswerText'
import { AskWarnings } from '@/components/ask/AskWarnings'
import { ClarificationForm } from '@/components/ask/ClarificationForm'
import { DataRangeNote } from '@/components/ask/DataRangeNote'
import { ResultTable } from '@/components/ask/ResultTable'
import type { DrillTarget } from '@/components/ask/evidenceTypes'
import type { UnderstandingView } from '@/components/ask/types'
import { UnderstandingCard } from '@/components/ask/UnderstandingCard'
import { WhyOverview } from '@/components/ask/WhyOverview'
import { ErrorBoundary } from '@/components/ErrorBoundary'
import { FormMessage } from '@/components/forms/FormMessage'
import { cn } from '@/lib/utils'
import { buildBreakdown } from './breakdownView'
import { buildCellDrill, buildChartView, buildResultView } from './resultView'

const ResultChart = lazy(() =>
  import('@/components/ask/ResultChart').then((m) => ({
    default: m.ResultChart,
  })),
)

/** Một lần hỏi đã có response, kèm đúng request đã gửi: hỏi lại cần cả hai. */
export type Answer = {
  variables: AskVariables
  response: AskResponse
}

type Props = {
  answer: Answer
  /** Đang hỏi lại: câu trả lời cũ vẫn hiện, mờ đi, không bấm được. */
  pending: boolean
  /** Lượt cuối của khung trò chuyện: thẻ "Cách tính" mở sẵn; lượt cũ thì thu gọn. */
  latest: boolean
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

export function AskOutcome(props: Props) {
  const { answer, pending, latest, onCellDrill, onChangeAssumption, onAnswerClarifications } = props
  const { response } = answer
  // Không có số nào thì câu trả lời đã nói đủ; cảnh báo kết quả rỗng hay cột toàn ô trống chỉ lặp lại ý đó bằng lời kỹ
  // thuật, nên không hiện.
  const warnings = hasNoData(response)
    ? response.warnings.filter((warning) => warning.code !== 'empty_result' && warning.code !== 'all_null')
    : response.warnings
  return (
    <div aria-busy={pending} className={cn('flex min-w-0 flex-col gap-5', pending && 'pointer-events-none opacity-60')}>
      <AskWarnings messages={warnings.map((warning) => warning.message)} />
      <OutcomeBody
        answer={answer}
        pending={pending}
        latest={latest}
        onCellDrill={onCellDrill}
        onChangeAssumption={onChangeAssumption}
        onAnswerClarifications={onAnswerClarifications}
      />
    </div>
  )
}

function OutcomeBody({ answer, pending, latest, onCellDrill, onChangeAssumption, onAnswerClarifications }: Props) {
  const { response, variables } = answer
  switch (response.status) {
    case 'answered':
      return (
        <AnsweredBody
          response={response}
          pending={pending}
          latest={latest}
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
  latest: boolean
  onCellDrill: (target: DrillTarget) => void
  onChangeAssumption: (choiceId: string, optionId: string) => void
}

// Thứ tự đọc: lời trả lời trước (answer_text và phần giải thích, hay ba số lớn và lời giải thích của câu "tại sao"), rồi
// biểu đồ và bảng, cuối cùng thẻ "Cách tính": người hỏi cần con số trước, cách hệ thống hiểu câu hỏi là phần để kiểm
// lại. Bảng phân rã đủ mọi chiều và SQL đứng sau từng con số thuộc pane minh chứng (#52).
function AnsweredBody({ response, pending, latest, onCellDrill, onChangeAssumption }: AnsweredProps) {
  const understanding = response.understanding && (
    <UnderstandingCard
      understanding={buildUnderstandingView(response.understanding)}
      onChangeAssumption={onChangeAssumption}
      disabled={pending}
      defaultOpen={latest}
    />
  )
  const range = response.data_range
  const dataRange = range && (
    <DataRangeNote from={range.from} to={range.to} label={range.label ?? range.column} partial={range.partial} />
  )
  // Không có số nào: chỉ câu trả lời và thẻ "Cách tính" (để thấy đã hỏi kỳ, nhóm nào), không bảng trống hay
  // biểu đồ rỗng. Backend cũ không gửi câu trả lời cho kết quả 0 dòng nên có câu dự phòng.
  if (hasNoData(response)) {
    return (
      <>
        <p className="text-[15px] leading-relaxed">{response.answer_text ?? noDataText}</p>
        {dataRange}
        {understanding}
      </>
    )
  }
  const result = response.result && buildResultView(response.result)
  const chart = result && response.chart && buildChartView(response.chart, result)
  const breakdown = buildBreakdown(response).data
  return (
    <>
      {response.answer_text && (
        <AnswerText
          text={response.answer_text}
          byModel={response.answer_text_by_model ?? false}
          // Đoạn trả lời là lời của phần giải thích khi do mô hình viết, nên các chỗ chưa rõ của lời đó đi kèm.
          warnings={response.answer_text_by_model ? (response.insight?.summary_warnings ?? []) : []}
        />
      )}
      {dataRange}
      {response.insight ? (
        // Câu thường: phần giải thích theo hình dạng kết quả (#227). Phân rã tự động của nó (explanation) chỉ hiện ở tab
        // Phân rã của pane minh chứng, để thẻ không thành một câu "tại sao" thứ hai. Danh sách dòng mà đoạn trả lời là câu
        // của code thì không hiện: đoạn trả lời đã dựng từ đúng các câu đó (số dòng và điều kiện lọc, hay một dòng kể bằng
        // lời), hiện nữa chỉ lặp lại.
        !(response.insight.kind === 'rows' && !response.answer_text_by_model) && (
          <AnswerSummary {...buildInsightView(response.insight, response.answer_text)} />
        )
      ) : (
        breakdown && <WhyOverview breakdown={breakdown} />
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
          cellDrill={buildCellDrill(response, onCellDrill)}
          collapsible
        />
      )}
      {understanding}
    </>
  )
}

const noDataText = 'Không có dữ liệu để trả lời câu hỏi này.'

/** Câu chính, các ý và lời của mô hình (chỉ khi lời đã qua bước kiểm) cho AnswerSummary. Lời đó đã đứng đầu thẻ làm
 * đoạn trả lời (answer_text, backend dùng lại lời của phần giải thích) thì không hiện lần nữa, chỉ còn các câu của code. */
function buildInsightView(insight: Insight, answerText: string | null) {
  const shown = insight.summary_by_model && insight.summary !== answerText
  return {
    title: insight.title,
    points: insight.points.map((point) => ({ lead: point.lead, text: point.text })),
    modelSummary: shown ? insight.summary : null,
  }
}

/** Câu thường đã trả lời mà kết quả không có số nào: không dòng nào, hay mọi ô đều trống (tổng trên tập rỗng). */
function hasNoData(response: AskResponse): boolean {
  const result = response.result
  if (response.status !== 'answered' || response.explanation || !result) return false
  return result.rows.every((row) => row.every((cell) => cell === null))
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
