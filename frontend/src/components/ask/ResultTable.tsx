// SPDX-License-Identifier: Apache-2.0
/* components/ask/ResultTable.tsx - bảng kết quả: phân trang trong các dòng backend đã gửi (tối đa 1000), dòng ghi số
dòng, nút tải CSV. Số dòng ghi theo hợp đồng (ARCHITECTURE mục 8): bị cắt thì "N trên M dòng", chạm trần của guard thì
"ít nhất M dòng", để không nói sai tổng. Trang gắn key theo response để về trang 1 khi có kết quả mới.
Tab Dòng nguồn của pane bằng chứng dùng lại bảng này với tiêu đề riêng, không có nút tải CSV (ngoài phạm vi #52). */

import { useId, useState } from 'react'
import { ChevronLeft, ChevronRight, Download } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { formatCell } from './cellFormat'
import { buildResultCsv, downloadCsv } from './resultCsv'
import type { CellKind, ResultView } from './types'

type Props = {
  result: ResultView
  /** Tên file CSV khi tải; không có thì không có nút tải. */
  fileName?: string
  heading?: string
}

// 50 dòng vừa một hai lần cuộn trên màn laptop; 1000 dòng là 20 trang.
const pageSize = 50
const countFormat = new Intl.NumberFormat('vi-VN')

export function ResultTable({ result, fileName, heading = 'Kết quả' }: Props) {
  // id theo từng bảng: trang có thể có hai bảng cùng lúc (kết quả và dòng nguồn), id trùng thì nhãn trỏ sai bảng.
  const headingId = useId()
  const [page, setPage] = useState(0)
  const pageCount = Math.max(1, Math.ceil(result.rows.length / pageSize))
  const rows = result.rows.slice(page * pageSize, (page + 1) * pageSize)
  const partial = result.truncated || result.limitReached

  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h3 id={headingId} className="text-sm font-semibold">
            {heading}
          </h3>
          <p className="text-xs text-muted-foreground">{describeCount(result)}</p>
        </div>
        {fileName && (
          <Button variant="outline" size="sm" onClick={() => downloadCsv(buildResultCsv(result), fileName)}>
            <Download aria-hidden />
            Tải CSV
          </Button>
        )}
      </div>
      {fileName && partial && (
        <p className="text-xs text-muted-foreground">
          Tệp CSV chỉ gồm {countFormat.format(result.rows.length)} dòng đã nhận, không phải toàn bộ kết quả.
        </p>
      )}
      {/* Bảng rộng hơn màn thì cuộn ngang trong khung; khung nhận focus để người dùng bàn phím cuộn được. */}
      <div tabIndex={0} role="region" aria-labelledby={headingId} className="overflow-x-auto rounded-lg border">
        <table className="w-full text-sm">
          <thead className="bg-muted/50">
            <tr>
              {result.columns.map((column) => (
                <th
                  key={column.name}
                  scope="col"
                  className={cn('px-3 py-2 font-medium whitespace-nowrap', alignClass(column.kind))}
                >
                  {column.name}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((row, r) => (
              <tr key={page * pageSize + r} className="border-t">
                {row.map((cell, c) => (
                  <td key={c} className={cn('px-3 py-1.5', alignClass(result.columns[c].kind))}>
                    {cell === null ? (
                      <span className="text-muted-foreground">trống</span>
                    ) : (
                      formatCell(cell, result.columns[c].kind)
                    )}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {pageCount > 1 && (
        <nav
          aria-label={`Trang của bảng ${heading.toLowerCase()}`}
          className="flex items-center justify-end gap-2 text-sm"
        >
          <Button
            variant="outline"
            size="icon-sm"
            aria-label="Trang trước"
            disabled={page === 0}
            onClick={() => setPage(page - 1)}
          >
            <ChevronLeft aria-hidden />
          </Button>
          <span aria-live="polite">
            Trang {page + 1}/{pageCount}
          </span>
          <Button
            variant="outline"
            size="icon-sm"
            aria-label="Trang sau"
            disabled={page === pageCount - 1}
            onClick={() => setPage(page + 1)}
          >
            <ChevronRight aria-hidden />
          </Button>
        </nav>
      )}
    </section>
  )
}

function describeCount({ rows, totalRows, truncated, limitReached }: ResultView): string {
  const received = countFormat.format(rows.length)
  const total = countFormat.format(totalRows)
  if (limitReached) return `Ít nhất ${total} dòng (chạm giới hạn số dòng của truy vấn), hiện ${received} dòng đầu.`
  if (truncated) return `Hiện ${received} trên ${total} dòng.`
  return `${total} dòng.`
}

// Số canh phải với chữ số đều bề ngang, để cột số so được bằng mắt.
function alignClass(kind: CellKind): string {
  return kind === 'number' || kind === 'decimal' ? 'text-right tabular-nums' : 'text-left'
}
