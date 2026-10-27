// SPDX-License-Identifier: Apache-2.0
/* pages/AskConversation.tsx - các lượt của cuộc trò chuyện đang mở trên trang hỏi (#164), xếp dọc theo thứ tự hỏi. Mỗi
lượt là một AskOutcome riêng, giữ thẻ "Đã hiểu là", bảng và câu hỏi lại của chính nó. Bấm hay đưa focus vào một lượt
thì pane bằng chứng hiện của lượt đó (AskPage giữ lượt đang chọn); lượt đang chọn có viền màu chính để biết pane đang
nói về lượt nào. */

import type { ChosenOption } from '@/api/ask'
import type { DrillTarget } from '@/components/ask/evidenceTypes'
import { cn } from '@/lib/utils'
import { AskOutcome, type Answer } from './AskOutcome'

type Props = {
  turns: Answer[]
  selected: number
  onSelect: (index: number) => void
  /** Lượt đang được hỏi lại (đổi giả định, trả lời câu hỏi lại); null khi không có. */
  pendingTurn: number | null
  onCellDrill: (index: number, target: DrillTarget) => void
  onAskAgain: (index: number, picks: ChosenOption[], answered?: string[]) => void
}

export function AskConversation({ turns, selected, onSelect, pendingTurn, onCellDrill, onAskAgain }: Props) {
  // Chỉ có một lượt thì không cần chỉ ra lượt nào đang chọn.
  const marked = turns.length > 1
  return (
    <div className="flex flex-col gap-4">
      {turns.map((turn, index) => (
        <div
          // Các lượt chỉ thêm vào cuối hoặc thay lượt cuối, nên vị trí là khóa ổn định của lượt.
          key={index}
          onClickCapture={() => onSelect(index)}
          onFocusCapture={() => onSelect(index)}
          className={cn('rounded-xl', marked && index === selected && 'ring-2 ring-primary/70')}
        >
          <AskOutcome
            answer={turn}
            pending={pendingTurn === index}
            onCellDrill={(target) => onCellDrill(index, target)}
            onChangeAssumption={(choiceId, optionId) =>
              onAskAgain(index, [{ choice_id: choiceId, option_id: optionId }])
            }
            onAnswerClarifications={(picks) =>
              onAskAgain(
                index,
                picks.map((pick) => ({ choice_id: pick.choiceId, option_id: pick.optionId })),
                picks.map((pick) => pick.choiceId),
              )
            }
          />
        </div>
      ))}
    </div>
  )
}
