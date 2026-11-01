// SPDX-License-Identifier: Apache-2.0
/* components/lessons/CardField.tsx - một phần có tên trong thẻ bài học hay thẻ nhật ký lỗi (#228): nhãn chữ nhỏ ở trên,
nội dung ở dưới. Thẻ ghi rõ phần nào là bài học, câu hỏi, lỗi, áp cho đâu, thay cho câu ghi chú giải thích cả thẻ. */

import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'

export function CardField({ label, children, className }: { label: string; children: ReactNode; className?: string }) {
  return (
    <div className={cn('flex min-w-0 flex-col gap-0.5', className)}>
      <dt className="text-[11px] font-medium tracking-wide text-muted-foreground uppercase">{label}</dt>
      <dd className="wrap-break-word">{children}</dd>
    </div>
  )
}
