// SPDX-License-Identifier: Apache-2.0
/* components/ask/AskSection.tsx - thẻ có viền nhẹ cho một khối của trang hỏi (câu hỏi, câu gợi ý, kết quả), để mắt
tách được các khối mà không cần đường kẻ ngang. Tiêu đề là h2: h1 là tên trang trên thanh trên. */

import { useId, type ReactNode } from 'react'
import { cn } from '@/lib/utils'

type Props = {
  title: string
  /** Tiêu đề lớn hơn cho khối kết quả, nơi tiêu đề là chính câu vừa hỏi. */
  prominent?: boolean
  /** Nhãn nhỏ phía trên tiêu đề, ví dụ "Câu trả lời", để biết thẻ này là gì khi tiêu đề là câu hỏi của người dùng. */
  eyebrow?: string
  children: ReactNode
  className?: string
}

export function AskSection({ title, prominent = false, eyebrow, children, className }: Props) {
  const headingId = useId()
  return (
    <section
      aria-labelledby={headingId}
      className={cn(
        'flex flex-col gap-4 rounded-xl border bg-card p-4 text-card-foreground shadow-xs sm:px-6 sm:py-5',
        className,
      )}
    >
      <div className="flex flex-col gap-1">
        {eyebrow && <p className="text-xs font-semibold tracking-wider text-label uppercase">{eyebrow}</p>}
        <h2
          id={headingId}
          className={cn('font-semibold tracking-tight', prominent ? 'text-lg leading-snug' : 'text-[15px]')}
        >
          {title}
        </h2>
      </div>
      {children}
    </section>
  )
}
