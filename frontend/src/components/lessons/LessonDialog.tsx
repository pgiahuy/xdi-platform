// SPDX-License-Identifier: Apache-2.0
/* components/lessons/LessonDialog.tsx - hộp viết một bài học (#228): một hai câu ngắn, áp cho một dataset hay cả
workspace. Mở từ nút "Viết bài học" đầu màn (trống) hay ở một dòng nhật ký lỗi (điền sẵn dataset, gắn dòng đó và hiện
câu hỏi, lỗi của dòng ngay trong hộp). Hộp mở đè lên chỗ đang xem, không như form nằm đầu trang mà người bấm ở cuối
nhật ký không thấy. Lưu là chờ duyệt; bài chỉ vào prompt khi admin bấm Duyệt và bật. Chỉ nhận dữ liệu qua props. */

import { useId, useState } from 'react'
import { FormMessage } from '@/components/forms/FormMessage'
import { NativeSelect } from '@/components/metrics/NativeSelect'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Textarea } from '@/components/ui/textarea'
import { CardField } from './CardField'
import type { LessonDraft, LessonSource } from './types'

type Props = {
  /** Bài đang soạn; undefined là hộp đóng. */
  initial: LessonDraft | undefined
  /** Dòng nhật ký đang viết bài học từ đó; null khi viết tay. */
  source: LessonSource | null
  datasets: { id: string; name: string }[]
  pending: boolean
  error: string | null
  onSubmit: (draft: LessonDraft) => void
  onClose: () => void
}

export function LessonDialog({ initial, onClose, ...formProps }: Props) {
  return (
    <Dialog open={initial !== undefined} onOpenChange={(open) => !open && onClose()}>
      {/* Nút đóng góc trên của shadcn có chữ tiếng Anh cho trình đọc màn hình; dùng nút Hủy ở chân hộp thay cho nó. */}
      <DialogContent showCloseButton={false} className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Viết bài học</DialogTitle>
          <DialogDescription>Lưu xong bài học chờ duyệt, bấm Duyệt và bật thì mô hình mới dùng.</DialogDescription>
        </DialogHeader>
        {/* Radix gỡ nội dung khi hộp đóng: mỗi lần mở là form đọc lại giá trị đầu, không còn chữ của lần trước. */}
        {initial && <LessonForm initial={initial} onClose={onClose} {...formProps} />}
      </DialogContent>
    </Dialog>
  )
}

// Cùng trần với backend (service/lessons/mod.rs).
const textMaxChars = 300

function LessonForm({
  initial,
  source,
  datasets,
  pending,
  error,
  onSubmit,
  onClose,
}: Omit<Props, 'initial'> & { initial: LessonDraft }) {
  const [draft, setDraft] = useState(initial)
  const textId = useId()
  const scopeId = useId()
  const options = [{ value: '', label: 'Cả workspace' }, ...datasets.map((d) => ({ value: d.id, label: d.name }))]
  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={(event) => {
        event.preventDefault()
        onSubmit(draft)
      }}
    >
      {source && (
        <dl className="flex flex-col gap-2 rounded-lg border bg-muted/50 px-3 py-2.5 text-sm">
          <CardField label="Câu hỏi">{source.question}</CardField>
          <CardField label={source.stepLabel}>
            <span className="text-muted-foreground">{source.error}</span>
          </CardField>
        </dl>
      )}
      <div className="flex flex-col gap-1.5">
        <label htmlFor={textId} className="text-sm font-medium">
          Bài học
        </label>
        <Textarea
          id={textId}
          value={draft.text}
          maxLength={textMaxChars}
          rows={3}
          placeholder="Ví dụ: Cột studied_credits là số tín chỉ mỗi sinh viên, không cộng qua bảng bài nộp."
          onChange={(e) => setDraft({ ...draft, text: e.target.value })}
          disabled={pending}
        />
        <span className="self-end text-xs text-muted-foreground tabular-nums">
          {draft.text.length}/{textMaxChars}
        </span>
      </div>
      <div className="flex flex-col gap-1.5">
        <label htmlFor={scopeId} className="text-sm font-medium">
          Áp cho
        </label>
        <NativeSelect
          id={scopeId}
          options={options}
          value={draft.datasetId}
          onChange={(e) => setDraft({ ...draft, datasetId: e.target.value })}
          disabled={pending}
        />
      </div>
      {error && <FormMessage kind="error">{error}</FormMessage>}
      <DialogFooter>
        <Button type="button" variant="outline" onClick={onClose} disabled={pending}>
          Hủy
        </Button>
        <Button type="submit" disabled={pending || draft.text.trim() === ''}>
          Lưu
        </Button>
      </DialogFooter>
    </form>
  )
}
