// SPDX-License-Identifier: Apache-2.0
/* components/ask/EvidenceRows.tsx - tab Dòng nguồn của pane bằng chứng: các dòng dữ liệu đứng sau một dòng phân rã
(một lát cắt, một cột, một giá trị nhóm), bảng dùng lại ResultTable. Trang truy dòng và truyền trạng thái vào; khi
backend chưa truy được (route drill trả 501, chờ #34) trang truyền lời báo qua state error. */

import { formatCell } from './cellFormat'
import type { DrillTarget, RowsView } from './evidenceTypes'
import { ResultTable } from './ResultTable'

type Props = {
  rows: RowsView
  /** Nhãn của hai lát cắt, để ghi đang xem lát nào. */
  sliceLabels: { base: string; compare: string }
}

export function EvidenceRows({ rows, sliceLabels }: Props) {
  if (rows.state === 'idle') {
    return (
      <p className="text-sm text-muted-foreground">
        Bấm số của một kỳ trong tab Phân rã để xem dòng nguồn của nhóm đó.
      </p>
    )
  }
  return (
    <div className="flex flex-col gap-3">
      <p className="text-sm">
        <span className="text-muted-foreground">Đang xem: </span>
        {describeTarget(rows.target, sliceLabels)}
      </p>
      <RowsBody rows={rows} />
    </div>
  )
}

function RowsBody({ rows }: { rows: Exclude<RowsView, { state: 'idle' }> }) {
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
          {rows.sample && (
            <p className="w-fit rounded-md border border-dashed px-2 py-0.5 text-xs font-medium">
              Dữ liệu mẫu: vài dòng minh họa để xem bố cục, không lấy từ dataset
            </p>
          )}
          <ResultTable result={rows.result} heading="Dòng nguồn" />
          <details className="text-xs">
            <summary className="cursor-pointer text-muted-foreground">SQL đã chạy để lấy các dòng này</summary>
            <pre className="mt-1 rounded-md bg-muted px-3 py-2 font-mono break-all whitespace-pre-wrap">{rows.sql}</pre>
          </details>
        </>
      )
  }
}

function describeTarget(target: DrillTarget, sliceLabels: { base: string; compare: string }): string {
  const value = target.value === null ? 'trống' : formatCell(target.value, target.kind)
  return `${target.column} = ${value}, ${sliceLabels[target.slice]}`
}
