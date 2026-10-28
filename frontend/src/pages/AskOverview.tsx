// SPDX-License-Identifier: Apache-2.0
/* pages/AskOverview.tsx - khung "Tổng quan dữ liệu" trên trang hỏi, mở bằng nút cạnh ô chọn dataset (OverviewToggle) và
nằm ngay dưới dòng đó. Bấm một cột, giá trị, chỉ số hay biểu đồ thì câu hỏi điền vào ô hỏi ở đáy trang và tiêu điểm
chuyển xuống ô đó để người dùng sửa rồi gửi; khung vẫn mở để bấm tiếp. */

import { X } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { OverviewPanel } from './OverviewPanel'

/** id của khung, để nút mở trỏ aria-controls tới. */
export const askOverviewId = 'ask-data-overview'

type Props = {
  ws: string
  datasetId: string
  onPick: (question: string) => void
  onClose: () => void
}

export function AskOverview({ ws, datasetId, onPick, onClose }: Props) {
  const pick = (question: string) => {
    onPick(question)
    // Ô hỏi do QuestionForm vẽ với id cố định; chưa có (không hỏi được) thì bỏ qua.
    document.getElementById('ask-question')?.focus()
  }
  return (
    <section
      id={askOverviewId}
      aria-labelledby={`${askOverviewId}-title`}
      className="flex flex-col gap-3 rounded-xl border bg-card p-4 text-card-foreground shadow-sm"
    >
      <div className="flex items-center justify-between gap-2">
        <h2 id={`${askOverviewId}-title`} className="text-[15px] font-semibold tracking-tight">
          Tổng quan dữ liệu
        </h2>
        <Button variant="ghost" size="icon" aria-label="Đóng tổng quan dữ liệu" onClick={onClose}>
          <X aria-hidden />
        </Button>
      </div>
      <p className="text-xs text-muted-foreground">Bấm một cột, giá trị hay biểu đồ để điền câu hỏi vào ô hỏi.</p>
      <OverviewPanel ws={ws} datasetId={datasetId} onPick={pick} />
    </section>
  )
}
