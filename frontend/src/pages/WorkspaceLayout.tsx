// SPDX-License-Identifier: Apache-2.0
/* pages/WorkspaceLayout.tsx - khung của mọi trang dưới /w/:ws. Lấy người dùng và workspace của họ, kiểm :ws là
workspace người đó thuộc về, rồi dựng AppShell; trang con hiện ở Outlet.
Workspace nằm trong đường dẫn, không lưu trong phiên (#42): mỗi tab trình duyệt làm việc ở workspace riêng. */

import { useEffect } from 'react'
import { Link, matchPath, Outlet, useLocation, useMatches, useNavigate, useParams } from 'react-router'
import { useLogout, useMe, type Membership } from '@/api/auth'
import { isUnauthorized } from '@/api/queryClient'
import { useVersion } from '@/api/system'
import { AppShell } from '@/components/shell/AppShell'
import type { WorkspaceOption } from '@/components/shell/types'
import { Button } from '@/components/ui/button'
import { RouterLink } from './RouterLink'
import { SessionStatus } from './SessionStatus'
import { SignedInScreen } from './SignedInScreen'
import { workspaceHref, workspaceNav } from './workspaceNav'

export function WorkspaceLayout() {
  const { ws } = useParams()
  const { pathname } = useLocation()
  const navigate = useNavigate()
  const me = useMe()
  const version = useVersion()
  const logout = useLogout()
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

  const current = me.data.workspaces.find((m) => m.slug === ws)
  if (!current) {
    // Không thuộc workspace và workspace không tồn tại báo cùng một câu, giống backend trả cùng một 404.
    const hasWorkspace = me.data.workspaces.length > 0
    return (
      <SignedInScreen
        user={me.data.user}
        title="Không mở được workspace này"
        description={
          'Workspace không tồn tại, hoặc bạn không phải thành viên. Nhờ admin của workspace thêm bạn bằng email.'
        }
        action={
          hasWorkspace && (
            <Button variant="outline" asChild>
              <Link to="/">Về workspace của bạn</Link>
            </Button>
          )
        }
      />
    )
  }

  const base = workspaceHref(current.slug, '')
  const navItems = workspaceNav.map((entry) => {
    const href = workspaceHref(current.slug, entry.path)
    // Trang đầu chỉ sáng khi đúng là trang đầu; mục khác sáng cả ở trang con, ví dụ datasets/<id>.
    const active = matchPath({ path: href, end: entry.path === '' }, pathname) !== null
    return { href, label: entry.label, active }
  })
  const activeEntry = workspaceNav.find((_, i) => navItems[i].active)

  // Đổi workspace thì giữ mục đang xem (Dữ liệu sang Dữ liệu), nhưng bỏ phần sau đó: id dataset của workspace này
  // không có ở workspace kia.
  const toOption = (m: Membership): WorkspaceOption => ({
    slug: m.slug,
    name: m.name,
    role: m.role,
    href: workspaceHref(m.slug, activeEntry?.path ?? ''),
  })

  const handleLogout = () => logout.mutate(undefined, { onSuccess: () => navigate('/', { replace: true }) })

  // Route con tự đặt tiêu đề qua handle (ví dụ trang không tồn tại); không có thì lấy tên mục điều hướng đang mở.
  const routeTitle = matches
    .map((m) => (m.handle as { title?: string } | undefined)?.title)
    .filter((title) => title !== undefined)
    .at(-1)
  const { user } = me.data

  return (
    <AppShell
      title={routeTitle ?? activeEntry?.label ?? current.name}
      navItems={navItems}
      currentWorkspace={{ ...toOption(current), href: base }}
      workspaces={me.data.workspaces.map(toOption)}
      user={{ name: user.display_name, email: user.email, isSystemAdmin: user.is_system_admin }}
      onLogout={handleLogout}
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
