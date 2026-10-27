// SPDX-License-Identifier: Apache-2.0
/* pages/MembersPage.tsx - trang Thành viên của workspace: danh sách thành viên, tự rời workspace (mọi người); thêm,
đổi vai, xóa thành viên, đổi tên và xóa workspace (admin). Quyền do backend kiểm (docs/ARCHITECTURE.md mục 4b); trang
chỉ ẩn nút người dùng không bấm được, còn lời từ chối của backend (ví dụ hạ vai admin cuối cùng) hiện nguyên văn. */

import { useMemo, useState } from 'react'
import { useNavigate, useParams } from 'react-router'
import { useMe } from '@/api/auth'
import { ApiError } from '@/api/client'
import {
  useAddMember,
  useDeleteWorkspace,
  useLeaveWorkspace,
  useMembers,
  useRemoveMember,
  useRenameWorkspace,
  useSetMemberRole,
} from '@/api/workspaces'
import { EmptyState } from '@/components/EmptyState'
import type { FormError } from '@/components/forms/formError'
import { FormMessage } from '@/components/forms/FormMessage'
import { ResultMessage } from '@/components/forms/ResultMessage'
import { type AddMemberField, AddMemberForm } from '@/components/members/AddMemberForm'
import { MemberList } from '@/components/members/MemberList'
import type { MemberItem } from '@/components/members/types'
import { WorkspaceSettings } from '@/components/members/WorkspaceSettings'
import { roleLabel } from '@/components/shell/labels'
import { Button } from '@/components/ui/button'

export function MembersPage() {
  const { ws = '' } = useParams()
  // Đổi workspace bằng thanh bên thì route giữ nguyên trang này. Gắn lại theo workspace để lỗi, thông báo và ô nhập của
  // workspace trước không hiện sang workspace sau.
  return <MembersContent key={ws} ws={ws} />
}

function MembersContent({ ws }: { ws: string }) {
  const navigate = useNavigate()
  const me = useMe()
  const members = useMembers(ws)
  const addMember = useAddMember(ws)
  const setRole = useSetMemberRole(ws)
  const removeMember = useRemoveMember(ws)
  const leave = useLeaveWorkspace(ws)
  const rename = useRenameWorkspace(ws)
  const deleteWorkspace = useDeleteWorkspace(ws)
  // Mỗi lần thêm xong thì tăng, để AddMemberForm gắn lại và xóa ô email.
  const [addFormKey, setAddFormKey] = useState(0)
  const [addedMessage, setAddedMessage] = useState<string>()
  // Câu báo sau khi xóa một thành viên; id đổi mỗi lần để ResultMessage gắn lại và nhận focus.
  const [removed, setRemoved] = useState<{ id: number; text: string }>()
  // 404 là email chưa có tài khoản, 400 là đã là thành viên: cả hai báo ở ô email. Lỗi khác (mạng) báo chung cuối form.
  const addError = useMemo<FormError<AddMemberField> | undefined>(() => {
    const error = addMember.error
    if (!error) return undefined
    const emailProblem = error instanceof ApiError && (error.status === 400 || error.status === 404)
    return { message: error.message, field: emailProblem ? 'email' : undefined }
  }, [addMember.error])

  // AppLayout đã kiểm người dùng thuộc workspace này trước khi hiện trang, nên hai giá trị dưới đây luôn có.
  const current = me.data?.workspaces.find((m) => m.slug === ws)
  if (!me.data || !current) return null
  const userId = me.data.user.id
  const isAdmin = current.role === 'admin'

  const listPending = setRole.isPending || removeMember.isPending || leave.isPending
  // Ba việc trên danh sách dùng chung một dòng báo; bắt đầu việc mới thì bỏ lỗi và câu báo của việc cũ.
  const startListAction = () => {
    setRole.reset()
    removeMember.reset()
    leave.reset()
    setRemoved(undefined)
  }
  const listError = setRole.error ?? removeMember.error ?? leave.error
  const busy = setRole.isPending
    ? setRole.variables
    : removeMember.isPending
      ? { userId: removeMember.variables }
      : leave.isPending
        ? { userId }
        : undefined

  const handleRoleChange = (member: MemberItem, role: string) => {
    if (listPending) return
    startListAction()
    setRole.mutate({ userId: member.userId, role })
  }
  const handleRemove = (member: MemberItem) => {
    if (listPending) return
    startListAction()
    const name = member.name || member.email
    // Dòng của người bị xóa biến mất (mutation chờ danh sách tải lại xong), nên đưa focus tới câu báo.
    removeMember.mutate(member.userId, {
      onSuccess: () => setRemoved({ id: Date.now(), text: `Đã xóa ${name} khỏi workspace.` }),
    })
  }
  const handleLeave = () => {
    if (listPending) return
    startListAction()
    leave.mutate(userId, { onSuccess: () => navigate('/', { replace: true }) })
  }
  const handleAdd = (email: string, role: string) => {
    setAddedMessage(undefined)
    addMember.mutate(
      { email, role },
      {
        onSuccess: () => {
          setAddedMessage(`Đã thêm ${email} với vai ${roleLabel(role)}.`)
          setAddFormKey((key) => key + 1)
        },
      },
    )
  }
  const handleRename = (name: string) => {
    if (!rename.isPending) rename.mutate(name)
  }
  const handleDelete = () => {
    if (deleteWorkspace.isPending) return
    deleteWorkspace.mutate(undefined, { onSuccess: () => navigate('/', { replace: true }) })
  }

  const items: MemberItem[] = (members.data ?? []).map((m) => ({
    userId: m.user_id,
    name: m.display_name,
    email: m.email,
    role: m.role,
  }))

  return (
    <div className="flex max-w-3xl flex-col gap-10">
      <section aria-labelledby="members-heading" className="flex flex-col gap-3">
        <h2 id="members-heading" className="text-[15px] font-semibold tracking-tight">
          Thành viên{members.data && ` (${members.data.length})`}
        </h2>
        {members.error ? (
          <EmptyState
            title="Không tải được danh sách thành viên"
            description={members.error.message}
            action={
              <Button variant="outline" onClick={() => members.refetch()}>
                Thử lại
              </Button>
            }
          />
        ) : members.data ? (
          <>
            {listError && <FormMessage kind="error">{listError.message}</FormMessage>}
            {removed && (
              <ResultMessage key={removed.id} kind="success">
                {removed.text}
              </ResultMessage>
            )}
            <MemberList
              members={items}
              currentUserId={userId}
              canManage={isAdmin}
              busy={busy}
              workspaceName={current.name}
              onRoleChange={handleRoleChange}
              onRemove={handleRemove}
              onLeave={handleLeave}
            />
          </>
        ) : (
          <p aria-busy="true" className="text-sm text-muted-foreground">
            Đang tải...
          </p>
        )}
      </section>

      {isAdmin && (
        <section aria-labelledby="add-member-heading" className="flex flex-col gap-3 rounded-xl border bg-card p-4 shadow-xs sm:px-6 sm:py-5">
          <h2 id="add-member-heading" className="text-[15px] font-semibold tracking-tight">
            Thêm thành viên
          </h2>
          <p className="text-sm text-muted-foreground">
            Nhập email của một tài khoản đã có. Chưa có tài khoản thì nhờ quản trị hệ thống tạo trước.
          </p>
          <AddMemberForm
            key={addFormKey}
            onSubmit={handleAdd}
            pending={addMember.isPending}
            error={addError}
            success={addedMessage}
          />
        </section>
      )}

      {isAdmin && (
        <section aria-labelledby="workspace-heading" className="flex flex-col gap-3 rounded-xl border bg-card p-4 shadow-xs sm:px-6 sm:py-5">
          <h2 id="workspace-heading" className="text-[15px] font-semibold tracking-tight">
            Workspace
          </h2>
          <WorkspaceSettings
            name={current.name}
            onRename={handleRename}
            renamePending={rename.isPending}
            renameError={rename.error?.message}
            renameSuccess={rename.isSuccess ? 'Đã đổi tên.' : undefined}
            onDelete={handleDelete}
            deletePending={deleteWorkspace.isPending}
            deleteError={deleteWorkspace.error?.message}
          />
        </section>
      )}
    </div>
  )
}
