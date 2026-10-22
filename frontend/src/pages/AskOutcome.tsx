// SPDX-License-Identifier: Apache-2.0
/* pages/AskOutcome.tsx - phần dưới ô hỏi của trang hỏi: hiện một AskResponse theo status (hợp đồng ở
backend/src/service/ask_contract.rs). answered: thẻ "Đã hiểu là", lời giải thích của câu "tại sao"; needs_choice: màn
câu hỏi lại; rejected, failed: lời báo của backend. Cảnh báo hiện với mọi status. Dựng dữ liệu cho component từ
response; việc gửi câu hỏi do AskPage. */

import { isRepeatedClarification, type AskResponse, type AskVariables, type Understanding } from '@/api/ask'
import { AnswerSummary } from '@/components/ask/AnswerSummary'
import { AskWarnings } from '@/components/ask/AskWarnings'
import { ClarificationForm } from '@/components/ask/ClarificationForm'
import { SampleBadge } from '@/components/ask/SampleModeSwitch'
import type { UnderstandingView } from '@/components/ask/types'
import { UnderstandingCard } from '@/components/ask/UnderstandingCard'
import { FormMessage } from '@/components/forms/FormMessage'
import { cn } from '@/lib/utils'

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
    <div aria-busy={pending} className={cn('flex flex-col gap-4', pending && 'pointer-events-none opacity-60')}>
      {import.meta.env.DEV && variables.fromSamples && <SampleBadge />}
      <AskWarnings messages={response.warnings.map((warning) => warning.message)} />
      <OutcomeBody
        answer={answer}
        pending={pending}
        onChangeAssumption={onChangeAssumption}
        onAnswerClarifications={onAnswerClarifications}
      />
    </div>
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
    </>
  )
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
