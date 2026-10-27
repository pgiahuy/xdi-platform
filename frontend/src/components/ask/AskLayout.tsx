// SPDX-License-Identifier: Apache-2.0
/* components/ask/AskLayout.tsx - bố cục trang hỏi: cột danh sách cuộc trò chuyện bên trái (từ lg), cột chính (khung
trò chuyện và ô hỏi) và cột bằng chứng bên phải trên màn rộng, xuống dưới cột chính trên màn hẹp. Dưới lg cột danh
sách ẩn, trang mở danh sách trong ngăn kéo. Cột bằng chứng là chỗ của pane bằng chứng (#52: SQL, phân rã, dòng nguồn,
các bước); trang truyền nội dung vào khi đã có câu trả lời, không có thì cột không chiếm chỗ.
Từ xl cột chính và cột bằng chứng chia 3:2, cột bằng chứng không hẹp hơn 26rem để bảng phân rã bốn cột số đọc được
không cần cuộn ngang; giữa lg và xl cột bằng chứng xuống dưới cột chính. Thu gọn thì cột bằng chứng chỉ còn tiêu đề và
nút mở, cột chính rộng ra; trang giữ trạng thái này. Cột danh sách và cột bằng chứng dính dưới thanh trên (h-16) khi
cuộn, cuộn riêng khi dài hơn màn: cuộc trò chuyện dài thì bằng chứng của lượt đang chọn vẫn ở trong tầm mắt. */

import type { ReactNode } from 'react'
import { PanelRightClose, PanelRightOpen, ShieldCheck } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

type Props = {
  children: ReactNode
  /** Danh sách cuộc trò chuyện, cột trái từ lg. */
  history?: ReactNode
  evidence?: ReactNode
  evidenceOpen: boolean
  onEvidenceOpenChange: (open: boolean) => void
}

export function AskLayout({ children, history, evidence, evidenceOpen, onEvidenceOpenChange }: Props) {
  const open = evidenceOpen && Boolean(evidence)
  return (
    <div
      className={cn(
        'grid grid-cols-1 items-start gap-6',
        history && 'lg:grid-cols-[13rem_minmax(0,1fr)]',
        evidence &&
          (history
            ? open
              ? 'xl:grid-cols-[13rem_minmax(0,3fr)_minmax(26rem,2fr)]'
              : 'xl:grid-cols-[13rem_minmax(0,1fr)_auto]'
            : open
              ? 'xl:grid-cols-[minmax(0,3fr)_minmax(26rem,2fr)]'
              : 'xl:grid-cols-[minmax(0,1fr)_auto]'),
      )}
    >
      {history && <div className={cn('hidden lg:block', stickyColumn)}>{history}</div>}
      <div className="flex min-w-0 flex-col gap-6">{children}</div>
      {evidence && (
        <aside
          id="evidence"
          aria-labelledby="evidence-heading"
          className={cn(
            'flex min-w-0 scroll-mt-20 flex-col gap-3 rounded-xl border bg-card p-4 text-card-foreground shadow-xs sm:p-5',
            // Giữa lg và xl cột bằng chứng nằm dưới cột chính, đúng dưới khung trò chuyện, không dưới cột danh sách.
            history && 'lg:col-start-2 xl:col-start-auto',
            'xl:sticky xl:top-20 xl:max-h-[calc(100dvh-6rem)] xl:overflow-y-auto',
          )}
        >
          <div className="flex items-center justify-between gap-3">
            <h2 id="evidence-heading" className="flex items-center gap-2 text-[15px] font-semibold">
              <ShieldCheck aria-hidden className="size-[18px] text-label" />
              Bằng chứng
            </h2>
            <Button
              variant="outline"
              size="sm"
              aria-expanded={open}
              aria-controls="evidence-body"
              onClick={() => onEvidenceOpenChange(!open)}
            >
              {open ? <PanelRightClose aria-hidden /> : <PanelRightOpen aria-hidden />}
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

const stickyColumn = 'lg:sticky lg:top-20 lg:max-h-[calc(100dvh-6rem)] lg:overflow-y-auto'
