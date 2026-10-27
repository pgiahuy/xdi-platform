// SPDX-License-Identifier: Apache-2.0
/* pages/RootPage.tsx - địa chỉ gốc /. Mọi trang làm việc nằm dưới /w/<workspace>, nên ở đây chỉ chuyển sang
workspace đầu tiên của người dùng (backend sắp theo tên), hoặc báo khi họ chưa thuộc workspace nào. Quản trị hệ thống
chưa thuộc workspace nào (ví dụ vừa xóa workspace duy nhất của mình) có thêm lối vào trang quản trị, nơi họ tự chỉ định
mình làm admin một workspace (#99). Ai đăng nhập cũng tạo được workspace (backend/src/service/workspaces.rs), nên màn
chưa thuộc workspace nào luôn có nút tạo; tạo xong thì mở trang đầu của workspace mới. */

import { useState } from 'react'
import { Link, Navigate, useNavigate } from 'react-router'
import { useMe } from '@/api/auth'
import { isUnauthorized } from '@/api/queryClient'
import { useCreateWorkspace } from '@/api/workspaces'
import { CreateWorkspaceDialog } from '@/components/shell/CreateWorkspaceDialog'
import { Button } from '@/components/ui/button'
import { adminWorkspacesRoute } from './globalNav'
import { SessionStatus } from './SessionStatus'
import { SignedInScreen } from './SignedInScreen'
import { workspaceHref } from './workspaceNav'

export function RootPage() {
  const me = useMe()
  const navigate = useNavigate()
  const createWorkspace = useCreateWorkspace()
  const [createOpen, setCreateOpen] = useState(false)
  // Kiểm 401 trước: phiên bị thu hồi thì bộ đệm vẫn còn người dùng cũ, chỉ nhìn data sẽ tưởng còn đăng nhập.
  if (!me.data || isUnauthorized(me.error)) {
    return <SessionStatus error={me.error} onRetry={() => me.refetch()} />
  }
  const first = me.data.workspaces[0]
  // Vừa tạo xong thì bộ đệm đã có workspace mới (useCreateWorkspace), nhánh này chuyển sang nó.
  if (first) return <Navigate replace to={workspaceHref(first.slug, '')} />

  const handleCreateOpenChange = (open: boolean) => {
    createWorkspace.reset()
    setCreateOpen(open)
  }
  const handleCreate = (name: string) => {
    if (createWorkspace.isPending) return
    createWorkspace.mutate(name, { onSuccess: (created) => navigate(workspaceHref(created.slug, '')) })
  }
  const createButton = (
    <CreateWorkspaceDialog
      trigger={<Button>Tạo workspace</Button>}
      open={createOpen}
      onOpenChange={handleCreateOpenChange}
      onCreate={handleCreate}
      pending={createWorkspace.isPending}
      error={createWorkspace.error?.message}
    />
  )

  if (me.data.user.is_system_admin) {
    return (
      <SignedInScreen
        user={me.data.user}
        title="Bạn chưa thuộc workspace nào"
        description="Tạo một workspace mới, hoặc ở trang Mọi workspace tự chỉ định mình làm admin của một workspace có sẵn."
        action={
          <>
            {createButton}
            <Button variant="outline" asChild>
              <Link to={adminWorkspacesRoute}>Mở trang quản trị</Link>
            </Button>
          </>
        }
      />
    )
  }
  return (
    <SignedInScreen
      user={me.data.user}
      title="Bạn chưa thuộc workspace nào"
      description="Tạo một workspace mới, hoặc nhờ admin của một workspace có sẵn thêm bạn bằng email đăng nhập rồi tải lại trang."
      action={createButton}
    />
  )
}
