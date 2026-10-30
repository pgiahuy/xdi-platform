// SPDX-License-Identifier: Apache-2.0
/* pages/DatasetOverviewSection.tsx - tổng quan một dataset trên trang Dữ liệu, mở bằng nút "Tổng quan" của dòng dataset
(components/datasets/DatasetList.tsx). Bấm một cột, giá trị hay biểu đồ thì mở trang hỏi trên đúng dataset đó với câu
hỏi điền sẵn trong ô hỏi (?dataset=...&q=..., AskPage đọc), chưa gửi. */

import { useEffect, useRef } from 'react'
import { X } from 'lucide-react'
import { useNavigate } from 'react-router'
import { Button } from '@/components/ui/button'
import { OverviewPanel } from './OverviewPanel'
import { workspaceHref } from './workspaceNav'

type Props = {
  ws: string
  dataset: { id: string; name: string }
  onClose: () => void
}

export function DatasetOverviewSection({ ws, dataset, onClose }: Props) {
  const navigate = useNavigate()
  const heading = useRef<HTMLHeadingElement>(null)
  // Khung nằm dưới danh sách, có thể khuất: đưa tiêu điểm tới tiêu đề để người dùng thấy ngay, như khung nạp phiên bản.
  useEffect(() => {
    heading.current?.focus()
    heading.current?.scrollIntoView({ block: 'start' })
  }, [dataset.id])

  const pick = (question: string) => {
    const params = new URLSearchParams({ dataset: dataset.id, q: question })
    void navigate(`${workspaceHref(ws, '')}?${params.toString()}`)
  }
  return (
    <section aria-labelledby="dataset-overview-heading" className="flex flex-col gap-3 rounded-xl border bg-card p-4 shadow-xs">
      <div className="flex items-center justify-between gap-2">
        <h2
          id="dataset-overview-heading"
          ref={heading}
          tabIndex={-1}
          className="text-[15px] font-semibold tracking-tight outline-none"
        >
          {`Tổng quan dữ liệu: ${dataset.name}`}
        </h2>
        <Button variant="ghost" size="icon" aria-label="Đóng tổng quan dữ liệu" onClick={onClose}>
          <X aria-hidden />
        </Button>
      </div>
      <p className="text-xs text-muted-foreground">
        Bấm một cột, giá trị hay biểu đồ để mở trang hỏi với câu hỏi điền sẵn.
      </p>
      <OverviewPanel ws={ws} datasetId={dataset.id} onPick={pick} />
    </section>
  )
}
