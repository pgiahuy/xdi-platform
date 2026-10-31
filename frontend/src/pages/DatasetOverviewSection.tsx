// SPDX-License-Identifier: Apache-2.0
/* pages/DatasetOverviewSection.tsx - khung tổng quan một bảng trên trang Tổng quan dữ liệu (OverviewPage.tsx). Tổng
quan vẫn theo từng bảng (route /datasets/{id}/overview nhận id bảng, #257). Bấm một cột, giá trị hay biểu đồ thì mở
trang hỏi trên dataset chứa bảng, ghim đúng bảng đó, với câu hỏi điền sẵn trong ô hỏi (?dataset=...&table=...&q=...,
AskPage đọc), chưa gửi. */

import { useNavigate } from 'react-router'
import { OverviewPanel } from './OverviewPanel'
import { workspaceHref } from './workspaceNav'

type Props = {
  ws: string
  /** Dataset chứa bảng (DatasetSummary.id). */
  datasetId: string
  table: { id: string; name: string }
}

export function DatasetOverviewSection({ ws, datasetId, table }: Props) {
  const navigate = useNavigate()
  const pick = (question: string) => {
    const params = new URLSearchParams({ dataset: datasetId, table: table.id, q: question })
    void navigate(`${workspaceHref(ws, '')}?${params.toString()}`)
  }
  return (
    <section
      aria-labelledby="dataset-overview-heading"
      className="flex min-w-0 flex-col gap-3 rounded-xl border bg-card p-4 shadow-xs"
    >
      {/* Tiêu đề trang đã là "Tổng quan dữ liệu"; khung chỉ cần tên bảng. */}
      <h2 id="dataset-overview-heading" className="text-[15px] font-semibold tracking-tight break-all">
        {table.name}
      </h2>
      <p className="text-xs text-muted-foreground">
        Bấm một cột, giá trị hay biểu đồ để mở trang hỏi với câu hỏi điền sẵn.
      </p>
      <OverviewPanel ws={ws} datasetId={table.id} onPick={pick} />
    </section>
  )
}
