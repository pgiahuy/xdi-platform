// SPDX-License-Identifier: Apache-2.0
/* components/ask/QuestionForm.tsx - ô hỏi ở đáy khung trò chuyện: gõ câu hỏi, bấm Hỏi. Ô chọn dataset nằm ở đầu trang
(DatasetPicker). Trang đặt ô này dính đáy cột chính, nên giữ thấp: nhãn "Câu hỏi" chỉ cho trình đọc màn hình (chữ gợi
ý trong ô đã nói ô để làm gì), nút Hỏi đứng cạnh ô chữ ở mọi cỡ màn.
Enter gửi, Shift+Enter xuống dòng, như ô chat quen thuộc. Bỏ qua Enter khi bộ gõ đang ghép chữ (isComposing): gõ tiếng
Việt bằng bộ gõ kiểu IBus, Enter lúc đó là chốt chữ, gửi luôn thì câu hỏi bị cắt giữa chừng. */

import type { FormEvent, KeyboardEvent } from 'react'
import { SendHorizontal } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'

type Props = {
  question: string
  onQuestionChange: (question: string) => void
  onSubmit: () => void
  pending: boolean
}

export function QuestionForm(props: Props) {
  const { question, onQuestionChange, onSubmit, pending } = props
  const canSubmit = !pending && question.trim() !== ''

  const handleSubmit = (event: FormEvent) => {
    event.preventDefault()
    if (canSubmit) onSubmit()
  }

  const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault()
      if (canSubmit) onSubmit()
    }
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-2">
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="ask-question" className="sr-only">
          Câu hỏi
        </Label>
        <div className="flex flex-row items-end gap-2">
          <Textarea
            id="ask-question"
            value={question}
            onChange={(event) => onQuestionChange(event.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Gõ câu hỏi về dữ liệu của dataset"
            aria-describedby="ask-question-hint"
            rows={2}
            className="max-h-48 min-h-16 min-w-0 flex-1"
          />
          <Button type="submit" size="lg" disabled={!canSubmit} className="px-4 sm:min-w-24">
            {/* Icon cạnh chữ chỉ để nhận ra nút gửi nhanh hơn; chữ đã đủ nghĩa nên trình đọc màn hình bỏ qua icon. */}
            <SendHorizontal aria-hidden />
            {pending ? 'Đang hỏi...' : 'Hỏi'}
          </Button>
        </div>
        <p id="ask-question-hint" className="hidden text-xs text-muted-foreground sm:block">
          <Kbd>Enter</Kbd> để hỏi, <Kbd>Shift+Enter</Kbd> để xuống dòng.
        </p>
      </div>
    </form>
  )
}

function Kbd({ children }: { children: string }) {
  return (
    <kbd className="rounded border border-b-2 border-input bg-background px-1 font-mono text-[11px] text-foreground">
      {children}
    </kbd>
  )
}
