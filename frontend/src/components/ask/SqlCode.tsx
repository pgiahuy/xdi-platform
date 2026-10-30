// SPDX-License-Identifier: Apache-2.0
/* components/ask/SqlCode.tsx - khung hiện một câu SQL: xuống dòng theo mệnh đề, tô màu từ khóa (sqlFormat.ts). Dùng ở
tab SQL và tab Dòng nguồn của pane bằng chứng. Dòng dài chỉ xuống dòng ở dấu cách, không ngắt giữa một từ (trước đây
"GROUP BY" bị cắt thành "GROUP B" và "Y"); một từ dài hơn khung thì khung cuộn ngang.
Có compareTo (bản mô hình viết) thì dòng nào không có trong bản đó được đánh dấu: vạch hổ phách bên trái và chữ ẩn
"dòng guard thêm" cho trình đọc màn hình. So theo từng dòng đã định dạng, bỏ khoảng trắng đầu cuối; đủ để thấy guard
thêm LIMIT hay điều kiện lọc, không phải một bộ so khác biệt đầy đủ. */

import { useMemo } from 'react'
import { cn } from '@/lib/utils'
import { buildSqlParts, type SqlPart, type SqlPartKind } from './sqlFormat'

// Khung nền mực: màu chữ chọn để đạt tương phản WCAG AA (4,5:1) trên nền --code (giá trị ở index.css).
const partClass: Record<SqlPartKind, string> = {
  keyword: 'font-medium text-code-keyword',
  string: 'text-highlight-border',
  // Tên cột trong ngoặc kép giữ trên một dòng ("Order Item Quantity" tách đôi thì trông như hai tên).
  identifier: 'text-code-identifier whitespace-pre',
  comment: 'text-code-muted italic',
  plain: '',
}

type Props = {
  sql: string
  /** Bản SQL để so; dòng của sql không có trong bản này được đánh dấu là guard thêm. */
  compareTo?: string | null
  className?: string
}

export function SqlCode({ sql, compareTo, className }: Props) {
  const lines = useMemo(() => buildLines(buildSqlParts(sql)), [sql])
  const added = useMemo(() => {
    if (!compareTo) return lines.map(() => false)
    const known = new Set(buildLines(buildSqlParts(compareTo)).map(lineText))
    return lines.map((line) => lineText(line) !== '' && !known.has(lineText(line)))
  }, [lines, compareTo])

  return (
    <pre
      className={cn(
        'overflow-x-auto rounded-[9px] bg-code px-3.5 py-3 font-mono text-xs leading-relaxed whitespace-pre-wrap text-code-foreground',
        className,
      )}
    >
      {lines.map((line, i) => (
        <span
          key={i}
          className={cn('block', added[i] && '-mx-3.5 border-l-[3px] border-highlight bg-code-added pr-3.5 pl-[11px]')}
        >
          {line.map((part, j) =>
            part.kind === 'plain' ? (
              part.text
            ) : (
              <span key={j} className={partClass[part.kind]}>
                {part.text}
              </span>
            ),
          )}
          {added[i] && <span className="sr-only"> (dòng guard thêm)</span>}
        </span>
      ))}
    </pre>
  )
}

/** Chia các mảnh thành từng dòng ở ký tự xuống dòng; mảnh chứa xuống dòng (chú thích nhiều dòng) tách làm nhiều mảnh. */
function buildLines(parts: SqlPart[]): SqlPart[][] {
  const lines: SqlPart[][] = [[]]
  for (const part of parts) {
    part.text.split('\n').forEach((text, k) => {
      if (k > 0) lines.push([])
      if (text !== '') lines[lines.length - 1].push({ text, kind: part.kind })
    })
  }
  return lines
}

function lineText(line: SqlPart[]): string {
  return line
    .map((part) => part.text)
    .join('')
    .trim()
}
