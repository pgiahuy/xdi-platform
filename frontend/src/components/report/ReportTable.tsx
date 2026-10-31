// SPDX-License-Identifier: Apache-2.0
/* components/report/ReportTable.tsx - bảng kết quả của một mục báo cáo: chỉ các dòng đầu (REPORT_ROWS), kèm tổng số dòng
và lời chỉ chỗ có đủ dòng (file Excel), để một mục vừa một trang in. Không phân trang hay truy dòng nguồn như bảng
của trang hỏi (components/ask/ResultTable.tsx): báo cáo là trang để đọc và in. Định dạng ô dùng chung cellFormat.ts. */

import { useId } from 'react'
import { buildDisplayKind, formatCell } from '@/components/ask/cellFormat'
import type { CellKind, ResultView } from '@/components/ask/types'
import { cn } from '@/lib/utils'

/** Số dòng đầu hiện trên trang báo cáo và bản in. */
export const REPORT_ROWS = 15

const countFormat = new Intl.NumberFormat('vi-VN')

type Props = {
  result: ResultView
}

export function ReportTable({ result }: Props) {
  const headingId = useId()
  const rows = result.rows.slice(0, REPORT_ROWS)
  const kinds = result.columns.map((column, c) =>
    buildDisplayKind(
      column.kind,
      result.rows.map((row) => row[c]),
    ),
  )
  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-2">
      <div className="flex flex-wrap items-baseline justify-between gap-x-3">
        <h4 id={headingId} className="text-sm font-semibold">
          Bảng kết quả
        </h4>
        <p className="text-xs text-muted-foreground">{describeRows(result, rows.length)}</p>
      </div>
      <div tabIndex={0} role="region" aria-labelledby={headingId} className="overflow-x-auto rounded-[10px] border print:overflow-visible">
        <table className="w-full text-sm">
          <thead className="bg-background text-xs text-muted-foreground">
            <tr>
              {result.columns.map((column) => (
                <th key={column.name} scope="col" className={cn('px-3 py-2 font-semibold', alignClass(column.kind))}>
                  {column.name}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((row, r) => (
              <tr key={r} className="border-t break-inside-avoid">
                {row.map((cell, c) => (
                  <td key={c} className={cn('px-3 py-1.5 tabular-nums', alignClass(result.columns[c].kind))}>
                    {formatCell(cell, kinds[c])}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  )
}

function describeRows(result: ResultView, shown: number): string {
  const total = countFormat.format(result.totalRows)
  const atLeast = result.limitReached ? 'ít nhất ' : ''
  if (shown >= result.totalRows) return `${atLeast}${total} dòng`
  const head = `${atLeast}${total} dòng, hiện ${countFormat.format(shown)} dòng đầu`
  // Backend gửi tối đa một số dòng (truncated): file Excel có đúng các dòng đã gửi, không phải toàn bộ.
  if (result.truncated) return `${head}; file Excel có ${countFormat.format(result.rows.length)} dòng đầu`
  return `${head}; đủ dòng trong file Excel`
}

function alignClass(kind: CellKind): string {
  return kind === 'number' || kind === 'decimal' ? 'text-right' : 'text-left'
}
