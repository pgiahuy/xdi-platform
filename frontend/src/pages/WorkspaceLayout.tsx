// SPDX-License-Identifier: Apache-2.0
/* pages/WorkspaceLayout.tsx - khung của mọi trang dưới /w/:ws. Lấy người dùng và workspace của họ, kiểm :ws là
workspace người đó thuộc về, rồi dựng AppShell; trang con hiện ở Outlet.
Workspace nằm trong đường dẫn, không lưu trong phiên (#42): mỗi tab trình duyệt làm việc ở workspace riêng. */

import { Link, matchPath, Outlet, useLocation, useNavigate, useParams } from 'react-router'
import { useLogout, useMe, type Membership } from '@/api/auth'
import { useVersion } from '@/api/system'
import { AppShell } from '@/components/shell/AppShell'
import type { WorkspaceOption } from '@/components/shell/types'
import { StatusScreen } from '@/components/StatusScreen'
import { Button } from '@/components/ui/button'
import { RouterLink } from './RouterLink'
import { SessionStatus } from './SessionStatus'
import { workspaceNav } from './workspaceNav'

export function WorkspaceLayout() {
  const { ws } = useParams()
  const { pathname } = useLocation()
  const navigate = useNavigate()
  const me = useMe()
  const version = useVersion()
  const logout = useLogout()

  if (!me.data) {
    return <SessionStatus error={me.error} onRetry={() => me.refetch()} />
  }

  const current = me.data.workspaces.find((m) => m.slug === ws)
  if (!current) {
    // Không thuộc workspace và workspace không tồn tại báo cùng một câu, giống backend trả cùng một 404.
    return (
      <StatusScreen
        title="Không mở được workspace này"
        description={
          'Workspace không tồn tại, hoặc bạn không phải thành viên. Nhờ admin của workspace thêm bạn bằng email.'
        }
        action={
          <Button variant="outline" asChild>
            <Link to="/">Về workspace của bạn</Link>
          </Button>
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

  const handleLogout = () =>
    logout.mutate(undefined, { onSuccess: () => navigate('/', { replace: true }) })

  return (
    <AppShell
      title={activeEntry?.label ?? current.name}
      navItems={navItems}
      currentWorkspace={{ ...toOption(current), href: base }}
      workspaces={me.data.workspaces.map(toOption)}
      user={me.data.user}
      onLogout={handleLogout}
      logoutPending={logout.isPending}
      notice={logout.error ? `Đăng xuất không được: ${logout.error.message}` : undefined}
      serverInfo={version.data}
      link={RouterLink}
    >
      <Outlet />
    </AppShell>
  )
}

function workspaceHref(slug: string, path: string): string {
  const base = `/w/${encodeURIComponent(slug)}`
  return path ? `${base}/${path}` : base
}
