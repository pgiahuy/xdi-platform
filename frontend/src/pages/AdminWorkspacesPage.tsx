// SPDX-License-Identifier: Apache-2.0
/* pages/AdminWorkspacesPage.tsx - trang Mọi workspace cho quản trị hệ thống: danh sách workspace kèm thành viên, chỉ
định admin. Mục điều hướng chỉ hiện với quản trị hệ thống (workspaceNav.ts); người khác gõ thẳng đường dẫn thì thấy một
câu báo và trang không gọi API (backend vẫn trả 403 nếu gọi). */

import { useMemo, useState } from 'react'
import { useMe } from '@/api/auth'
import { ApiError } from '@/api/client'
import { useAssignWorkspaceAdmin, useWorkspacesOverview } from '@/api/admin'
import { SystemAdminOnly } from '@/components/admin/SystemAdminOnly'
import type { WorkspaceItem } from '@/components/admin/types'
import { type AssignAdminField, WorkspaceOverviewList } from '@/components/admin/WorkspaceOverviewList'
import { EmptyState } from '@/components/EmptyState'
import type { FormError } from '@/components/forms/formError'
import { Button } from '@/components/ui/button'

export function AdminWorkspacesPage() {
  const me = useMe()
  const isSystemAdmin = me.data?.user.is_system_admin === true
  const overview = useWorkspacesOverview(isSystemAdmin)
  const assign = useAssignWorkspaceAdmin()
  const [success, setSuccess] = useState<{ workspaceId: string; text: string }>()
  // 404 thường là email không có tài khoản đang hoạt động: báo ở ô email. Lỗi khác báo chung dưới form.
  const assignError = useMemo(() => {
    const error = assign.error
    const workspaceId = assign.variables?.workspaceId
    if (!error || !workspaceId) return undefined
    const field: AssignAdminField | undefined = error instanceof ApiError && error.status === 404 ? 'email' : undefined
    const value: FormError<AssignAdminField> = { message: error.message, field }
    return { workspaceId, value }
  }, [assign.error, assign.variables])

  if (me.data && !isSystemAdmin) return <SystemAdminOnly />
  if (overview.error) {
    return (
      <EmptyState
        title="Không tải được danh sách workspace"
        description={overview.error.message}
        action={
          <Button variant="outline" onClick={() => overview.refetch()}>
            Thử lại
          </Button>
        }
      />
    )
  }
  if (!overview.data) {
    return (
      <p aria-busy="true" className="text-sm text-muted-foreground">
        Đang tải...
      </p>
    )
  }

  const handleAssign = (workspace: WorkspaceItem, email: string, clearForm: () => void) => {
    if (assign.isPending) return
    setSuccess(undefined)
    assign.mutate(
      { workspaceId: workspace.id, email },
      {
        onSuccess: () => {
          setSuccess({ workspaceId: workspace.id, text: `Đã chỉ định ${email} làm admin.` })
          clearForm()
        },
      },
    )
  }

  const workspaces: WorkspaceItem[] = overview.data.map((w) => ({
    id: w.workspace_id,
    name: w.name,
    slug: w.slug,
    members: w.members.map((m) => ({ userId: m.user_id, name: m.display_name, email: m.email, role: m.role })),
  }))

  return (
    <section aria-labelledby="workspaces-heading" className="flex max-w-3xl flex-col gap-3">
      <h2 id="workspaces-heading" className="text-sm font-semibold">
        Workspace ({workspaces.length})
      </h2>
      <p className="text-sm text-muted-foreground">
        Quản trị hệ thống thấy thành viên của mọi workspace nhưng không thấy dữ liệu bên trong. Chỉ định admin khi
        workspace không còn admin dùng được, ví dụ admin cuối đã bị khóa tài khoản.
      </p>
      <WorkspaceOverviewList
        workspaces={workspaces}
        busyId={assign.isPending ? assign.variables?.workspaceId : undefined}
        error={assignError}
        success={success}
        onAssignAdmin={handleAssign}
      />
    </section>
  )
}
