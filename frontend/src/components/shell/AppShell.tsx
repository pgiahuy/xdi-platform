// SPDX-License-Identifier: Apache-2.0
/* components/shell/AppShell.tsx - bố cục chung của mọi trang trong một workspace: thanh bên, thanh trên, vùng nội dung.
Từ 1024px trở lên thanh bên đứng cố định bên trái. Hẹp hơn thì thanh bên ẩn vào ngăn kéo mở từ nút menu trên thanh
trên; cùng một nội dung thanh bên dùng cho cả hai chỗ, nên thêm mục điều hướng chỉ sửa một nơi.
Tiêu đề trang là h1 trên thanh trên; nội dung trang bắt đầu từ h2. */

import { useEffect, useState, type ReactNode } from 'react'
import { MenuIcon, XIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Sheet, SheetClose, SheetContent, SheetDescription, SheetTitle, SheetTrigger } from '@/components/ui/sheet'
import { ServerInfo } from './ServerInfo'
import { SidebarNav } from './SidebarNav'
import { UserMenu } from './UserMenu'
import { roleLabel } from './labels'
import { WorkspaceSwitcher } from './WorkspaceSwitcher'
import type { LinkComponent, NavItem, ServerInfo as ServerInfoData, SignedInUser, WorkspaceOption } from './types'

// Mốc lg của Tailwind, cùng mốc với các lớp lg: bên dưới. Đổi mốc thì đổi cả hai.
const WIDE_LAYOUT_QUERY = '(min-width: 1024px)'

type Props = {
  title: string
  navItems: NavItem[]
  currentWorkspace: WorkspaceOption
  workspaces: WorkspaceOption[]
  user: SignedInUser
  onLogout: () => void
  logoutPending: boolean
  /** Lỗi cần báo ngay dưới thanh trên, ví dụ đăng xuất không được. */
  notice?: string
  onDismissNotice?: () => void
  serverInfo?: ServerInfoData
  link: LinkComponent
  children: ReactNode
}

export function AppShell(props: Props) {
  const [drawerOpen, setDrawerOpen] = useState(false)
  const closeDrawer = () => setDrawerOpen(false)
  const { title, currentWorkspace } = props

  // Ngăn kéo đang mở mà kéo cửa sổ rộng ra thì thanh bên cố định hiện lên, ngăn kéo đè lên nó; đóng ngăn kéo lúc đó.
  useEffect(() => {
    const wide = window.matchMedia(WIDE_LAYOUT_QUERY)
    const closeWhenWide = (event: MediaQueryListEvent) => {
      if (event.matches) setDrawerOpen(false)
    }
    wide.addEventListener('change', closeWhenWide)
    return () => wide.removeEventListener('change', closeWhenWide)
  }, [])

  return (
    <div className="min-h-svh bg-background lg:grid lg:grid-cols-[16rem_minmax(0,1fr)]">
      {/* Mỗi tab một workspace (đường dẫn /w/<workspace>), nên tên tab ghi cả trang và workspace. */}
      <title>{title === currentWorkspace.name ? title : `${title} - ${currentWorkspace.name}`}</title>
      <a
        href="#main"
        className="sr-only rounded-md bg-background px-3 py-2 text-sm shadow-md focus:not-sr-only focus:fixed focus:top-3 focus:left-3 focus:z-50"
      >
        Bỏ qua điều hướng
      </a>

      <aside className="hidden border-r bg-sidebar text-sidebar-foreground lg:sticky lg:top-0 lg:flex lg:h-svh lg:flex-col">
        <SidebarContent {...props} />
      </aside>

      <div className="flex min-w-0 flex-col">
        <header className="sticky top-0 z-30 flex h-14 items-center gap-2 border-b bg-background px-4 lg:px-6">
          <Sheet open={drawerOpen} onOpenChange={setDrawerOpen}>
            <SheetTrigger asChild>
              <Button variant="ghost" size="icon" className="-ml-1.5 lg:hidden" aria-label="Mở menu">
                <MenuIcon aria-hidden="true" />
              </Button>
            </SheetTrigger>
            <SheetContent
              side="left"
              showCloseButton={false}
              className="w-72 max-w-[85vw] gap-0 bg-sidebar p-0 text-sidebar-foreground"
            >
              <SheetTitle className="sr-only">Điều hướng</SheetTitle>
              <SheetDescription className="sr-only">Đổi workspace và chọn trang</SheetDescription>
              <SidebarContent
                {...props}
                onNavigate={closeDrawer}
                closeButton={
                  <SheetClose asChild>
                    <Button variant="ghost" size="icon" aria-label="Đóng menu">
                      <XIcon aria-hidden="true" />
                    </Button>
                  </SheetClose>
                }
              />
            </SheetContent>
          </Sheet>

          <div className="min-w-0">
            <h1 className="truncate text-base font-semibold">{title}</h1>
            {/* Dưới lg thanh bên ẩn trong ngăn kéo; vẫn phải thấy mình đang ở workspace nào, với vai gì, trước khi
                làm việc ghi dữ liệu như nạp file. */}
            <p className="flex min-w-0 text-xs text-muted-foreground lg:hidden">
              {/* Tên dài thì chỉ cắt tên; vai luôn đọc được. */}
              <span className="truncate" title={currentWorkspace.name}>
                {currentWorkspace.name}
              </span>
              <span className="shrink-0">, {roleLabel(currentWorkspace.role)}</span>
            </p>
          </div>

          <div className="ml-auto">
            <UserMenu user={props.user} onLogout={props.onLogout} logoutPending={props.logoutPending} />
          </div>
        </header>

        {props.notice && (
          <div
            role="alert"
            className="flex items-center gap-2 border-b bg-destructive/10 py-1 pr-2 pl-4 text-sm text-destructive lg:pl-6"
          >
            <p className="min-w-0 flex-1">{props.notice}</p>
            {props.onDismissNotice && (
              <Button variant="ghost" size="icon-sm" aria-label="Đóng thông báo" onClick={props.onDismissNotice}>
                <XIcon aria-hidden="true" />
              </Button>
            )}
          </div>
        )}

        <main id="main" tabIndex={-1} className="flex-1 px-4 py-6 focus:outline-none lg:px-8">
          {props.children}
        </main>
      </div>
    </div>
  )
}

type SidebarProps = Props & {
  onNavigate?: () => void
  closeButton?: ReactNode
}

function SidebarContent(props: SidebarProps) {
  const { navItems, currentWorkspace, workspaces, serverInfo, link, onNavigate, closeButton } = props
  return (
    <div className="flex h-full flex-col">
      {/* Cùng chiều cao h-14 với thanh trên, để hai đường kẻ dưới nằm trên một hàng. */}
      <div className="flex h-14 shrink-0 items-center gap-1 border-b px-2">
        <div className="min-w-0 flex-1">
          <WorkspaceSwitcher current={currentWorkspace} workspaces={workspaces} link={link} onNavigate={onNavigate} />
        </div>
        {closeButton}
      </div>
      <div className="flex-1 overflow-y-auto p-2">
        <SidebarNav items={navItems} link={link} onNavigate={onNavigate} />
      </div>
      {serverInfo && (
        <div className="border-t px-4 py-3">
          <ServerInfo info={serverInfo} />
        </div>
      )}
    </div>
  )
}
