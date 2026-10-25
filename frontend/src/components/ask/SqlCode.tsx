// SPDX-License-Identifier: Apache-2.0
/* components/ask/SqlCode.tsx - khung hiện một câu SQL: xuống dòng theo mệnh đề, tô màu từ khóa (sqlFormat.ts). Dùng ở
tab SQL và tab Dòng nguồn của pane bằng chứng. Dòng dài chỉ xuống dòng ở dấu cách, không ngắt giữa một từ (trước đây
"GROUP BY" bị cắt thành "GROUP B" và "Y"); một từ dài hơn khung thì khung cuộn ngang. */

import { useMemo } from 'react'
import { cn } from '@/lib/utils'
import { buildSqlParts, type SqlPartKind } from './sqlFormat'

// Màu chọn để chữ đạt tương phản WCAG AA (4,5:1) trên nền bg-muted, ở cả nền sáng và nền tối.
const partClass: Record<SqlPartKind, string> = {
  keyword: 'font-semibold text-sky-800 dark:text-sky-300',
  string: 'text-emerald-800 dark:text-emerald-300',
  // Tên cột trong ngoặc kép giữ trên một dòng ("Order Item Quantity" tách đôi thì trông như hai tên).
  identifier: 'text-foreground whitespace-pre',
  comment: 'text-muted-foreground italic',
  plain: '',
}

type Props = {
  sql: string
  className?: string
}

export function SqlCode({ sql, className }: Props) {
  const parts = useMemo(() => buildSqlParts(sql), [sql])
  return (
    <pre
      className={cn(
        'overflow-x-auto rounded-md bg-muted px-3 py-2 font-mono text-xs leading-relaxed whitespace-pre-wrap',
        className,
      )}
    >
      {parts.map((part, i) =>
        part.kind === 'plain' ? (
          part.text
        ) : (
          <span key={i} className={partClass[part.kind]}>
            {part.text}
          </span>
        ),
      )}
    </pre>
  )
}
