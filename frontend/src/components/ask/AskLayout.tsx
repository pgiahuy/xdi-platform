// SPDX-License-Identifier: Apache-2.0
/* components/ask/AskLayout.tsx - bố cục trang hỏi: cột danh sách cuộc trò chuyện bên trái (từ lg), cột chính (khung
trò chuyện và ô hỏi) và cột bằng chứng bên phải trên màn rộng, xuống dưới cột chính trên màn hẹp. Dưới lg cột danh
sách ẩn, trang mở danh sách trong ngăn kéo. Cột bằng chứng là chỗ của pane bằng chứng (#52: SQL, phân rã, dòng nguồn,
các bước); trang truyền nội dung vào khi đã có câu trả lời, không có thì cột không chiếm chỗ.
Từ xl cột chính và cột bằng chứng chia 3:2, cột bằng chứng không hẹp hơn 26rem để bảng phân rã bốn cột số đọc được
không cần cuộn ngang; giữa lg và xl cột bằng chứng xuống dưới cột chính. Thu gọn thì cột bằng chứng biến mất, chỉ còn
nút "Mở minh chứng" (từ xl dính mép phải màn hình, ngang đầu trang), cột chính rộng ra; trang giữ trạng thái này. Cột danh sách ẩn được (nút ở đầu cột, trang nhớ trạng thái);
ẩn thì đầu cột chính có nút hiện lại. Cột danh sách và cột bằng chứng dính dưới thanh trên (h-16) khi
cuộn, cuộn riêng khi dài hơn màn: cuộc trò chuyện dài thì bằng chứng của lượt đang chọn vẫn ở trong tầm mắt. */

import type { ReactNode } from 'react'
import { PanelLeftClose, PanelLeftOpen, PanelRightClose, PanelRightOpen, ShieldCheck } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

type Props = {
  children: ReactNode
  /** Danh sách cuộc trò chuyện, cột trái từ lg. */
  history?: ReactNode
  /** Cột danh sách đang hiện (từ lg); ẩn thì chỉ còn nút hiện lại ở đầu cột chính. */
  historyOpen: boolean
  onHistoryOpenChange: (open: boolean) => void
  /** Nút đứng cạnh nút hiện lại danh sách khi cột đang ẩn (Cuộc trò chuyện mới): ẩn danh sách không làm mất lối tạo
   * cuộc mới. */
  historyHiddenActions?: ReactNode
  evidence?: ReactNode
  evidenceOpen: boolean
  onEvidenceOpenChange: (open: boolean) => void
}

export function AskLayout(props: Props) {
  const { children, evidence, evidenceOpen, onEvidenceOpenChange, historyOpen, onHistoryOpenChange } = props
  const open = evidenceOpen && Boolean(evidence)
  // Ẩn cột danh sách thì bố cục như trang không có danh sách; danh sách vẫn mở được trong ngăn kéo dưới lg.
  const history = historyOpen ? props.history : undefined
  return (
    <div
      className={cn(
        'grid grid-cols-1 items-start gap-6',
        history && 'lg:grid-cols-[13rem_minmax(0,1fr)]',
        open &&
          (history
            ? 'xl:grid-cols-[13rem_minmax(0,3fr)_minmax(26rem,2fr)]'
            : 'xl:grid-cols-[minmax(0,3fr)_minmax(26rem,2fr)]'),
      )}
    >
      {history && (
        // Cột danh sách là một khung riêng như pane minh chứng, để không lẫn với câu gợi ý và khung trò chuyện bên cạnh.
        <div
          className={cn(
            'hidden lg:flex lg:flex-col lg:gap-2 lg:rounded-xl lg:border lg:bg-card lg:p-3 lg:text-card-foreground lg:shadow-xs',
            // Cột không tự cuộn: phần đầu cột đứng yên, danh sách bên trong cuộn riêng (ConversationList).
            'lg:sticky lg:top-20 lg:max-h-[calc(100dvh-6rem)] lg:overflow-hidden',
          )}
        >
          {/* Tên cột và nút ẩn cùng một hàng: nút đứng riêng một hàng thì đầu cột trống một khoảng. */}
          <div className="flex items-center justify-between gap-2">
            <h2 className="text-sm font-semibold">Cuộc trò chuyện</h2>
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label="Ẩn danh sách cuộc trò chuyện"
              title="Ẩn danh sách cuộc trò chuyện"
              onClick={() => onHistoryOpenChange(false)}
            >
              <PanelLeftClose aria-hidden />
            </Button>
          </div>
          {history}
        </div>
      )}
      {/* Khung trò chuyện có nền riêng (secondary), khác nền trang và nền thẻ trả lời, để nhìn ra ranh giới với cột
      danh sách và cột minh chứng. */}
      <div className="flex min-w-0 flex-col gap-6 border bg-secondary p-4 sm:p-5">
        {props.history && !historyOpen && (
          <div className="hidden flex-wrap items-center gap-2 lg:flex">
            <Button variant="outline" size="sm" onClick={() => onHistoryOpenChange(true)}>
              <PanelLeftOpen aria-hidden />
              Hiện danh sách cuộc trò chuyện
            </Button>
            {props.historyHiddenActions}
          </div>
        )}
        {children}
      </div>
      {evidence && (
        <aside
          id="evidence"
          aria-labelledby="evidence-heading"
          // Thu gọn chỉ ẩn, không bỏ nội dung: tab đang mở và dòng nguồn vừa truy vẫn còn khi mở lại.
          hidden={!open}
          className={cn(
            'flex min-w-0 scroll-mt-20 flex-col gap-3 rounded-xl border bg-card p-4 text-card-foreground shadow-xs sm:p-5',
            // Giữa lg và xl cột minh chứng nằm dưới cột chính, đúng dưới khung trò chuyện, không dưới cột danh sách.
            history && 'lg:col-start-2 xl:col-start-auto',
            'xl:sticky xl:top-20 xl:max-h-[calc(100dvh-6rem)] xl:overflow-y-auto',
          )}
        >
          <div className="flex items-center justify-between gap-3">
            <h2 id="evidence-heading" className="flex items-center gap-2 text-[15px] font-semibold">
              <ShieldCheck aria-hidden className="size-[18px] text-label" />
              Minh chứng
            </h2>
            <Button variant="outline" size="sm" aria-expanded={open} onClick={() => onEvidenceOpenChange(false)}>
              <PanelRightClose aria-hidden />
              Thu gọn
            </Button>
          </div>
          <div className="min-w-0">{evidence}</div>
        </aside>
      )}
      {evidence && !open && (
        // Thu gọn thì từ xl pane chỉ còn nút nhỏ dính mép phải màn hình, không chiếm cột nào: khung trò chuyện rộng hết
        // chỗ. Dưới xl pane vốn nằm dưới khung trò chuyện, nút dính mép sẽ đè lên thẻ trả lời; ở đó nút nằm đúng chỗ pane.
        <Button
          variant="outline"
          aria-label="Mở minh chứng"
          aria-expanded={false}
          aria-controls="evidence"
          title="Mở minh chứng"
          onClick={() => onEvidenceOpenChange(true)}
          className={cn(
            'justify-self-start',
            history && 'lg:col-start-2',
            'xl:fixed xl:top-24 xl:right-0 xl:z-20 xl:h-auto xl:flex-col xl:gap-2 xl:rounded-r-none xl:border-r-0 xl:px-1.5 xl:py-3 xl:shadow-sm',
          )}
        >
          <PanelRightOpen aria-hidden />
          <span aria-hidden className="font-semibold xl:text-xs xl:[writing-mode:vertical-rl]">
            Minh chứng
          </span>
        </Button>
      )}
    </div>
  )
}
