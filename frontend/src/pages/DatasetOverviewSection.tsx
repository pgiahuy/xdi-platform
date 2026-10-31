// SPDX-License-Identifier: Apache-2.0
/* pages/DatasetOverviewSection.tsx - khung tổng quan một dataset trên trang Tổng quan dữ liệu (OverviewPage.tsx). Bấm
một cột, giá trị hay biểu đồ thì mở trang hỏi trên đúng dataset đó với câu hỏi điền sẵn trong ô hỏi
(?dataset=...&q=..., AskPage đọc), chưa gửi. */

import { useNavigate } from 'react-router'
import { OverviewPanel } from './OverviewPanel'
import { workspaceHref } from './workspaceNav'

type Props = {
  ws: string
  dataset: { id: string; name: string }
}

export function DatasetOverviewSection({ ws, dataset }: Props) {
  const navigate = useNavigate()
  const pick = (question: string) => {
    const params = new URLSearchParams({ dataset: dataset.id, q: question })
    void navigate(`${workspaceHref(ws, '')}?${params.toString()}`)
  }
  return (
    <section aria-labelledby="dataset-overview-heading" className="flex flex-col gap-3 rounded-xl border bg-card p-4 shadow-xs">
      {/* Tiêu đề trang đã là "Tổng quan dữ liệu"; khung chỉ cần tên dataset. */}
      <h2 id="dataset-overview-heading" className="text-[15px] font-semibold tracking-tight">
        {dataset.name}
      </h2>
      <p className="text-xs text-muted-foreground">
        Bấm một cột, giá trị hay biểu đồ để mở trang hỏi với câu hỏi điền sẵn.
      </p>
      <OverviewPanel ws={ws} datasetId={dataset.id} onPick={pick} />
    </section>
  )
}
