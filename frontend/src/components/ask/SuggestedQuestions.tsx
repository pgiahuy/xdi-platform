// SPDX-License-Identifier: Apache-2.0
/* components/ask/SuggestedQuestions.tsx - danh sách câu hỏi gợi ý; bấm một câu là hỏi luôn câu đó. Trang đặt danh sách
trong thẻ "Câu gợi ý" (AskSection), nên ở đây không có tiêu đề riêng.
Danh sách rỗng thì không hiện gì: câu gợi ý do lớp hồ sơ ngữ nghĩa viết sau khi nạp (ARCHITECTURE mục 6), chưa xong
hoặc mô hình không đề xuất câu nào thì dataset không có câu gợi ý. */

import { Lightbulb } from 'lucide-react'
import { Button } from '@/components/ui/button'

type Props = {
  questions: string[]
  onPick: (question: string) => void
  disabled: boolean
}

export function SuggestedQuestions({ questions, onPick, disabled }: Props) {
  if (questions.length === 0) return null
  return (
    <ul className="flex flex-wrap gap-2">
      {questions.map((question) => (
        <li key={question} className="max-w-full">
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={disabled}
            onClick={() => onPick(question)}
            className="h-auto max-w-full gap-2 rounded-full px-3 py-1.5 text-left font-normal whitespace-normal text-foreground/80 hover:border-label hover:bg-label-soft hover:text-label"
          >
            <Lightbulb aria-hidden className="text-label" />
            {question}
          </Button>
        </li>
      ))}
    </ul>
  )
}
