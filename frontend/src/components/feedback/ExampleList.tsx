// SPDX-License-Identifier: Apache-2.0
/* components/feedback/ExampleList.tsx - danh sách câu trả lời được đánh giá "Tốt" để admin duyệt làm câu mẫu (#212).
Mỗi thẻ có câu hỏi, dataset, SQL mô hình viết, dòng tóm tắt "Đã hiểu là", số dòng kết quả lúc hỏi (kết quả không lưu),
ai đánh giá và ai đã duyệt. Bấm "Duyệt làm câu mẫu" thì câu vào prompt làm ví dụ ở các câu hỏi sau có từ chung. Chỉ
nhận dữ liệu qua props (pages/ExamplesPage.tsx). */

import { BadgeCheck } from 'lucide-react'
import { SqlCode } from '@/components/ask/SqlCode'
import { Button } from '@/components/ui/button'

export type ExampleItem = {
  queryId: string
  question: string
  datasetName: string | null
  sql: string | null
  understood: string | null
  rowCount: number | null
  why: boolean
  ratedBy: string | null
  ratedAt: string
  approvedBy: string | null
  approvedAt: string | null
}

type Props = {
  items: ExampleItem[]
  /** queryId của thẻ đang gửi duyệt hay bỏ duyệt; null khi không có. */
  pendingId: string | null
  onApprove: (queryId: string) => void
  onWithdraw: (queryId: string) => void
}

const dateFormat = new Intl.DateTimeFormat('vi-VN', { dateStyle: 'short', timeStyle: 'short' })

export function ExampleList({ items, pendingId, onApprove, onWithdraw }: Props) {
  return (
    <ul className="flex flex-col gap-4">
      {items.map((item) => (
        <li key={item.queryId} className="flex flex-col gap-3 rounded-xl border bg-card p-4">
          <div className="flex flex-wrap items-start justify-between gap-2">
            <div className="flex min-w-0 flex-col gap-0.5">
              <p className="font-medium wrap-break-word">{item.question}</p>
              <p className="text-xs text-muted-foreground">
                {[
                  item.datasetName ?? 'Dataset đã xóa',
                  item.why ? 'câu hỏi tìm nguyên nhân' : null,
                  item.rowCount === null ? null : `${item.rowCount} dòng kết quả`,
                  `đánh giá Tốt bởi ${item.ratedBy ?? 'tài khoản đã xóa'}, ${dateFormat.format(new Date(item.ratedAt))}`,
                ]
                  .filter(Boolean)
                  .join(', ')}
              </p>
            </div>
            {item.approvedAt ? (
              <Button variant="outline" size="sm" disabled={pendingId === item.queryId} onClick={() => onWithdraw(item.queryId)}>
                Bỏ duyệt
              </Button>
            ) : (
              <Button size="sm" disabled={pendingId === item.queryId || !item.sql} onClick={() => onApprove(item.queryId)}>
                Duyệt làm câu mẫu
              </Button>
            )}
          </div>
          {item.understood && (
            <p className="text-sm">
              <span className="font-semibold text-label">Đã hiểu là: </span>
              {item.understood}
            </p>
          )}
          {item.sql && <SqlCode sql={item.sql} />}
          {item.approvedAt && (
            <p className="inline-flex items-center gap-1 text-sm">
              <BadgeCheck aria-hidden className="size-4" />
              Đã duyệt bởi {item.approvedBy ?? 'tài khoản đã xóa'}, {dateFormat.format(new Date(item.approvedAt))}
            </p>
          )}
        </li>
      ))}
    </ul>
  )
}
