// SPDX-License-Identifier: Apache-2.0
/* components/shell/ConversationFlyout.tsx - nút "Lịch sử trò chuyện" trên thanh bên và khung nổi của nó (Popover của
Radix, components/ui/popover.tsx). Khung nằm trong portal nên đè lên trang đang mở, không đẩy bố cục. Thanh bên cố
định: khung bung sang phải, ra ngoài mép thanh bên. Ngăn kéo của màn hẹp: khung mở xuống dưới nút (Radix lật lên trên
khi bên dưới thiếu chỗ), vì bên phải ngăn kéo không đủ chỗ cho khung rộng 18rem.
Khung đóng khi bấm ra ngoài hay nhấn Esc (Radix lo), và khi trang gọi close sau khi người dùng chọn một mục. modal như
menu thả xuống: tiêu điểm ở trong khung cho tới khi đóng, bấm ra ngoài chỉ đóng khung, không bấm luôn vào chỗ đó.
Nội dung do trang dựng (render) vì nó gọi API; Radix chỉ gắn nội dung khi khung mở, nên trang chỉ tải danh sách khi
người dùng mở menu. Mỗi chỗ đặt (thanh bên cố định, ngăn kéo) giữ trạng thái mở riêng, để hai khung không cùng mở. */

import { useState } from 'react'
import { History } from 'lucide-react'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { cn } from '@/lib/utils'
import type { ConversationMenu } from './types'

const label = 'Lịch sử trò chuyện'

type Props = {
  render: ConversationMenu['render']
  /** Thanh bên thu gọn: nút chỉ còn icon, tên ở aria-label và title. */
  compact?: boolean
  /** Nút nằm trong ngăn kéo của màn hẹp. */
  drawer?: boolean
  /** Gọi khi chọn xong một mục; trong ngăn kéo dùng để đóng ngăn kéo. */
  onNavigate?: () => void
}

export function ConversationFlyout({ render, compact = false, drawer = false, onNavigate }: Props) {
  const [open, setOpen] = useState(false)
  const close = () => {
    setOpen(false)
    onNavigate?.()
  }
  return (
    <Popover open={open} onOpenChange={setOpen} modal>
      <PopoverTrigger
        aria-label={compact ? label : undefined}
        title={compact ? label : undefined}
        // Cùng dáng mục điều hướng (SidebarNav.tsx) để đọc ra là một mục của thanh bên; khung đang mở thì nền nổi như
        // lúc rê chuột.
        className={cn(
          'flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-sm font-medium text-sidebar-muted transition-colors',
          compact && 'justify-center',
          'hover:bg-sidebar-raised hover:text-sidebar-foreground',
          'data-[state=open]:bg-sidebar-raised data-[state=open]:text-sidebar-foreground',
          'focus-visible:ring-2 focus-visible:ring-sidebar-ring focus-visible:outline-none',
        )}
      >
        <History className="size-4 shrink-0" aria-hidden="true" />
        {!compact && label}
      </PopoverTrigger>
      <PopoverContent
        aria-label={label}
        side={drawer ? 'bottom' : 'right'}
        align="start"
        // Thanh bên cố định: nút cách mép phải thanh bên 12px (thu gọn: 8px), thêm 8px để khung đứng hẳn ra ngoài.
        sideOffset={drawer ? 4 : compact ? 16 : 20}
        // Màn thấp thì khung không cao quá chỗ còn lại; danh sách bên trong co lại và cuộn. Khung đè lên trang nên cần
        // viền và bóng đậm hơn mặc định của shadcn (ring mảnh, shadow-md), không thì lẫn vào nền trắng phía sau.
        className="max-h-(--radix-popover-content-available-height) w-80 rounded-xl border border-border p-3 shadow-xl ring-0"
      >
        {render(close)}
      </PopoverContent>
    </Popover>
  )
}
