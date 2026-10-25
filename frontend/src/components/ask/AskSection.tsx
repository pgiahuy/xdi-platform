// SPDX-License-Identifier: Apache-2.0
/* components/ask/AskSection.tsx - thẻ có viền nhẹ cho một khối của trang hỏi (câu hỏi, câu gợi ý, kết quả), để mắt
tách được các khối mà không cần đường kẻ ngang. Tiêu đề là h2: h1 là tên trang trên thanh trên. */

import { useId, type ReactNode } from 'react'
import { cn } from '@/lib/utils'

type Props = {
  title: string
  /** Tiêu đề lớn hơn cho khối kết quả, nơi tiêu đề là chính câu vừa hỏi. */
  prominent?: boolean
  children: ReactNode
  className?: string
}

export function AskSection({ title, prominent = false, children, className }: Props) {
  const headingId = useId()
  return (
    <section
      aria-labelledby={headingId}
      className={cn('flex flex-col gap-4 rounded-xl border bg-card p-4 text-card-foreground sm:p-5', className)}
    >
      <h2 id={headingId} className={cn('font-semibold', prominent ? 'text-lg leading-snug' : 'text-base')}>
        {title}
      </h2>
      {children}
    </section>
  )
}
