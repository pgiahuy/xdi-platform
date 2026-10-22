// SPDX-License-Identifier: Apache-2.0
/* components/admin/WorkspaceOverviewList.tsx - mọi workspace của bản cài kèm thành viên, cho quản trị hệ thống, và ô
chỉ định admin cho từng workspace. Dùng khi workspace mất admin, ví dụ admin cuối nghỉ việc và bị khóa tài khoản.
Không có link vào workspace: quản trị hệ thống không tự có quyền xem dữ liệu bên trong (docs/ARCHITECTURE.md mục 4b).
Email không có tài khoản thì lời backend hiện dưới ô email và focus về ô đó (quy ước ở forms/formError.ts). */

import { useMemo, useState, type FormEvent } from 'react'
import { FormField } from '@/components/forms/FormField'
import { type FormError, useFocusOnError } from '@/components/forms/formError'
import { FormMessage } from '@/components/forms/FormMessage'
import { roleLabel } from '@/components/shell/labels'
import { Button } from '@/components/ui/button'
import type { WorkspaceItem } from './types'

export type AssignAdminField = 'email'

type Props = {
  workspaces: WorkspaceItem[]
  busyId?: string
  /** Lỗi của lần chỉ định gần nhất, kèm workspace nó thuộc về. */
  error?: { workspaceId: string; value: FormError<AssignAdminField> }
  success?: { workspaceId: string; text: string }
  /** clearForm xóa ô email; trang gọi khi chỉ định xong, lỗi thì không gọi để người dùng sửa. */
  onAssignAdmin: (workspace: WorkspaceItem, email: string, clearForm: () => void) => void
}

export function WorkspaceOverviewList({ workspaces, busyId, error, success, onAssignAdmin }: Props) {
  return (
    <ul className="flex flex-col gap-4">
      {workspaces.map((workspace) => (
        <li key={workspace.id} className="flex flex-col gap-3 rounded-lg border p-3">
          <div className="min-w-0">
            <h3 className="truncate text-sm font-semibold" title={workspace.name}>
              {workspace.name}
            </h3>
            <p className="truncate text-xs text-muted-foreground">/w/{workspace.slug}</p>
          </div>
          <ul className="flex flex-col gap-1 text-sm">
            {workspace.members.map((member) => (
              <li key={member.userId} className="flex flex-wrap justify-between gap-x-3">
                <span className="min-w-0 truncate" title={member.email}>
                  {member.name || member.email}
                </span>
                <span className="text-muted-foreground">{roleLabel(member.role)}</span>
              </li>
            ))}
          </ul>
          <AssignAdminForm
            workspace={workspace}
            pending={workspace.id === busyId}
            error={error?.workspaceId === workspace.id ? error.value : undefined}
            onSubmit={onAssignAdmin}
          />
          {success?.workspaceId === workspace.id && <FormMessage kind="success">{success.text}</FormMessage>}
        </li>
      ))}
    </ul>
  )
}

type FormProps = {
  workspace: WorkspaceItem
  pending: boolean
  error?: FormError<AssignAdminField>
  onSubmit: Props['onAssignAdmin']
}

function AssignAdminForm({ workspace, pending, error, onSubmit }: FormProps) {
  const [email, setEmail] = useState('')
  // useFocusOnError cần đối tượng id không đổi giữa các lần vẽ.
  const fieldIds = useMemo(() => ({ email: `assign-admin-${workspace.id}` }), [workspace.id])
  useFocusOnError(error, fieldIds)

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (pending) return
    onSubmit(workspace, email.trim(), () => setEmail(''))
  }
  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-3">
      {/* items-start: câu lỗi dưới ô email làm cột đó cao hơn, nút vẫn phải ngang hàng với ô. */}
      <div className="flex flex-wrap items-start gap-3">
        <div className="min-w-0 flex-1 basis-56">
          <FormField
            id={fieldIds.email}
            label="Chỉ định admin theo email"
            type="email"
            autoComplete="off"
            required
            error={error?.field === 'email' ? error.message : undefined}
            value={email}
            onChange={(e) => setEmail(e.target.value)}
          />
        </div>
        <div className="flex flex-col gap-1.5">
          {/* Chỗ trống cao bằng nhãn, để nút ngang hàng với ô nhập. */}
          <span aria-hidden="true" className="text-sm leading-none">
            &nbsp;
          </span>
          <Button type="submit" variant="outline" className="h-9" aria-disabled={pending}>
            {pending ? 'Đang chỉ định...' : 'Chỉ định'}
          </Button>
        </div>
      </div>
      {error && !error.field && <FormMessage kind="error">{error.message}</FormMessage>}
    </form>
  )
}
