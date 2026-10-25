// SPDX-License-Identifier: Apache-2.0
/* components/ask/SuggestedQuestions.tsx - câu hỏi gợi ý dưới ô hỏi; bấm một câu là hỏi luôn câu đó.
Danh sách rỗng thì không hiện gì: câu gợi ý do lớp hồ sơ ngữ nghĩa viết sau khi nạp (ARCHITECTURE mục 6), chưa xong
hoặc mô hình không đề xuất câu nào thì dataset không có câu gợi ý. */

import { Button } from '@/components/ui/button'

type Props = {
  questions: string[]
  onPick: (question: string) => void
  disabled: boolean
}

export function SuggestedQuestions({ questions, onPick, disabled }: Props) {
  if (questions.length === 0) return null
  return (
    <div className="flex flex-col gap-1.5">
      <p id="ask-suggestions" className="text-xs text-muted-foreground">
        Câu gợi ý
      </p>
      <ul aria-labelledby="ask-suggestions" className="flex flex-wrap gap-2">
        {questions.map((question) => (
          <li key={question} className="max-w-full">
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={disabled}
              onClick={() => onPick(question)}
              className="h-auto max-w-full py-1 text-left whitespace-normal"
            >
              {question}
            </Button>
          </li>
        ))}
      </ul>
    </div>
  )
}
