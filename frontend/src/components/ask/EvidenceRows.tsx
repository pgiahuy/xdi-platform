// SPDX-License-Identifier: Apache-2.0
/* components/ask/EvidenceRows.tsx - tab Dòng nguồn của pane bằng chứng: các dòng dữ liệu đứng sau một dòng phân rã
(một lát cắt, một cột, một giá trị nhóm) hoặc một ô số đo của bảng kết quả, bảng dùng lại ResultTable. Trang truy dòng
và truyền trạng thái vào; khi backend không truy được (trả 400 kèm lý do) trang truyền lời báo qua state error.
Backend trả từng trang dòng nguồn kèm tổng số dòng (DrillResponse.total_rows); nút trang ở đây xin trang khác qua
onPageChange, vì một chỗ bấm có thể đứng sau hàng nghìn dòng, nhiều hơn một lần gửi. Trên bảng là phần nói dòng đến từ
bảng nào (câu nối bảng: bảng mà độ đo cộng, cột nào lấy từ bảng nối) và độ đo tính lại trên mọi dòng, so với ô đã bấm,
để người dùng tự kiểm con số bằng dòng nguồn. */

import { ChevronLeft, ChevronRight } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { formatCell } from './cellFormat'
import { describeDrillTarget } from './drillTarget'
import type { RowsMeasure, RowsPage, RowsSource, RowsView } from './evidenceTypes'
import { ResultTable } from './ResultTable'
import { SqlCode } from './SqlCode'

type Props = {
  rows: RowsView
  /** Nhãn của hai lát cắt, để ghi đang xem lát nào. */
  sliceLabels: { base: string; compare: string }
  /** Xin trang dòng nguồn bắt đầu ở offset. */
  onPageChange: (offset: number) => void
}

const countFormat = new Intl.NumberFormat('vi-VN')

export function EvidenceRows({ rows, sliceLabels, onPageChange }: Props) {
  if (rows.state === 'idle') return <p className="text-sm text-muted-foreground">{rows.hint}</p>
  return (
    <div className="flex flex-col gap-3">
      <p className="text-sm">
        <span className="text-muted-foreground">Đang xem: </span>
        {describeDrillTarget(rows.target, sliceLabels)}
      </p>
      <RowsBody rows={rows} onPageChange={onPageChange} />
    </div>
  )
}

type BodyProps = {
  rows: Exclude<RowsView, { state: 'idle' }>
  onPageChange: (offset: number) => void
}

function RowsBody({ rows, onPageChange }: BodyProps) {
  switch (rows.state) {
    case 'loading':
      return (
        <p role="status" className="text-sm text-muted-foreground">
          Đang tải dòng nguồn...
        </p>
      )
    case 'error':
      return (
        <p role="alert" className="rounded-md bg-muted px-3 py-2 text-sm">
          {rows.message}
        </p>
      )
    case 'ready':
      return (
        <>
          <SourceNote source={rows.source} total={rows.page.total} />
          <ResultTable result={rows.result} heading="Dòng nguồn" nowrap />
          <PageNav page={rows.page} onPageChange={onPageChange} />
          <details className="text-xs">
            <summary className="cursor-pointer text-muted-foreground">SQL đã chạy để lấy các dòng này</summary>
            <SqlCode sql={rows.sql} className="mt-1" />
          </details>
        </>
      )
  }
}

function SourceNote({ source, total }: { source: RowsSource; total: number }) {
  const from = describeFrom(source)
  const measure = source.measure ? describeMeasure(source.measure, total) : null
  if (!from && !measure) return null
  return (
    <div className="flex flex-col gap-1 text-sm">
      {from && <p className="text-muted-foreground">{from}</p>}
      {measure && <p>{measure}</p>}
    </div>
  )
}

function describeFrom({ table, joinedColumns }: RowsSource): string | null {
  if (!table) return null
  if (joinedColumns.length === 0) return `Mỗi dòng là một dòng của bảng ${table}.`
  return `Mỗi dòng là một dòng của bảng ${table}; cột ${joinedColumns.join(', ')} lấy từ bảng nối theo khóa ngoại.`
}

function describeMeasure(measure: RowsMeasure, total: number): string {
  const value = measure.value === null ? 'trống' : formatCell(measure.value, measure.kind)
  const rows = countFormat.format(total)
  const head =
    measure.rule === 'sum'
      ? `Tổng cột ${measure.rowColumn ?? measure.column} của ${rows} dòng: ${value}`
      : measure.rule === 'count'
        ? `Đếm được ${rows} dòng: ${value}`
        : `${measure.column} tính lại trên ${rows} dòng: ${value}`
  if (!measure.compared) return `${head}.`
  if (sameValue(measure.value, measure.clicked)) return `${head}, bằng ô đã bấm.`
  const clicked = measure.clicked === null ? 'trống' : formatCell(measure.clicked, measure.kind)
  return `${head}, khác ô đã bấm (${clicked}), có thể vì vai dữ liệu của bạn chỉ thấy một phần các dòng của con số này.`
}

// Hai bên là cùng hàm tổng hợp trên cùng tập dòng; số thực có thể lệch ở chữ số cuối do thứ tự cộng, nên so tương đối.
function sameValue(a: RowsMeasure['value'], b: RowsMeasure['value']): boolean {
  if (a === b) return true
  const x = Number(a)
  const y = Number(b)
  if (a === null || b === null || Number.isNaN(x) || Number.isNaN(y)) return false
  return Math.abs(x - y) <= 1e-9 * Math.max(1, Math.abs(x), Math.abs(y))
}

function PageNav({ page, onPageChange }: { page: RowsPage; onPageChange: (offset: number) => void }) {
  if (page.total <= page.limit) return null
  const last = Math.min(page.offset + page.limit, page.total)
  return (
    <nav aria-label="Trang dòng nguồn" className="flex items-center justify-end gap-2 text-sm">
      <Button
        variant="outline"
        size="icon-sm"
        aria-label="Trang dòng nguồn trước"
        disabled={page.offset === 0}
        onClick={() => onPageChange(Math.max(0, page.offset - page.limit))}
      >
        <ChevronLeft aria-hidden />
      </Button>
      <span aria-live="polite" className="tabular-nums">
        Dòng {countFormat.format(page.offset + 1)} đến {countFormat.format(last)}
      </span>
      <Button
        variant="outline"
        size="icon-sm"
        aria-label="Trang dòng nguồn sau"
        disabled={last >= page.total}
        onClick={() => onPageChange(page.offset + page.limit)}
      >
        <ChevronRight aria-hidden />
      </Button>
    </nav>
  )
}
