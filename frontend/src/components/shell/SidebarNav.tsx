// SPDX-License-Identifier: Apache-2.0
/* components/shell/SidebarNav.tsx - danh sách mục điều hướng trong thanh bên.
Mục chỉ có chữ, không icon: danh sách ngắn, chữ đọc nhanh hơn icon (AGENTS.md mục 3, Icon và emoji). */

import { cn } from '@/lib/utils'
import type { LinkComponent, NavItem } from './types'

type Props = {
  items: NavItem[]
  link: LinkComponent
  /** Gọi khi bấm một mục; trên điện thoại dùng để đóng ngăn kéo. */
  onNavigate?: () => void
}

export function SidebarNav({ items, link: Link, onNavigate }: Props) {
  return (
    <nav aria-label="Điều hướng chính" className="flex flex-col gap-0.5">
      {items.map((item) => (
        <Link
          key={item.href}
          href={item.href}
          aria-current={item.active ? 'page' : undefined}
          onClick={onNavigate}
          className={cn(
            'rounded-md px-2.5 py-1.5 text-sm font-medium transition-colors',
            'focus-visible:ring-2 focus-visible:ring-sidebar-ring focus-visible:outline-none',
            item.active
              ? 'bg-sidebar-accent text-sidebar-accent-foreground'
              : 'text-muted-foreground hover:bg-sidebar-accent/60 hover:text-sidebar-foreground',
          )}
        >
          {item.label}
        </Link>
      ))}
    </nav>
  )
}
