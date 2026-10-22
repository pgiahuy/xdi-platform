// SPDX-License-Identifier: Apache-2.0
/* pages/RootPage.tsx - địa chỉ gốc /. Mọi trang làm việc nằm dưới /w/<workspace>, nên ở đây chỉ chuyển sang
workspace đầu tiên của người dùng (backend sắp theo tên), hoặc báo khi họ chưa thuộc workspace nào. Quản trị hệ thống
chưa thuộc workspace nào (ví dụ vừa xóa workspace duy nhất của mình) có thêm lối vào trang quản trị, nơi họ tự chỉ định
mình làm admin một workspace (#99). */

import { Link, Navigate } from 'react-router'
import { useMe } from '@/api/auth'
import { isUnauthorized } from '@/api/queryClient'
import { Button } from '@/components/ui/button'
import { adminWorkspacesRoute } from './globalNav'
import { SessionStatus } from './SessionStatus'
import { SignedInScreen } from './SignedInScreen'
import { workspaceHref } from './workspaceNav'

export function RootPage() {
  const me = useMe()
  // Kiểm 401 trước: phiên bị thu hồi thì bộ đệm vẫn còn người dùng cũ, chỉ nhìn data sẽ tưởng còn đăng nhập.
  if (!me.data || isUnauthorized(me.error)) {
    return <SessionStatus error={me.error} onRetry={() => me.refetch()} />
  }
  const first = me.data.workspaces[0]
  if (first) return <Navigate replace to={workspaceHref(first.slug, '')} />

  if (me.data.user.is_system_admin) {
    return (
      <SignedInScreen
        user={me.data.user}
        title="Bạn chưa thuộc workspace nào"
        description="Bạn là quản trị hệ thống: ở trang Mọi workspace, tự chỉ định mình làm admin của một workspace để vào làm việc."
        action={
          <Button asChild>
            <Link to={adminWorkspacesRoute}>Mở trang quản trị</Link>
          </Button>
        }
      />
    )
  }
  return (
    <SignedInScreen
      user={me.data.user}
      title="Bạn chưa thuộc workspace nào"
      description="Nhờ admin của một workspace thêm bạn bằng email đăng nhập, rồi tải lại trang."
    />
  )
}
