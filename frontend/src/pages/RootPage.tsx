// SPDX-License-Identifier: Apache-2.0
/* pages/RootPage.tsx - địa chỉ gốc /. Mọi trang làm việc nằm dưới /w/<workspace>, nên ở đây chỉ chuyển sang
workspace đầu tiên của người dùng (backend sắp theo tên), hoặc báo khi họ chưa thuộc workspace nào. */

import { Navigate } from 'react-router'
import { useMe } from '@/api/auth'
import { StatusScreen } from '@/components/StatusScreen'
import { SessionStatus } from './SessionStatus'

export function RootPage() {
  const me = useMe()
  if (!me.data) {
    return <SessionStatus error={me.error} onRetry={() => me.refetch()} />
  }
  const first = me.data.workspaces[0]
  if (!first) {
    return (
      <StatusScreen
        title="Bạn chưa thuộc workspace nào"
        description="Nhờ admin của một workspace thêm bạn bằng email đăng nhập, rồi tải lại trang."
      />
    )
  }
  return <Navigate replace to={`/w/${encodeURIComponent(first.slug)}`} />
}
