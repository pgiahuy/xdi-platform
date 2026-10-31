// SPDX-License-Identifier: Apache-2.0
/* components/shell/SidebarNav.tsx - danh sách mục điều hướng trong thanh bên.
Mỗi mục một icon Lucide trước chữ: đây là các nút chức năng, icon giúp quét nhanh (AGENTS.md mục 3, Icon và emoji).
Mục mang `section` thì vẽ nhãn nhóm khi nhóm đổi (ví dụ Workspace, rồi Quản trị hệ thống), giúp phân biệt mục của
workspace đang mở với mục của quản trị hệ thống.
Mục đang mở có vạch hổ phách bên trái và icon hổ phách: màu nhấn duy nhất của giao diện, nhìn là biết đang ở đâu; trình
đọc màn hình vẫn nghe "trang hiện tại" qua aria-current, không chỉ dựa vào màu.
Thanh bên thu gọn (compact): mỗi mục chỉ còn icon, tên mục ở aria-label và title (chú thích khi rê chuột); nhãn nhóm
thành một vạch ngăn.
leading chèn một phần đứng đầu danh sách, ngay dưới nhãn nhóm đầu tiên, ví dụ nút menu lịch sử trò chuyện
(SidebarContent.tsx). */

import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'
import type { LinkComponent, NavItem } from './types'

type Props = {
  items: NavItem[]
  link: LinkComponent
  /** Gọi khi bấm một mục; trên điện thoại dùng để đóng ngăn kéo. */
  onNavigate?: () => void
  compact?: boolean
  /** Phần đứng đầu danh sách, ngay dưới nhãn nhóm đầu tiên. */
  leading?: ReactNode
}

export function SidebarNav({ items, link: Link, onNavigate, compact = false, leading }: Props) {
  // Vùng điều hướng rỗng vẫn được trình đọc màn hình liệt kê như một landmark; không có mục thì không vẽ.
  if (items.length === 0) return null
  return (
    <nav aria-label="Điều hướng chính" className="flex flex-col gap-0.5">
      {items.map((item, index) => (
        <div key={item.href} className="contents">
          {compact && index > 0 && item.section !== items[index - 1]?.section && (
            <hr aria-hidden="true" className="mx-2 my-2 border-sidebar-border" />
          )}
          {!compact && item.section && item.section !== items[index - 1]?.section && (
            // Mỗi nhãn nằm trong một div riêng nên first: không phân biệt được nhãn đầu; dùng index.
            <p
              className={cn(
                'px-2.5 pb-2 text-[11px] font-medium tracking-wider text-sidebar-muted uppercase',
                index === 0 ? 'pt-2' : 'pt-5',
              )}
            >
              {item.section}
            </p>
          )}
          {index === 0 && leading}
          <Link
            href={item.href}
            aria-current={item.active ? 'page' : undefined}
            aria-label={compact ? item.label : undefined}
            title={compact ? item.label : undefined}
            onClick={onNavigate}
            className={cn(
              'relative flex items-center gap-2.5 rounded-lg px-2.5 py-2 text-sm font-medium transition-colors',
              compact && 'justify-center',
              // Thu gọn thì vạch hổ phách nằm sát mép trái của dải icon (px-2 thay cho px-3 của thanh bên rộng).
              compact && item.active && 'before:-left-2',
              'focus-visible:ring-2 focus-visible:ring-sidebar-ring focus-visible:outline-none',
              item.active
                ? 'bg-sidebar-accent text-sidebar-accent-foreground before:absolute before:inset-y-2 before:-left-3 before:w-[3px] before:rounded-r-sm before:bg-sidebar-primary [&>svg]:text-sidebar-primary'
                : 'text-sidebar-muted hover:bg-sidebar-raised hover:text-sidebar-foreground',
            )}
          >
            {item.icon ? (
              <item.icon className="size-4 shrink-0" aria-hidden="true" />
            ) : (
              compact && <span aria-hidden="true">{item.label.charAt(0)}</span>
            )}
            {!compact && item.label}
          </Link>
        </div>
      ))}
    </nav>
  )
}
