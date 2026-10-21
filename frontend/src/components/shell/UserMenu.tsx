// SPDX-License-Identifier: Apache-2.0
/* components/shell/UserMenu.tsx - nút tài khoản ở góc phải thanh trên: tên, email, đăng xuất.
Trên điện thoại chỉ còn ô chữ viết tắt, nên nút có aria-label đủ tên người dùng. */

import type { User } from '@/api/auth'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { initials } from './labels'

type Props = {
  user: User
  onLogout: () => void
  logoutPending: boolean
}

export function UserMenu({ user, onLogout, logoutPending }: Props) {
  const name = user.display_name || user.email
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" className="h-9 gap-2 px-1.5 sm:pr-2.5" aria-label={`Tài khoản ${name}`}>
          <span
            aria-hidden="true"
            className="flex size-7 items-center justify-center rounded-full bg-muted text-xs font-medium text-foreground"
          >
            {initials(user.display_name, user.email)}
          </span>
          <span className="hidden max-w-40 truncate text-sm sm:inline">{name}</span>
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-64">
        <DropdownMenuLabel className="flex flex-col gap-0.5 font-normal">
          <span className="truncate text-sm font-medium text-foreground">{name}</span>
          <span className="truncate text-xs text-muted-foreground">{user.email}</span>
          {user.is_system_admin && <span className="mt-1 text-xs text-muted-foreground">Quản trị hệ thống</span>}
        </DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuItem disabled={logoutPending} onSelect={onLogout}>
          {logoutPending ? 'Đang đăng xuất...' : 'Đăng xuất'}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
