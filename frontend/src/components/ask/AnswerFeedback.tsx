// SPDX-License-Identifier: Apache-2.0
/* components/ask/AnswerFeedback.tsx - hai nút "Tốt" và "Chưa đạt" dưới một câu trả lời (#212). Bấm "Tốt" ghi ngay;
bấm "Chưa đạt" mở một form nhỏ chọn lý do và ghi chú (không bắt buộc) rồi mới gửi. Bấm lại nút đang chọn thì bỏ đánh
giá. Câu admin đã duyệt làm câu mẫu có nhãn "Đã duyệt làm câu mẫu". Chỉ nhận dữ liệu qua props; trang nối với API
(pages/AnswerFeedbackBar.tsx). */

import { useId, useState } from 'react'
import { BadgeCheck, ThumbsDown, ThumbsUp } from 'lucide-react'
import { FormMessage } from '@/components/forms/FormMessage'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'

export type FeedbackReasonId = 'misunderstood' | 'wrong_numbers' | 'unclear_explanation' | 'other'

export type FeedbackView = {
  rating: 'good' | 'bad'
  reason: FeedbackReasonId | null
  approved: boolean
}

type Props = {
  feedback: FeedbackView | null
  pending: boolean
  /** Lời báo khi lần gửi gần nhất lỗi; null khi không lỗi. */
  error: string | null
  onRateGood: () => void
  onRateBad: (reason: FeedbackReasonId, note: string) => void
  onClear: () => void
}

const reasons: { id: FeedbackReasonId; label: string }[] = [
  { id: 'misunderstood', label: 'Hiểu sai câu hỏi' },
  { id: 'wrong_numbers', label: 'Số sai' },
  { id: 'unclear_explanation', label: 'Giải thích khó hiểu' },
  { id: 'other', label: 'Khác' },
]

// Cùng trần với backend (service/feedback/mod.rs); textarea chặn gõ quá, backend vẫn kiểm.
const noteMaxChars = 500

export function AnswerFeedback({ feedback, pending, error, onRateGood, onRateBad, onClear }: Props) {
  const [formOpen, setFormOpen] = useState(false)
  const rating = feedback?.rating ?? null
  const reasonLabel = reasons.find((r) => r.id === feedback?.reason)?.label
  const pressGood = () => (rating === 'good' ? onClear() : onRateGood())
  const pressBad = () => {
    if (rating === 'bad') onClear()
    else setFormOpen(true)
  }
  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-sm text-muted-foreground">Câu trả lời này:</span>
        <Button variant={rating === 'good' ? 'default' : 'outline'} size="sm" aria-pressed={rating === 'good'} onClick={pressGood} disabled={pending}>
          <ThumbsUp aria-hidden />
          Tốt
        </Button>
        <Button variant={rating === 'bad' ? 'default' : 'outline'} size="sm" aria-pressed={rating === 'bad'} onClick={pressBad} disabled={pending || formOpen}>
          <ThumbsDown aria-hidden />
          Chưa đạt
        </Button>
        {rating === 'bad' && reasonLabel && <span className="text-sm text-muted-foreground">Lý do: {reasonLabel}</span>}
        {feedback?.approved && (
          <span className="inline-flex items-center gap-1 rounded-md bg-muted px-2 py-0.5 text-xs font-medium">
            <BadgeCheck aria-hidden className="size-3.5" />
            Đã duyệt làm câu mẫu
          </span>
        )}
      </div>
      {formOpen && (
        <BadAnswerForm
          disabled={pending}
          onCancel={() => setFormOpen(false)}
          onSubmit={(reason, note) => {
            onRateBad(reason, note)
            setFormOpen(false)
          }}
        />
      )}
      {error && <FormMessage kind="error">{error}</FormMessage>}
    </div>
  )
}

function BadAnswerForm({
  disabled,
  onCancel,
  onSubmit,
}: {
  disabled: boolean
  onCancel: () => void
  onSubmit: (reason: FeedbackReasonId, note: string) => void
}) {
  const [reason, setReason] = useState<FeedbackReasonId>('misunderstood')
  const [note, setNote] = useState('')
  const noteId = useId()
  return (
    <form
      className="flex flex-col gap-3 rounded-lg border p-3"
      onSubmit={(event) => {
        event.preventDefault()
        onSubmit(reason, note)
      }}
    >
      <fieldset className="flex flex-col gap-2" disabled={disabled}>
        <legend className="mb-1 text-sm font-medium">Vì sao chưa đạt?</legend>
        {reasons.map((option) => (
          <label key={option.id} className="flex items-center gap-2 text-sm">
            <input
              type="radio"
              name="reason"
              value={option.id}
              checked={reason === option.id}
              onChange={() => setReason(option.id)}
              className="size-4 accent-primary"
            />
            {option.label}
          </label>
        ))}
      </fieldset>
      <div className="flex flex-col gap-1">
        <label htmlFor={noteId} className="text-sm">
          Ghi chú (không bắt buộc)
        </label>
        <Textarea id={noteId} value={note} maxLength={noteMaxChars} onChange={(e) => setNote(e.target.value)} disabled={disabled} />
      </div>
      <div className="flex gap-2">
        <Button type="submit" size="sm" disabled={disabled}>
          Gửi đánh giá
        </Button>
        <Button type="button" variant="outline" size="sm" onClick={onCancel} disabled={disabled}>
          Hủy
        </Button>
      </div>
    </form>
  )
}
