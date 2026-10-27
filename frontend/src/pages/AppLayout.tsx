// SPDX-License-Identifier: Apache-2.0
/* pages/AppLayout.tsx - khung của mọi trang sau khi đăng nhập: trang trong workspace (/w/:ws, workspaceNav.ts) và trang
ngoài workspace (quản trị hệ thống, tài khoản; globalNav.ts). Lấy người dùng và workspace của họ rồi dựng AppShell; trang
con hiện ở Outlet. Có :ws thì kiểm người dùng thuộc workspace đó; không có thì dựng khung không có workspace đang mở,
nên quản trị hệ thống chưa thuộc workspace nào vẫn vào được trang quản trị (#99).
Một khung cho cả hai nhóm trang: đi từ workspace sang trang quản trị và ngược lại thì AppShell không bị gỡ rồi dựng lại,
focus bàn phím ở lại mục vừa bấm trên thanh bên. Hai khung riêng thì mỗi lần chuyển focus rơi về đầu trang.
Workspace nằm trong đường dẫn, không lưu trong phiên (#42): mỗi tab trình duyệt làm việc ở workspace riêng. */

import { useEffect } from 'react'
import { Link, matchPath, Outlet, useLocation, useMatches, useParams } from 'react-router'
import { useLogout, useMe, type Membership } from '@/api/auth'
import { isUnauthorized } from '@/api/queryClient'
import { useVersion } from '@/api/system'
import { AppShell } from '@/components/shell/AppShell'
import type { NavItem, WorkspaceOption } from '@/components/shell/types'
import { Button } from '@/components/ui/button'
import { signOutTo } from './authNav'
import { accountRoute, adminNav } from './globalNav'
import { RouterLink } from './RouterLink'
import { SessionStatus } from './SessionStatus'
import { SignedInScreen } from './SignedInScreen'
import { accountPath, workspaceHref, workspaceNav } from './workspaceNav'

export function AppLayout() {
  const { ws } = useParams()
  const { pathname } = useLocation()
  const me = useMe()
  const version = useVersion()
  const logout = useLogout(signOutTo)
  const matches = useMatches()

  // Thông báo đăng xuất lỗi thuộc về lúc bấm; chuyển trang thì bỏ, không để nó nằm lại mãi dưới thanh trên.
  const resetLogout = logout.reset
  useEffect(() => {
    resetLogout()
  }, [pathname, resetLogout])

  // Kiểm 401 trước: phiên bị thu hồi thì bộ đệm vẫn còn người dùng cũ, chỉ nhìn data sẽ tưởng còn đăng nhập.
  if (!me.data || isUnauthorized(me.error)) {
    return <SessionStatus error={me.error} onRetry={() => me.refetch()} />
  }

  const { user, workspaces } = me.data
  // ws chỉ có ở trang dưới /w/:ws; trang quản trị và /account không có.
  const current = ws === undefined ? undefined : workspaces.find((m) => m.slug === ws)
  if (ws !== undefined && !current) {
    // Không thuộc workspace và workspace không tồn tại báo cùng một câu, giống backend trả cùng một 404.
    const hasWorkspace = workspaces.length > 0
    // Quản trị hệ thống chưa thuộc workspace nào cũng cần nút: trang gốc có lối vào trang quản trị cho họ.
    const showHome = hasWorkspace || user.is_system_admin
    return (
      <SignedInScreen
        user={user}
        title="Không mở được workspace này"
        description={
          'Workspace không tồn tại, hoặc bạn không phải thành viên. Nhờ admin của workspace thêm bạn bằng email.'
        }
        action={
          showHome && (
            <Button variant="outline" asChild>
              <Link to="/">{hasWorkspace ? 'Về workspace của bạn' : 'Về trang đầu'}</Link>
            </Button>
          )
        }
      />
    )
  }

  // Trang đầu của workspace chỉ sáng khi đúng là trang đầu; mục khác sáng cả ở trang con, ví dụ datasets/<id>.
  const workspaceItems = current
    ? workspaceNav.map((entry) => ({
        ...buildNavItem(workspaceHref(current.slug, entry.path), entry.label, entry.path === '', pathname),
        icon: entry.icon,
      }))
    : []
  const activeEntry = workspaceNav.find((_, i) => workspaceItems[i]?.active)
  // Ngoài workspace, người chưa thuộc workspace nào không có nút đổi workspace để quay về (ví dụ gõ thẳng /account):
  // cho họ một mục về trang đầu, nơi giải thích phải làm gì.
  const homeItems = !current && workspaces.length === 0 ? [buildNavItem('/', 'Trang đầu', true, pathname)] : []
  // Mục quản trị hệ thống đứng cuối thanh bên, cả trong và ngoài workspace. Ẩn với người khác chỉ để gọn; quyền vẫn do
  // backend kiểm.
  const adminItems = user.is_system_admin
    ? adminNav.map((entry) => ({ ...buildNavItem(entry.href, entry.label, false, pathname), icon: entry.icon }))
    : []

  // Đổi workspace thì giữ mục đang xem (Dữ liệu sang Dữ liệu), nhưng bỏ phần sau đó: id dataset của workspace này
  // không có ở workspace kia. Từ trang ngoài workspace thì về trang đầu của workspace được chọn.
  const toOption = (m: Membership): WorkspaceOption => ({
    slug: m.slug,
    name: m.name,
    role: m.role,
    href: workspaceHref(m.slug, activeEntry?.path ?? ''),
  })

  // Route con tự đặt tiêu đề qua handle (ví dụ trang không tồn tại); không có thì lấy tên mục điều hướng đang mở.
  const routeTitle = matches
    .map((m) => (m.handle as { title?: string } | undefined)?.title)
    .filter((title) => title !== undefined)
    .at(-1)

  return (
    <AppShell
      title={routeTitle ?? activeEntry?.label ?? current?.name ?? ''}
      navItems={[...homeItems, ...workspaceItems, ...adminItems]}
      currentWorkspace={current && { ...toOption(current), href: workspaceHref(current.slug, '') }}
      workspaces={workspaces.map(toOption)}
      user={{ name: user.display_name, email: user.email, isSystemAdmin: user.is_system_admin }}
      accountHref={current ? workspaceHref(current.slug, accountPath) : accountRoute}
      onLogout={() => logout.mutate()}
      logoutPending={logout.isPending}
      notice={logout.error ? `Đăng xuất không được: ${logout.error.message}` : undefined}
      onDismissNotice={resetLogout}
      serverInfo={version.data}
      link={RouterLink}
    >
      <Outlet />
    </AppShell>
  )
}

/** Một mục thanh bên; exact = true thì chỉ sáng khi đúng đường dẫn đó, không sáng ở trang con. */
function buildNavItem(href: string, label: string, exact: boolean, pathname: string): NavItem {
  return { href, label, active: matchPath({ path: href, end: exact }, pathname) !== null }
}
