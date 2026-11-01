// SPDX-License-Identifier: Apache-2.0
/* components/lessons/LessonList.tsx - danh sách bài học của workspace (#228). Mỗi thẻ ghi tên từng phần: trạng thái
(chờ duyệt, đang dùng, đã tắt), chữ bài học, áp cho dataset nào, viết từ đâu, ai duyệt. Bật là duyệt: chỉ bài đang dùng
vào prompt. Bài chờ duyệt có viền màu nhấn, vì đó là việc admin còn phải làm. Chỉ nhận dữ liệu qua props. */

import { Trash2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { CardField } from './CardField'
import type { LessonItem } from './types'

type Props = {
  items: LessonItem[]
  /** id của bài đang gửi thay đổi; null khi không có. */
  pendingId: string | null
  onToggle: (id: string, enabled: boolean) => void
  onDelete: (id: string) => void
}

const dateFormat = new Intl.DateTimeFormat('vi-VN', { dateStyle: 'short', timeStyle: 'short' })

type Status = 'enabled' | 'pending' | 'off'

function statusOf(item: LessonItem): Status {
  if (item.enabled) return 'enabled'
  return item.approvedAt ? 'off' : 'pending'
}

const statuses: Record<Status, { label: string; badge: string; dot: string }> = {
  enabled: { label: 'Đang dùng', badge: 'border-label-border bg-label-soft text-label', dot: 'bg-label' },
  pending: { label: 'Chờ duyệt', badge: 'border-highlight-border bg-highlight-soft text-highlight-text', dot: 'bg-highlight' },
  off: { label: 'Đã tắt', badge: 'bg-muted text-muted-foreground', dot: 'bg-muted-foreground' },
}

// Nhật ký chỉ tải các dòng mới nhất: dòng cũ trôi ra thì không còn câu hỏi, chỉ ghi là viết từ nhật ký.
function originText(item: LessonItem): string {
  if (item.fromQuestion) return `Lỗi của câu "${item.fromQuestion}"`
  return item.fromError ? 'Nhật ký lỗi' : 'Viết tay'
}

export function LessonList({ items, pendingId, onToggle, onDelete }: Props) {
  return (
    <ul className="flex flex-col gap-3">
      {items.map((item) => {
        const status = statusOf(item)
        const look = statuses[status]
        return (
          <li
            key={item.id}
            className={cn('flex flex-col gap-3 rounded-xl border bg-card p-4', status === 'pending' && 'border-highlight-border')}
          >
            <div className="flex flex-wrap items-center justify-between gap-2">
              <span
                className={cn(
                  'inline-flex items-center gap-1.5 rounded-full border px-2 py-0.5 text-[11px] font-medium whitespace-nowrap',
                  look.badge,
                )}
              >
                <span aria-hidden className={cn('size-1.5 rounded-full', look.dot)} />
                {look.label}
              </span>
              <div className="flex gap-2">
                <Button
                  size="sm"
                  variant={item.enabled ? 'outline' : 'default'}
                  disabled={pendingId === item.id}
                  onClick={() => onToggle(item.id, !item.enabled)}
                >
                  {item.enabled ? 'Tắt' : 'Duyệt và bật'}
                </Button>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  aria-label="Xóa bài học"
                  disabled={pendingId === item.id}
                  onClick={() => onDelete(item.id)}
                >
                  <Trash2 aria-hidden />
                </Button>
              </div>
            </div>
            <dl className="flex flex-col gap-3">
              <CardField label="Bài học">
                <span className="text-[15px] leading-relaxed">{item.text}</span>
              </CardField>
              <div className="grid gap-3 text-sm sm:grid-cols-3">
                <CardField label="Áp cho">{item.datasetName ?? 'Cả workspace'}</CardField>
                <CardField label="Viết từ">{originText(item)}</CardField>
                {item.approvedAt && (
                  <CardField label="Duyệt">
                    {`${item.approvedBy ?? 'Tài khoản đã xóa'}, ${dateFormat.format(new Date(item.approvedAt))}`}
                  </CardField>
                )}
              </div>
            </dl>
          </li>
        )
      })}
    </ul>
  )
}
