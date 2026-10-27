// SPDX-License-Identifier: Apache-2.0
/* components/shell/UserMenu.tsx - nút tài khoản ở góc phải thanh trên: tên, email, trang tài khoản, đăng xuất.
Trên điện thoại chỉ còn ô chữ viết tắt, nên nút có aria-label đủ tên người dùng. */

import { useState } from 'react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { cn } from '@/lib/utils'
import { initials } from './labels'
import { menuItemFocus } from './menuItemFocus'
import type { LinkComponent, SignedInUser } from './types'

type Props = {
  user: SignedInUser
  /** Trang tài khoản (đổi mật khẩu). */
  accountHref: string
  link: LinkComponent
  onLogout: () => void
  logoutPending: boolean
  /** wide: nút full-width cho chân thanh bên (tên và email xếp dọc); mặc định là nút gọn trên thanh trên. */
  wide?: boolean
  /** Thanh bên thu gọn: nút ở chân thanh bên chỉ còn ô chữ đầu; tên đọc qua aria-label và title. */
  compact?: boolean
}

export function UserMenu({ user, accountHref, link: Link, onLogout, logoutPending, wide, compact = false }: Props) {
  const name = user.name || user.email
  const [open, setOpen] = useState(false)

  // Radix đóng menu ngay khi chọn, nên chữ "Đang đăng xuất..." không kịp hiện; menu vẫn mở trong lúc đăng xuất, xong
  // thì đóng. Không giữ mở sau đó: menu mở thì phần còn lại của trang bị ẩn với trình đọc màn hình, nên thông báo đăng
  // xuất lỗi dưới thanh trên sẽ không được đọc.
  return (
    <DropdownMenu open={open || logoutPending} onOpenChange={setOpen}>
      <DropdownMenuTrigger asChild>
        {wide ? (
          <button
            type="button"
            aria-label={`Tài khoản ${name}`}
            title={compact ? name : undefined}
            className="flex w-full items-center gap-2.5 rounded-lg px-2 py-1.5 text-left transition-colors hover:bg-sidebar-raised data-[state=open]:bg-sidebar-raised focus-visible:ring-2 focus-visible:ring-sidebar-ring focus-visible:outline-none"
          >
            <span
              aria-hidden="true"
              className="flex size-8 shrink-0 items-center justify-center rounded-full bg-sidebar-accent text-xs font-semibold text-sidebar-foreground"
            >
              {initials(user.name, user.email)}
            </span>
            <span className={cn('flex min-w-0 flex-1 flex-col', compact && 'hidden')}>
              <span className="truncate text-sm font-medium">{name}</span>
              <span className="truncate text-xs text-sidebar-muted">{user.email}</span>
            </span>
          </button>
        ) : (
          <Button variant="ghost" className="h-9 gap-2 px-1.5 sm:pr-2.5" aria-label={`Tài khoản ${name}`}>
            <span
              aria-hidden="true"
              className="flex size-7 items-center justify-center rounded-full bg-muted text-xs font-medium text-foreground"
            >
              {initials(user.name, user.email)}
            </span>
            <span className="hidden max-w-40 truncate text-sm sm:inline">{name}</span>
          </Button>
        )}
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-64">
        <DropdownMenuLabel className="flex flex-col gap-0.5 font-normal">
          <span className="truncate text-sm font-medium text-foreground" title={name}>
            {name}
          </span>
          <span className="truncate text-xs text-muted-foreground" title={user.email}>
            {user.email}
          </span>
          {user.isSystemAdmin && <span className="mt-1 text-xs text-muted-foreground">Quản trị hệ thống</span>}
        </DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuItem asChild className={menuItemFocus}>
          <Link href={accountHref}>Tài khoản và mật khẩu</Link>
        </DropdownMenuItem>
        <DropdownMenuItem className={menuItemFocus} disabled={logoutPending} onSelect={onLogout}>
          {logoutPending ? 'Đang đăng xuất...' : 'Đăng xuất'}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
