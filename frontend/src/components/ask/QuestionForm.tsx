// SPDX-License-Identifier: Apache-2.0
/* components/ask/QuestionForm.tsx - ô hỏi: chọn dataset (kèm dòng báo hồ sơ chưa xong), gõ câu hỏi, bấm Hỏi; câu gợi ý
ngay dưới ô.
Enter gửi, Shift+Enter xuống dòng, như ô chat quen thuộc. Bỏ qua Enter khi bộ gõ đang ghép chữ (isComposing): gõ tiếng
Việt bằng bộ gõ kiểu IBus, Enter lúc đó là chốt chữ, gửi luôn thì câu hỏi bị cắt giữa chừng. */

import type { FormEvent, KeyboardEvent } from 'react'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import { cn } from '@/lib/utils'
import { ProfileNote } from './ProfileNote'
import { SuggestedQuestions } from './SuggestedQuestions'
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
  suggestions: string[]
  onSuggestion: (question: string) => void
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
    <form onSubmit={handleSubmit} className="flex flex-col gap-3">
      {datasets.length > 0 && (
        <div className="flex flex-col gap-1.5">
          <div className="flex flex-wrap items-center gap-2">
            <Label htmlFor="ask-dataset">Dataset</Label>
            <select
              id="ask-dataset"
              value={datasetId}
              onChange={(event) => onDatasetChange(event.target.value)}
              disabled={pending}
              className={cn(
                'h-9 max-w-full min-w-0 flex-1 rounded-lg border border-input bg-transparent px-2 text-sm outline-none',
                'sm:flex-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:opacity-50',
              )}
            >
              {datasets.map((dataset) => (
                <option key={dataset.id} value={dataset.id}>
                  {dataset.name}
                </option>
              ))}
            </select>
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
      <SuggestedQuestions questions={props.suggestions} onPick={props.onSuggestion} disabled={pending} />
    </form>
  )
}
