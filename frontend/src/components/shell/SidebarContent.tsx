// SPDX-License-Identifier: Apache-2.0
/* components/shell/SidebarContent.tsx - nội dung thanh bên: hàng thương hiệu, bộ chọn workspace, mục điều hướng, chân
có nút thu gọn, tài khoản và phiên bản. AppShell dùng cùng nội dung này cho thanh bên cố định của màn rộng và cho ngăn
kéo của màn hẹp. compact chỉ dùng cho thanh bên cố định: dải icon, chữ chuyển vào aria-label và title. */

import type { ReactNode } from 'react'
import { PanelLeftClose, PanelLeftOpen } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { ServerInfo } from './ServerInfo'
import { SidebarNav } from './SidebarNav'
import { UserMenu } from './UserMenu'
import { WorkspaceSwitcher } from './WorkspaceSwitcher'
import type { LinkComponent, NavItem, ServerInfo as ServerInfoData, SignedInUser, WorkspaceOption } from './types'

type Props = {
  navItems: NavItem[]
  currentWorkspace?: WorkspaceOption
  workspaces: WorkspaceOption[]
  user: SignedInUser
  accountHref: string
  onLogout: () => void
  logoutPending: boolean
  serverInfo?: ServerInfoData
  link: LinkComponent
  onNavigate?: () => void
  onCreateWorkspace?: () => void
  closeButton?: ReactNode
  /** Dải icon: chỉ thanh bên cố định của màn rộng, không bao giờ là ngăn kéo. */
  compact?: boolean
  /** Nút thu gọn hay mở rộng ở chân thanh bên; không có thì không có nút. */
  onToggleCompact?: () => void
}

export function SidebarContent(props: Props) {
  const { navItems, currentWorkspace, workspaces, serverInfo, link, onNavigate, onCreateWorkspace, closeButton } = props
  const { compact = false, onToggleCompact } = props
  // Nhãn đổi theo trạng thái nên không thêm aria-expanded: biến thể ghost tô nền khi aria-expanded, nút sẽ sáng bừng
  // trên nền mực của thanh bên.
  const toggleLabel = compact ? 'Mở rộng thanh bên' : 'Thu gọn thanh bên'
  return (
    <div className="flex h-full flex-col">
      {/* Hàng thương hiệu: cùng dấu hiệu với màn đăng nhập (favicon.svg). */}
      <div className={cn('flex h-16 shrink-0 items-center gap-2.5', compact ? 'justify-center px-2' : 'px-5')}>
        <img src="/favicon.svg" alt={compact ? 'xdi-platform' : ''} className="size-7" />
        {!compact && <span className="text-[15px] font-semibold tracking-tight">xdi-platform</span>}
      </div>
      <div className={cn('flex shrink-0 items-center gap-1 pb-2', compact ? 'px-2' : 'px-3')}>
        <div className="min-w-0 flex-1">
          <WorkspaceSwitcher
            current={currentWorkspace}
            workspaces={workspaces}
            link={link}
            onNavigate={onNavigate}
            onCreateWorkspace={onCreateWorkspace}
            compact={compact}
          />
        </div>
        {closeButton}
      </div>
      <div className={cn('flex-1 overflow-y-auto py-2', compact ? 'px-2' : 'px-3')}>
        <SidebarNav items={navItems} link={link} onNavigate={onNavigate} compact={compact} />
      </div>
      <div className={cn('border-t border-sidebar-border py-3', compact ? 'px-2' : 'px-3')}>
        {onToggleCompact && (
          <Button
            variant="ghost"
            size="sm"
            aria-label={toggleLabel}
            title={toggleLabel}
            onClick={onToggleCompact}
            className={cn(
              'mb-2 w-full text-sidebar-muted hover:bg-sidebar-raised hover:text-sidebar-foreground focus-visible:ring-sidebar-ring',
              compact ? 'justify-center px-0' : 'justify-start',
            )}
          >
            {compact ? <PanelLeftOpen aria-hidden="true" /> : <PanelLeftClose aria-hidden="true" />}
            {!compact && 'Thu gọn'}
          </Button>
        )}
        <UserMenu
          wide
          compact={compact}
          user={props.user}
          accountHref={props.accountHref}
          link={link}
          onLogout={props.onLogout}
          logoutPending={props.logoutPending}
        />
        {serverInfo && !compact && (
          <div className="px-2 pt-2 pb-0.5 text-sidebar-muted">
            <ServerInfo info={serverInfo} />
          </div>
        )}
      </div>
    </div>
  )
}
