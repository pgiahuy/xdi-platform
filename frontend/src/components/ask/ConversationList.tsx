// SPDX-License-Identifier: Apache-2.0
/* components/ask/ConversationList.tsx - danh sách cuộc trò chuyện cũ của người dùng trên mọi dataset của workspace (#164),
nội dung khung nổi "Lịch sử trò chuyện" của thanh bên (components/shell/ConversationFlyout.tsx). Đầu khung là tiêu đề
và nút dấu cộng mở cuộc mới; dưới là mỗi cuộc một dòng: tiêu đề (câu hỏi đầu, backend đã rút gọn), rồi dataset của cuộc
ngang hàng với thời gian bắt đầu ở mép phải, vì mỗi cuộc hỏi trên dataset riêng; cuộc đang mở có nền nhấn và aria-current.
Mỗi dòng cao đúng h-12 (3rem), tiêu đề một dòng (cắt bớt, đủ chữ ở title), nên khung danh sách cao tối đa 24rem
(max-h-96) hiện vừa 8 cuộc; nhiều hơn thì chỉ danh sách cuộn, đầu khung đứng yên. Đổi chiều cao dòng thì đổi cả
max-h. Màn thấp thì khung nổi giới hạn chiều cao, danh sách co lại (min-h-0) và hiện ít dòng hơn. */

import { Plus } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

export type ConversationEntry = {
  id: string
  title: string
  datasetName: string
  createdAt: string
}

type Props = {
  conversations: ConversationEntry[]
  activeId: string | null
  loading: boolean
  /** Lời báo khi tải lỗi; null khi không lỗi. */
  error: string | null
  /** Lời hiện khi tải xong mà không có cuộc nào. */
  emptyText: string
  onOpen: (id: string) => void
  /** Không có thì không có nút mở cuộc mới (chưa có dataset để hỏi). */
  onNew?: () => void
  disabled: boolean
}

const dateFormat = new Intl.DateTimeFormat('vi-VN', { dateStyle: 'short', timeStyle: 'short' })

export function ConversationList(props: Props) {
  const { conversations, activeId, loading, error, emptyText, onOpen, onNew, disabled } = props
  return (
    <nav aria-label="Các cuộc trò chuyện" className="flex min-h-0 flex-col gap-2">
      <div className="flex items-center gap-2 border-b border-border pb-2 pl-1">
        <h2 className="min-w-0 flex-1 text-sm font-semibold">Lịch sử trò chuyện</h2>
        {onNew && (
          <Button
            variant="outline"
            size="icon-sm"
            aria-label="Cuộc trò chuyện mới"
            title="Cuộc trò chuyện mới"
            onClick={onNew}
            disabled={disabled}
          >
            <Plus aria-hidden />
          </Button>
        )}
      </div>
      {loading && <p className="px-1 text-sm text-muted-foreground">Đang tải...</p>}
      {error && <p className="px-1 text-sm text-destructive">{error}</p>}
      {!loading && !error && conversations.length === 0 && (
        <p className="px-1 text-sm text-muted-foreground">{emptyText}</p>
      )}
      {conversations.length > 0 && (
        <ul className="scrollbar-subtle -mr-1 flex max-h-96 min-h-0 flex-col overflow-y-auto pr-1">
          {conversations.map((item) => {
            const active = item.id === activeId
            return (
              <li key={item.id}>
                <button
                  type="button"
                  aria-current={active ? 'true' : undefined}
                  onClick={() => onOpen(item.id)}
                  disabled={disabled}
                  className={cn(
                    // Viền focus vẽ vào trong: vùng cuộn cắt mất viền vẽ ra ngoài ở dòng đầu và dòng cuối.
                    // Rê chuột và cuộc đang mở dùng màu nhãn pha nhạt, đậm hơn nền muted gần trùng nền trắng của khung.
                    'flex h-12 w-full flex-col justify-center gap-0.5 rounded-md px-2 text-left text-[13px] leading-snug transition-colors outline-none hover:bg-label/15 hover:text-label focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset disabled:opacity-60',
                    active && 'bg-label/20 font-medium text-label',
                  )}
                >
                  {/* Hàng trên: tiêu đề; hàng dưới: dataset của cuộc và ngày giờ ngang hàng, ngày giờ sát mép phải. Mỗi
                  cuộc chỉ trên một dataset (đổi dataset ở trang cuộc trò chuyện là mở cuộc mới). */}
                  <span className="truncate" title={item.title}>
                    {item.title}
                  </span>
                  <span className="flex items-baseline gap-2 text-[11px] font-normal text-muted-foreground">
                    <span className="min-w-0 flex-1 truncate" title={item.datasetName}>
                      {item.datasetName}
                    </span>
                    <span className="shrink-0">{dateFormat.format(new Date(item.createdAt))}</span>
                  </span>
                </button>
              </li>
            )
          })}
        </ul>
      )}
    </nav>
  )
}
