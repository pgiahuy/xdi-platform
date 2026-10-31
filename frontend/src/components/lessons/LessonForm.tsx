// SPDX-License-Identifier: Apache-2.0
/* components/lessons/LessonForm.tsx - form viết một bài học (#228): một hai câu ngắn, áp cho một dataset hay cả
workspace. Mở từ nút "Viết bài học" (trống) hay từ một dòng nhật ký lỗi (điền sẵn dataset và gắn dòng đó). Lưu là bản
nháp; bài chỉ vào prompt khi admin bật ở danh sách. Chỉ nhận dữ liệu qua props. */

import { useId, useState } from 'react'
import { FormMessage } from '@/components/forms/FormMessage'
import { NativeSelect } from '@/components/metrics/NativeSelect'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import type { LessonDraft } from './types'

type Props = {
  initial: LessonDraft
  /** Câu hỏi của dòng nhật ký đang viết bài học từ đó; null khi viết tay. */
  fromQuestion: string | null
  datasets: { id: string; name: string }[]
  pending: boolean
  error: string | null
  onSubmit: (draft: LessonDraft) => void
  onCancel: () => void
}

// Cùng trần với backend (service/lessons/mod.rs).
const textMaxChars = 300

export function LessonForm({ initial, fromQuestion, datasets, pending, error, onSubmit, onCancel }: Props) {
  const [draft, setDraft] = useState(initial)
  const textId = useId()
  const options = [{ value: '', label: 'Cả workspace' }, ...datasets.map((d) => ({ value: d.id, label: d.name }))]
  return (
    <form
      className="flex flex-col gap-3 rounded-xl border bg-card p-4"
      onSubmit={(event) => {
        event.preventDefault()
        onSubmit(draft)
      }}
    >
      {fromQuestion && <p className="text-sm text-muted-foreground">Từ lỗi của câu hỏi: {fromQuestion}</p>}
      <div className="flex flex-col gap-1">
        <label htmlFor={textId} className="text-sm font-medium">
          Bài học
        </label>
        <Textarea
          id={textId}
          value={draft.text}
          maxLength={textMaxChars}
          placeholder="Ví dụ: Cột studied_credits là số tín chỉ mỗi sinh viên, không cộng qua bảng bài nộp."
          onChange={(e) => setDraft({ ...draft, text: e.target.value })}
          disabled={pending}
        />
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-sm">Áp cho</span>
        <NativeSelect
          aria-label="Áp cho"
          options={options}
          value={draft.datasetId}
          onChange={(e) => setDraft({ ...draft, datasetId: e.target.value })}
          disabled={pending}
        />
      </div>
      {error && <FormMessage kind="error">{error}</FormMessage>}
      <div className="flex gap-2">
        <Button type="submit" size="sm" disabled={pending || draft.text.trim() === ''}>
          Lưu bản nháp
        </Button>
        <Button type="button" variant="outline" size="sm" onClick={onCancel} disabled={pending}>
          Hủy
        </Button>
      </div>
    </form>
  )
}
