// SPDX-License-Identifier: Apache-2.0
/* components/shell/WorkspaceSwitcher.tsx - nút đổi workspace ở đầu thanh bên, hiện workspace và vai đang dùng.
Vai gắn với từng workspace (docs/ARCHITECTURE.md mục 4b), nên đổi workspace thì vai hiện ra đổi theo; danh sách hiện vai
của từng workspace để người dùng biết trước mình sẽ làm được gì ở đó. Nút này không đổi vai của ai.
Mỗi mục là một link: workspace nằm trong đường dẫn, đổi workspace là chuyển trang, mở được trong tab mới.
Ở trang ngoài workspace (quản trị hệ thống, tài khoản) không có workspace đang mở: nút thành "Chọn workspace", và người
chưa thuộc workspace nào thấy một dòng chữ thay cho nút, vì không có gì để chọn.
Mục cuối "Tạo workspace" chỉ báo cho trang qua onCreateWorkspace; hộp thoại nằm ngoài menu (AppShell), vì menu đóng
lại ngay khi chọn mục và gỡ mọi thứ bên trong nó. */

import { CheckIcon, ChevronsUpDownIcon, PlusIcon } from 'lucide-react'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { cn } from '@/lib/utils'
import { roleLabel } from './labels'
import { menuItemFocus } from './menuItemFocus'
import type { LinkComponent, WorkspaceOption } from './types'

type Props = {
  current?: WorkspaceOption
  workspaces: WorkspaceOption[]
  link: LinkComponent
  onNavigate?: () => void
  /** Không có thì menu không có mục tạo workspace. */
  onCreateWorkspace?: () => void
  /** Thanh bên thu gọn: nút chỉ còn ô chữ đầu của workspace; tên và vai đọc qua aria-label và title. */
  compact?: boolean
}

export function WorkspaceSwitcher(props: Props) {
  const { current, workspaces, link: Link, onNavigate, onCreateWorkspace, compact = false } = props
  if (workspaces.length === 0) {
    return compact ? null : <p className="truncate px-2 py-1.5 text-sm text-sidebar-muted">Chưa thuộc workspace nào</p>
  }
  const title = current?.name ?? 'Chọn workspace'
  const subtitle = current ? roleLabel(current.role) : 'Trang không thuộc workspace'
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        // Chữ trên nút chỉ là tên và vai; trình đọc màn hình cần biết thêm nút này để làm gì. Dùng aria-label thay cho
        // một span sr-only: Chrome chèn khoảng trắng quanh span đó, đọc thành "Admin , đổi workspace". Giữ tên và vai
        // đứng đầu theo đúng thứ tự đang hiện, để người dùng điều khiển bằng giọng nói gọi được nút.
        aria-label={current ? `${current.name}, ${roleLabel(current.role)}, đổi workspace` : title}
        title={compact ? title : undefined}
        className={cn(
          // Khung có viền và nền riêng để đọc ra đây là bộ chọn workspace, không phải chữ tĩnh.
          'flex w-full items-center gap-2.5 rounded-[10px] border border-sidebar-border bg-sidebar-raised px-2.5 py-2 text-left transition-colors',
          'hover:bg-sidebar-accent data-[state=open]:bg-sidebar-accent',
          'focus-visible:ring-2 focus-visible:ring-sidebar-ring focus-visible:outline-none',
          compact && 'justify-center px-1.5',
        )}
      >
        {/* Ô chữ đầu của tên workspace, giúp nhận ra workspace khi có nhiều cái. */}
        {(current || compact) && (
          <span
            aria-hidden="true"
            className="flex size-7 shrink-0 items-center justify-center rounded-md bg-label text-[13px] font-semibold text-sidebar-foreground"
          >
            {current ? current.name.charAt(0).toUpperCase() : <ChevronsUpDownIcon className="size-4" />}
          </span>
        )}
        <span className={cn('flex min-w-0 flex-1 flex-col leading-tight', compact && 'hidden')}>
          <span className="truncate text-sm font-semibold" title={title}>
            {title}
          </span>
          <span className="truncate text-xs text-sidebar-muted">{subtitle}</span>
        </span>
        {!compact && <ChevronsUpDownIcon aria-hidden="true" className="size-4 shrink-0 text-sidebar-muted" />}
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-(--radix-dropdown-menu-trigger-width) min-w-60">
        <DropdownMenuLabel className="text-xs font-normal text-muted-foreground">Workspace của bạn</DropdownMenuLabel>
        {workspaces.map((ws) => {
          const selected = ws.slug === current?.slug
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
        {onCreateWorkspace && (
          <>
            <DropdownMenuSeparator />
            <DropdownMenuItem className={menuItemFocus} onSelect={onCreateWorkspace}>
              <PlusIcon aria-hidden="true" className="size-4 shrink-0" />
              Tạo workspace
            </DropdownMenuItem>
          </>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
