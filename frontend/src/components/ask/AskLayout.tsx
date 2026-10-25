// SPDX-License-Identifier: Apache-2.0
/* components/ask/AskLayout.tsx - bố cục trang hỏi: cột chính (ô hỏi, câu gợi ý, kết quả) và cột bằng chứng bên phải
trên màn rộng, xuống dưới kết quả trên màn hẹp. Cột bằng chứng là chỗ của pane bằng chứng (#52: SQL, phân rã, dòng
nguồn, các bước); trang truyền nội dung vào khi đã có câu trả lời, không có thì cột không chiếm chỗ.
Trên màn rộng hai cột chia 3:2, cột bằng chứng không hẹp hơn 26rem để bảng phân rã bốn cột số đọc được không cần cuộn
ngang. Thu gọn thì cột bằng chứng chỉ còn tiêu đề và nút mở, cột chính rộng ra; trang giữ trạng thái này. */

import type { ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

type Props = {
  children: ReactNode
  evidence?: ReactNode
  evidenceOpen: boolean
  onEvidenceOpenChange: (open: boolean) => void
}

export function AskLayout({ children, evidence, evidenceOpen, onEvidenceOpenChange }: Props) {
  const open = evidenceOpen && Boolean(evidence)
  return (
    <div
      className={cn(
        'grid grid-cols-1 items-start gap-6',
        evidence && (open ? 'xl:grid-cols-[minmax(0,3fr)_minmax(26rem,2fr)]' : 'xl:grid-cols-[minmax(0,1fr)_auto]'),
      )}
    >
      <div className="flex min-w-0 flex-col gap-6">{children}</div>
      {evidence && (
        <aside
          aria-labelledby="evidence-heading"
          className="flex min-w-0 flex-col gap-3 rounded-xl border bg-card p-4 text-card-foreground sm:p-5"
        >
          <div className="flex items-center justify-between gap-3">
            <h2 id="evidence-heading" className="text-base font-semibold">
              Bằng chứng
            </h2>
            <Button
              variant="ghost"
              size="sm"
              aria-expanded={open}
              aria-controls="evidence-body"
              onClick={() => onEvidenceOpenChange(!open)}
            >
              {open ? 'Thu gọn' : 'Mở'}
            </Button>
          </div>
          {/* Thu gọn chỉ ẩn, không bỏ nội dung: tab đang mở và dòng nguồn vừa truy vẫn còn khi mở lại. */}
          <div id="evidence-body" hidden={!open} className="min-w-0">
            {evidence}
          </div>
        </aside>
      )}
    </div>
  )
}
