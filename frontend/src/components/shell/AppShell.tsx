// SPDX-License-Identifier: Apache-2.0
/* components/shell/AppShell.tsx - bố cục chung của mọi trang sau khi đăng nhập: thanh bên (có hàng thương hiệu),
thanh trên nền trắng, vùng nội dung nền sáng, rộng hết phần còn lại của màn. Thanh bên nền mực (màu sidebar trong index.css). Màn rộng: tài khoản và phiên bản ở chân thanh bên; màn hẹp: tài khoản ở góc thanh trên.
Trang trong workspace truyền workspace đang mở; trang ngoài workspace (quản trị hệ thống, tài khoản) thì không.
Từ 1024px trở lên thanh bên đứng cố định bên trái. Hẹp hơn thì thanh bên ẩn vào ngăn kéo mở từ nút menu trên thanh
trên; cùng một nội dung thanh bên dùng cho cả hai chỗ, nên thêm mục điều hướng chỉ sửa một nơi.
Tiêu đề trang là h1 trên thanh trên; nội dung trang bắt đầu từ h2.
Màn rộng thu gọn được thanh bên thành dải icon (nút ở chân thanh bên); trang giữ trạng thái và nhớ qua các lần tải
trang. Ngăn kéo của màn hẹp luôn là thanh bên đầy đủ.
Hộp tạo workspace đặt ở đây, một lần, ngoài thanh bên: mở từ ngăn kéo thì ngăn kéo đóng lại trước, và hộp không bị gỡ
theo ngăn kéo. */

import { useEffect, useState, type ReactNode } from 'react'
import { MenuIcon, XIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { Sheet, SheetClose, SheetContent, SheetDescription, SheetTitle, SheetTrigger } from '@/components/ui/sheet'
import { SidebarContent } from './SidebarContent'
import { UserMenu } from './UserMenu'
import { roleLabel } from './labels'
import { CreateWorkspaceDialog } from './CreateWorkspaceDialog'
import type { LinkComponent, NavItem, ServerInfo as ServerInfoData, SignedInUser, WorkspaceOption } from './types'

// Mốc lg của Tailwind, cùng mốc với các lớp lg: bên dưới. Đổi mốc thì đổi cả hai.
const WIDE_LAYOUT_QUERY = '(min-width: 1024px)'

type Props = {
  title: string
  navItems: NavItem[]
  /** Không có khi trang không thuộc workspace nào. */
  currentWorkspace?: WorkspaceOption
  workspaces: WorkspaceOption[]
  user: SignedInUser
  accountHref: string
  onLogout: () => void
  logoutPending: boolean
  /** Lỗi cần báo ngay dưới thanh trên, ví dụ đăng xuất không được. */
  notice?: string
  onDismissNotice?: () => void
  serverInfo?: ServerInfoData
  /** Hộp tạo workspace; trang giữ trạng thái mở để đóng hộp khi tạo xong. Không có thì menu không có mục tạo. */
  createWorkspace?: CreateWorkspaceControl
  /** Thanh bên màn rộng đang thu gọn thành dải icon. */
  sidebarCollapsed?: boolean
  onSidebarCollapsedChange?: (collapsed: boolean) => void
  link: LinkComponent
  children: ReactNode
}

export type CreateWorkspaceControl = {
  open: boolean
  onOpenChange: (open: boolean) => void
  onCreate: (name: string) => void
  pending: boolean
  error?: string
}

export function AppShell(props: Props) {
  const [drawerOpen, setDrawerOpen] = useState(false)
  const closeDrawer = () => setDrawerOpen(false)
  const { title, currentWorkspace, createWorkspace, sidebarCollapsed = false } = props
  const openCreateWorkspace = createWorkspace
    ? () => {
        setDrawerOpen(false)
        createWorkspace.onOpenChange(true)
      }
    : undefined

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
    <div
      className={cn(
        'min-h-svh bg-background lg:grid',
        sidebarCollapsed ? 'lg:grid-cols-[4rem_minmax(0,1fr)]' : 'lg:grid-cols-[16rem_minmax(0,1fr)]',
      )}
    >
      {/* Mỗi tab một workspace (đường dẫn /w/<workspace>), nên tên tab ghi cả trang và workspace. */}
      <title>
        {!currentWorkspace || title === currentWorkspace.name ? title : `${title} - ${currentWorkspace.name}`}
      </title>
      <a
        href="#main"
        className="sr-only rounded-md bg-background px-3 py-2 text-sm shadow-md focus:not-sr-only focus:fixed focus:top-3 focus:left-3 focus:z-50"
      >
        Bỏ qua điều hướng
      </a>

      <aside className="hidden bg-sidebar text-sidebar-foreground lg:sticky lg:top-0 lg:flex lg:h-svh lg:flex-col">
        <SidebarContent
          {...props}
          compact={sidebarCollapsed}
          onCreateWorkspace={openCreateWorkspace}
          onToggleCompact={
            props.onSidebarCollapsedChange && (() => props.onSidebarCollapsedChange?.(!sidebarCollapsed))
          }
        />
      </aside>

      <div className="flex min-w-0 flex-col">
        <header className="sticky top-0 z-30 flex h-16 items-center gap-2 border-b bg-card px-4 lg:px-8">
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
                onCreateWorkspace={openCreateWorkspace}
                closeButton={
                  <SheetClose asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      aria-label="Đóng menu"
                      className="text-sidebar-foreground hover:bg-sidebar-raised hover:text-sidebar-foreground focus-visible:ring-sidebar-ring"
                    >
                      <XIcon aria-hidden="true" />
                    </Button>
                  </SheetClose>
                }
              />
            </SheetContent>
          </Sheet>

          <div className="min-w-0">
            <h1 className="truncate text-lg font-semibold tracking-tight">{title}</h1>
            {/* Dưới lg thanh bên ẩn trong ngăn kéo; vẫn phải thấy mình đang ở workspace nào, với vai gì, trước khi
                làm việc ghi dữ liệu như nạp file. */}
            {currentWorkspace && (
              <p className="flex min-w-0 text-xs text-muted-foreground lg:hidden">
                {/* Tên dài thì chỉ cắt tên; vai luôn đọc được. */}
                <span className="truncate" title={currentWorkspace.name}>
                  {currentWorkspace.name}
                </span>
                <span className="shrink-0">, {roleLabel(currentWorkspace.role)}</span>
              </p>
            )}
          </div>

          <div className="ml-auto lg:hidden">
            <UserMenu
              user={props.user}
              accountHref={props.accountHref}
              link={props.link}
              onLogout={props.onLogout}
              logoutPending={props.logoutPending}
            />
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

        <main id="main" tabIndex={-1} className="flex-1 bg-background focus:outline-none">
          {/* Nội dung rộng hết vùng còn lại: trang hỏi có ba cột (danh sách, khung chat, minh chứng), giới hạn bề rộng thì
          màn rộng bỏ trống hai bên còn các cột lại chật. */}
          <div className="w-full px-4 py-6 lg:px-8">{props.children}</div>
        </main>
      </div>

      {createWorkspace && <CreateWorkspaceDialog {...createWorkspace} />}
    </div>
  )
}
