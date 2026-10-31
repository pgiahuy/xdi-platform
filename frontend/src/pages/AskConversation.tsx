// SPDX-License-Identifier: Apache-2.0
/* pages/AskConversation.tsx - khung trò chuyện của trang hỏi (#164): các lượt xếp dọc theo thứ tự hỏi, mỗi lượt là
bong bóng câu hỏi và khối trả lời (ChatTurn). Thứ tự: các lượt đã lưu của cuộc vừa mở lại (chỉ có câu hỏi và tóm tắt,
kèm nút Hỏi lại), rồi các lượt hỏi trong phiên này (AskOutcome đủ thẻ "Đã hiểu là", bảng, câu hỏi lại), rồi lượt đang
chờ câu trả lời nếu đang hỏi câu mới. Lượt đã ghi (có query_id) có hai nút đánh giá Tốt, Chưa đạt (#212).
Bấm hay đưa focus vào một lượt của phiên này thì pane bằng chứng hiện của lượt đó (AskPage giữ lượt đang chọn).
Số lượt hiện tăng (câu mới đang chờ, mở lại một cuộc) thì cuộn tới đầu lượt cuối: câu trả lời dài thì người đọc thấy
phần đầu trước. Câu trả lời về thay lượt đang chờ ở đúng chỗ, số lượt không đổi, nên không cuộn lần nữa. */

import { useEffect, useRef } from 'react'
import type { ChosenOption } from '@/api/ask'
import type { TurnItem } from '@/api/conversations'
import { ChatTurn } from '@/components/ask/ChatTurn'
import type { DrillTarget } from '@/components/ask/evidenceTypes'
import { SavedTurnBody } from '@/components/ask/SavedTurnBody'
import { AnswerFeedbackBar } from './AnswerFeedbackBar'
import { AskOutcome, type Answer } from './AskOutcome'
import { buildUnderstoodLines } from './savedTurnView'

type Props = {
  /** Slug của workspace, cho nút đánh giá câu trả lời. */
  ws: string
  savedTurns: TurnItem[]
  turns: Answer[]
  selected: number
  onSelect: (index: number) => void
  /** Lượt đang được hỏi lại (đổi giả định, trả lời câu hỏi lại); null khi không có. */
  pendingTurn: number | null
  /** Câu mới đang chờ câu trả lời; null khi không có. */
  pendingQuestion: string | null
  onCellDrill: (index: number, target: DrillTarget) => void
  onAskAgain: (index: number, picks: ChosenOption[], answered?: string[]) => void
  onShowEvidence: (index: number) => void
  /** Hỏi lại câu của một lượt đã lưu, như gõ lại câu đó vào ô hỏi. */
  onAskSaved: (question: string) => void
  pending: boolean
}

export function AskConversation(props: Props) {
  const { savedTurns, turns, selected, onSelect, pendingTurn, pendingQuestion, pending } = props
  const lastRef = useRef<HTMLElement>(null)
  const count = savedTurns.length + turns.length + (pendingQuestion === null ? 0 : 1)
  const shown = useRef(0)
  useEffect(() => {
    if (count > shown.current) {
      const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches
      lastRef.current?.scrollIntoView({ block: 'start', behavior: reduce ? 'auto' : 'smooth' })
    }
    shown.current = count
  }, [count])

  // Chỉ có một lượt thì không cần chỉ ra lượt nào đang chọn.
  const marked = turns.length > 1
  const isLast = (position: number) => position === count - 1
  return (
    // Khoảng cách giữa hai lượt lớn hơn hẳn khoảng trong một lượt, để mắt nhóm câu hỏi với đúng câu trả lời của nó.
    <div className="flex flex-col gap-10">
      {savedTurns.map((turn, index) => (
        <ChatTurn key={turn.query_id} question={turn.question} ref={isLast(index) ? lastRef : undefined}>
          <SavedTurnBody
            status={turn.status}
            understood={buildUnderstoodLines(turn.understood)}
            onAskAgain={() => props.onAskSaved(turn.question)}
            disabled={pending}
          />
          <AnswerFeedbackBar ws={props.ws} queryId={turn.query_id} initial={turn.feedback} />
        </ChatTurn>
      ))}
      {turns.map((turn, index) => (
        <div
          // Các lượt chỉ thêm vào cuối hoặc thay lượt cuối, nên vị trí là khóa ổn định của lượt.
          key={index}
          onClickCapture={() => onSelect(index)}
          onFocusCapture={() => onSelect(index)}
          className="min-w-0"
        >
          <ChatTurn
            question={turn.variables.question}
            selected={marked && index === selected}
            onShowEvidence={() => props.onShowEvidence(index)}
            ref={isLast(savedTurns.length + index) ? lastRef : undefined}
          >
            <AskOutcome
              answer={turn}
              pending={pendingTurn === index}
              latest={index === turns.length - 1}
              onCellDrill={(target) => props.onCellDrill(index, target)}
              onChangeAssumption={(choiceId, optionId) =>
                props.onAskAgain(index, [{ choice_id: choiceId, option_id: optionId }])
              }
              onAnswerClarifications={(picks) =>
                props.onAskAgain(
                  index,
                  picks.map((pick) => ({ choice_id: pick.choiceId, option_id: pick.optionId })),
                  picks.map((pick) => pick.choiceId),
                )
              }
            />
            {/* Câu cần chọn cách hiểu chưa được ghi (không có query_id) nên chưa có gì để đánh giá. key theo query_id:
                hỏi lại lượt này ra câu mới thì nút đánh giá bắt đầu lại từ chưa đánh giá. */}
            {turn.response.query_id && (
              <AnswerFeedbackBar key={turn.response.query_id} ws={props.ws} queryId={turn.response.query_id} initial={null} />
            )}
          </ChatTurn>
        </div>
      ))}
      {pendingQuestion !== null && (
        <ChatTurn question={pendingQuestion} ref={lastRef} compact>
          {/* Ba chấm nhảy lệch nhịp như ô chat quen thuộc; chữ chỉ cho trình đọc màn hình. */}
          <p className="flex h-5 items-center gap-1.5">
            <span className="sr-only">Đang trả lời...</span>
            {['[animation-delay:-0.3s]', '[animation-delay:-0.15s]', ''].map((delay) => (
              <span
                key={delay}
                aria-hidden
                className={`size-2 animate-bounce rounded-full bg-muted-foreground/70 motion-reduce:animate-none ${delay}`}
              />
            ))}
          </p>
        </ChatTurn>
      )}
    </div>
  )
}
