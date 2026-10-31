// SPDX-License-Identifier: Apache-2.0
/* components/lessons/LessonList.tsx - danh sách bài học của workspace (#228): chữ bài học, áp cho dataset nào, trạng
thái (nháp, đang dùng, đã tắt) và người duyệt. Bật là duyệt: chỉ bài đang dùng vào prompt. Chỉ nhận dữ liệu qua props. */

import { Trash2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import type { LessonItem } from './types'

type Props = {
  items: LessonItem[]
  /** id của bài đang gửi thay đổi; null khi không có. */
  pendingId: string | null
  onToggle: (id: string, enabled: boolean) => void
  onDelete: (id: string) => void
}

const dateFormat = new Intl.DateTimeFormat('vi-VN', { dateStyle: 'short', timeStyle: 'short' })

function statusText(item: LessonItem): string {
  if (item.enabled) {
    return `Đang dùng, duyệt bởi ${item.approvedBy ?? 'tài khoản đã xóa'} lúc ${dateFormat.format(new Date(item.approvedAt ?? ''))}`
  }
  return item.approvedAt ? 'Đã tắt' : 'Bản nháp, chưa duyệt'
}

export function LessonList({ items, pendingId, onToggle, onDelete }: Props) {
  return (
    <ul className="flex flex-col gap-3">
      {items.map((item) => (
        <li key={item.id} className="flex flex-wrap items-start justify-between gap-3 rounded-xl border bg-card p-4">
          <div className="flex min-w-0 flex-col gap-1">
            <p className="wrap-break-word">{item.text}</p>
            <p className="text-xs text-muted-foreground">
              {[item.datasetName ?? 'Cả workspace', item.fromError ? 'viết từ nhật ký lỗi' : null, statusText(item)]
                .filter(Boolean)
                .join(', ')}
            </p>
          </div>
          <div className="flex gap-2">
            <Button
              size="sm"
              variant={item.enabled ? 'outline' : 'default'}
              disabled={pendingId === item.id}
              onClick={() => onToggle(item.id, !item.enabled)}
            >
              {item.enabled ? 'Tắt' : 'Duyệt và bật'}
            </Button>
            <Button size="sm" variant="ghost" aria-label="Xóa bài học" disabled={pendingId === item.id} onClick={() => onDelete(item.id)}>
              <Trash2 aria-hidden />
            </Button>
          </div>
        </li>
      ))}
    </ul>
  )
}
