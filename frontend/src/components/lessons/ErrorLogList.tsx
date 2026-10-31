// SPDX-License-Identifier: Apache-2.0
/* components/lessons/ErrorLogList.tsx - nhật ký lỗi (#228): mỗi dòng là một lần mô hình sai (guard từ chối, SQL lỗi,
cộng lặp, form sai, lời bị bộ kiểm chặn) hay một câu bị đánh giá Chưa đạt, kèm câu hỏi và câu mô hình đã trả. Nút
"Viết bài học" mở form bài học gắn với dòng đó. Chỉ nhận dữ liệu qua props. */

import { SqlCode } from '@/components/ask/SqlCode'
import { Button } from '@/components/ui/button'
import type { ErrorItem } from './types'

type Props = {
  items: ErrorItem[]
  onWriteLesson: (item: ErrorItem) => void
  disabled: boolean
}

const dateFormat = new Intl.DateTimeFormat('vi-VN', { dateStyle: 'short', timeStyle: 'short' })

// Câu mô hình trả là SQL thì hiện như khung SQL; form JSON hay lời viết thì để chữ thường.
const looksLikeSql = (text: string) => /^\s*(select|with)\b/i.test(text)

export function ErrorLogList({ items, onWriteLesson, disabled }: Props) {
  return (
    <ul className="flex flex-col gap-3">
      {items.map((item) => (
        <li key={item.id} className="flex flex-col gap-2 rounded-xl border bg-card p-4">
          <div className="flex flex-wrap items-start justify-between gap-2">
            <div className="flex min-w-0 flex-col gap-0.5">
              <p className="font-medium wrap-break-word">{item.question}</p>
              <p className="text-xs text-muted-foreground">
                {[item.stepLabel, item.datasetName ?? 'Dataset đã xóa', dateFormat.format(new Date(item.createdAt))].join(', ')}
              </p>
            </div>
            <Button size="sm" variant="outline" onClick={() => onWriteLesson(item)} disabled={disabled}>
              Viết bài học
            </Button>
          </div>
          <p className="text-sm wrap-break-word">{item.error}</p>
          {item.modelOutput &&
            (looksLikeSql(item.modelOutput) ? (
              <SqlCode sql={item.modelOutput} />
            ) : (
              <p className="rounded-md bg-muted px-3 py-2 font-mono text-xs wrap-break-word">{item.modelOutput}</p>
            ))}
        </li>
      ))}
    </ul>
  )
}
