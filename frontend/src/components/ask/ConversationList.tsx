// SPDX-License-Identifier: Apache-2.0
/* components/ask/ConversationList.tsx - danh sách cuộc trò chuyện cũ của người dùng trên dataset đang chọn (#164):
nút "Cuộc trò chuyện mới", rồi mỗi cuộc một dòng với tiêu đề (câu hỏi đầu, backend đã rút gọn) và thời gian bắt đầu;
cuộc đang mở có nền nhấn và aria-current. Chữ nhỏ, tiêu đề cắt ở hai dòng (đủ chữ ở title) để cột hẹp mà vẫn thấy
nhiều cuộc. Trang đặt danh sách ở cột trái trên màn rộng và trong ngăn kéo trên màn hẹp,
nên ở đây không quyết định chỗ đặt. */

import { Plus } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

export type ConversationEntry = {
  id: string
  title: string
  createdAt: string
}

type Props = {
  conversations: ConversationEntry[]
  activeId: string | null
  loading: boolean
  /** Lời báo khi tải danh sách lỗi; null khi không lỗi. */
  error: string | null
  onOpen: (id: string) => void
  onNew: () => void
  disabled: boolean
}

const dateFormat = new Intl.DateTimeFormat('vi-VN', { dateStyle: 'short', timeStyle: 'short' })

export function ConversationList({ conversations, activeId, loading, error, onOpen, onNew, disabled }: Props) {
  return (
    <nav aria-label="Các cuộc trò chuyện" className="flex min-h-0 flex-col gap-3">
      <Button variant="outline" size="sm" onClick={onNew} disabled={disabled} className="w-full justify-start">
        <Plus aria-hidden />
        Cuộc trò chuyện mới
      </Button>
      <p className="text-xs font-semibold tracking-wider text-label uppercase">Đã hỏi</p>
      {loading && <p className="text-sm text-muted-foreground">Đang tải...</p>}
      {error && <p className="text-sm text-destructive">{error}</p>}
      {!loading && !error && conversations.length === 0 && (
        <p className="text-sm text-muted-foreground">Chưa có cuộc trò chuyện nào trên dataset này.</p>
      )}
      {conversations.length > 0 && (
        // Chỉ danh sách cuộn, nút "Cuộc trò chuyện mới" và nhãn ở trên đứng yên: danh sách dài thì vẫn tạo cuộc mới được
        // mà không phải cuộn ngược lên. Trong ngăn kéo của màn hẹp không có giới hạn chiều cao nên ngăn kéo cuộn cả khối.
        <ul className="-mr-1 flex min-h-0 flex-col gap-1 overflow-y-auto pr-1">
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
                    'flex w-full flex-col gap-0.5 rounded-md px-2 py-1.5 text-left text-[13px] leading-snug outline-none hover:bg-muted focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:opacity-60',
                    active && 'bg-secondary font-medium text-secondary-foreground',
                  )}
                >
                  <span className="line-clamp-2 wrap-break-word" title={item.title}>
                    {item.title}
                  </span>
                  <span className="text-[11px] text-muted-foreground">
                    {dateFormat.format(new Date(item.createdAt))}
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
