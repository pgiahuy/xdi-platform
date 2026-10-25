// SPDX-License-Identifier: Apache-2.0
/* components/ask/QuestionForm.tsx - ô hỏi: chọn dataset (kèm dòng báo hồ sơ chưa xong), gõ câu hỏi, bấm Hỏi. Câu gợi ý
nằm ở thẻ riêng ngay dưới (SuggestedQuestions.tsx), trang đặt cả hai.
Chọn dataset dùng Select của shadcn (Radix) thay cho select của trình duyệt, để giống các ô khác trên trang; Radix vẫn
cho chọn bằng bàn phím và trình đọc màn hình đọc được như một combobox.
Enter gửi, Shift+Enter xuống dòng, như ô chat quen thuộc. Bỏ qua Enter khi bộ gõ đang ghép chữ (isComposing): gõ tiếng
Việt bằng bộ gõ kiểu IBus, Enter lúc đó là chốt chữ, gửi luôn thì câu hỏi bị cắt giữa chừng. */

import type { FormEvent, KeyboardEvent } from 'react'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Textarea } from '@/components/ui/textarea'
import { ProfileNote } from './ProfileNote'
import type { DatasetOption, ProfileProgress } from './types'

type Props = {
  datasets: DatasetOption[]
  datasetId: string
  onDatasetChange: (id: string) => void
  profile: ProfileProgress
  question: string
  onQuestionChange: (question: string) => void
  onSubmit: () => void
  pending: boolean
}

export function QuestionForm(props: Props) {
  const { datasets, datasetId, onDatasetChange, question, onQuestionChange, onSubmit, pending } = props
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
    <form onSubmit={handleSubmit} className="flex flex-col gap-4">
      {datasets.length > 0 && (
        <div className="flex flex-col gap-1.5">
          <div className="flex flex-wrap items-center gap-2">
            <Label htmlFor="ask-dataset">Dataset</Label>
            <Select value={datasetId} onValueChange={onDatasetChange} disabled={pending}>
              <SelectTrigger id="ask-dataset" className="max-w-full min-w-40 flex-1 sm:flex-none">
                <SelectValue placeholder="Chọn dataset" />
              </SelectTrigger>
              <SelectContent>
                {datasets.map((dataset) => (
                  <SelectItem key={dataset.id} value={dataset.id}>
                    {dataset.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <ProfileNote progress={props.profile} />
        </div>
      )}
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="ask-question">Câu hỏi</Label>
        <div className="flex flex-col gap-2 sm:flex-row sm:items-start">
          <Textarea
            id="ask-question"
            value={question}
            onChange={(event) => onQuestionChange(event.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Gõ câu hỏi về dữ liệu của dataset"
            aria-describedby="ask-question-hint"
            rows={2}
            className="min-h-16 flex-1"
          />
          <Button type="submit" size="lg" disabled={!canSubmit} className="sm:w-24">
            {pending ? 'Đang hỏi...' : 'Hỏi'}
          </Button>
        </div>
        <p id="ask-question-hint" className="text-xs text-muted-foreground">
          Enter để hỏi, Shift+Enter để xuống dòng.
        </p>
      </div>
    </form>
  )
}
