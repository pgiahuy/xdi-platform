// SPDX-License-Identifier: Apache-2.0
/* components/lessons/LessonStats.tsx - hàng số tóm tắt đầu màn Bài học (#228): bài đang dùng, bài chờ duyệt, lỗi gần đây
và lỗi chưa có bài học. Hai số "chờ duyệt" và "chưa có bài học" là việc admin còn phải làm, nên tô màu khi lớn hơn 0.
Chỉ nhận dữ liệu qua props. */

import { cn } from '@/lib/utils'

type Props = {
  enabled: number
  pending: number
  errors: number
  errorsWithoutLesson: number
}

export function LessonStats({ enabled, pending, errors, errorsWithoutLesson }: Props) {
  return (
    <dl className="grid grid-cols-2 gap-3 lg:grid-cols-4">
      <Stat label="Bài học đang dùng" value={enabled} />
      <Stat label="Chờ duyệt" value={pending} todo />
      <Stat label="Lỗi gần đây" value={errors} />
      <Stat label="Lỗi chưa có bài học" value={errorsWithoutLesson} todo />
    </dl>
  )
}

function Stat({ label, value, todo = false }: { label: string; value: number; todo?: boolean }) {
  const active = todo && value > 0
  return (
    <div
      className={cn(
        'flex flex-col gap-1 rounded-xl border bg-card px-4 py-3',
        active && 'border-highlight-border bg-highlight-soft',
      )}
    >
      <dt className={cn('text-xs font-medium text-muted-foreground', active && 'text-highlight-text')}>{label}</dt>
      <dd className={cn('text-2xl font-semibold tracking-tight', active && 'text-highlight-text')}>{value}</dd>
    </div>
  )
}
