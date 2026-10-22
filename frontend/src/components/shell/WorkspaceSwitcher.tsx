// SPDX-License-Identifier: Apache-2.0
/* components/shell/WorkspaceSwitcher.tsx - nút đổi workspace ở đầu thanh bên, hiện workspace và vai đang dùng.
Vai gắn với từng workspace (docs/ARCHITECTURE.md mục 4b), nên đổi workspace thì vai hiện ra đổi theo; danh sách hiện vai
của từng workspace để người dùng biết trước mình sẽ làm được gì ở đó. Nút này không đổi vai của ai.
Mỗi mục là một link: workspace nằm trong đường dẫn, đổi workspace là chuyển trang, mở được trong tab mới. */

import { CheckIcon, ChevronsUpDownIcon } from 'lucide-react'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { cn } from '@/lib/utils'
import { roleLabel } from './labels'
import { menuItemFocus } from './menuItemFocus'
import type { LinkComponent, WorkspaceOption } from './types'

type Props = {
  current: WorkspaceOption
  workspaces: WorkspaceOption[]
  link: LinkComponent
  onNavigate?: () => void
}

export function WorkspaceSwitcher({ current, workspaces, link: Link, onNavigate }: Props) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        // Chữ trên nút chỉ là tên và vai; trình đọc màn hình cần biết thêm nút này để làm gì. Dùng aria-label thay cho
        // một span sr-only: Chrome chèn khoảng trắng quanh span đó, đọc thành "Admin , đổi workspace". Giữ tên và vai
        // đứng đầu theo đúng thứ tự đang hiện, để người dùng điều khiển bằng giọng nói gọi được nút.
        aria-label={`${current.name}, ${roleLabel(current.role)}, đổi workspace`}
        className={cn(
          'flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors',
          'hover:bg-sidebar-accent data-[state=open]:bg-sidebar-accent',
          'focus-visible:ring-2 focus-visible:ring-sidebar-ring focus-visible:outline-none',
        )}
      >
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="truncate text-sm font-semibold" title={current.name}>
            {current.name}
          </span>
          <span className="truncate text-xs text-muted-foreground">{roleLabel(current.role)}</span>
        </span>
        <ChevronsUpDownIcon aria-hidden="true" className="size-4 shrink-0 text-muted-foreground" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-(--radix-dropdown-menu-trigger-width) min-w-60">
        <DropdownMenuLabel className="text-xs font-normal text-muted-foreground">Workspace của bạn</DropdownMenuLabel>
        {workspaces.map((ws) => {
          const selected = ws.slug === current.slug
          return (
            <DropdownMenuItem key={ws.slug} asChild className={menuItemFocus}>
              <Link href={ws.href} aria-current={selected ? 'page' : undefined} onClick={onNavigate}>
                <span className="flex min-w-0 flex-1 flex-col">
                  <span className="truncate" title={ws.name}>
                    {ws.name}
                  </span>
                  <span className="truncate text-xs text-muted-foreground">{roleLabel(ws.role)}</span>
                </span>
                {selected && (
                  <>
                    <CheckIcon aria-hidden="true" className="size-4 shrink-0" />
                    <span className="sr-only">, đang mở</span>
                  </>
                )}
              </Link>
            </DropdownMenuItem>
          )
        })}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
