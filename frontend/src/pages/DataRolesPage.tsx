// SPDX-License-Identifier: Apache-2.0
/* pages/DataRolesPage.tsx - màn Vai dữ liệu của workspace (/w/:ws/data-roles, #45), chỉ cho admin: tạo vai, đặt luật lọc
dòng (dataset, bảng, cột chiều, giá trị), tick thành viên mang vai. Member mở đường dẫn này thì thấy lời báo; quyền
thật do backend kiểm (403). Luật ghép thế nào (AND trong một vai, OR giữa các vai) nói ở đầu trang, vì admin cần biết
trước khi gán nhiều vai cho một người. Mỗi lúc chỉ mở một khung sửa (luật hay tên) cho một vai. */

import { useState } from 'react'
import { useParams } from 'react-router'
import { useMe } from '@/api/auth'
import { useCatalog } from '@/api/catalog'
import {
  type DataRole,
  useCreateDataRole,
  useDataRoles,
  useDeleteDataRole,
  useFilterColumns,
  useSetDataRolePolicies,
  useSetMemberDataRoles,
  useUpdateDataRole,
} from '@/api/dataRoles'
import { useMembers } from '@/api/workspaces'
import { EmptyState } from '@/components/EmptyState'
import { FormMessage } from '@/components/forms/FormMessage'
import { RoleCard } from '@/components/dataRoles/RoleCard'
import { RoleNameForm } from '@/components/dataRoles/RoleNameForm'
import { RuleEditor } from '@/components/dataRoles/RuleEditor'
import type { DatasetChoice, MemberChoice, RoleItem, RuleDraft } from '@/components/dataRoles/types'

function toRoleItem(role: DataRole): RoleItem {
  return {
    id: role.id,
    name: role.name,
    description: role.description,
    rules: role.policies.map((p) => ({ tableId: p.table_id, column: p.column, values: p.allowed_values })),
    memberIds: role.member_ids,
  }
}

export function DataRolesPage() {
  const { ws = '' } = useParams()
  const me = useMe()
  const isAdmin = me.data?.workspaces.find((m) => m.slug === ws)?.role === 'admin'
  if (me.data && !isAdmin) {
    return (
      <EmptyState
        title="Chỉ admin xem được màn này"
        description="Admin của workspace đặt vai dữ liệu; vai của bạn hiện ở trang Thành viên."
      />
    )
  }
  return <AdminDataRoles key={ws} ws={ws} enabled={isAdmin} />
}

type Editing = { roleId: string; kind: 'rules' | 'name' }

function AdminDataRoles({ ws, enabled }: { ws: string; enabled: boolean }) {
  const roles = useDataRoles(ws, enabled)
  const catalog = useCatalog(ws)
  const members = useMembers(ws)
  const create = useCreateDataRole(ws)
  const update = useUpdateDataRole(ws)
  const remove = useDeleteDataRole(ws)
  const setPolicies = useSetDataRolePolicies(ws)
  const setMemberRoles = useSetMemberDataRoles(ws)
  const [editing, setEditing] = useState<Editing>()
  const [pickedTableId, setPickedTableId] = useState('')
  const [createKey, setCreateKey] = useState(0)
  const [notice, setNotice] = useState<string[]>([])
  const columns = useFilterColumns(ws, editing?.kind === 'rules' ? pickedTableId : '')

  const datasets: DatasetChoice[] = (catalog.data ?? []).map((d) => ({
    id: d.id,
    name: d.name,
    tables: d.tables.filter((t) => t.current_version_id !== null).map((t) => ({ id: t.id, name: t.name })),
  }))
  const items = (roles.data ?? []).map(toRoleItem)
  const memberChoices: MemberChoice[] = (members.data ?? []).map((m) => ({
    userId: m.user_id,
    name: m.display_name || m.email,
    isAdmin: m.role === 'admin',
  }))

  const open = (roleId: string, kind: Editing['kind']) => {
    setPolicies.reset()
    update.reset()
    setNotice([])
    setPickedTableId('')
    setEditing({ roleId, kind })
  }
  const saveRules = (role: RoleItem, rules: RuleDraft[]) =>
    setPolicies.mutate(
      {
        id: role.id,
        policies: rules.map((r) => ({ table_id: r.tableId, column: r.column, allowed_values: r.values })),
      },
      {
        onSuccess: (saved) => {
          setEditing(undefined)
          setNotice([`Đã lưu luật của vai ${role.name}.`, ...saved.warnings])
        },
      },
    )
  const toggleMember = (role: RoleItem, member: MemberChoice, on: boolean) => {
    if (setMemberRoles.isPending) return
    setNotice([])
    // API thay cả bộ vai của một người, nên dựng bộ mới từ mọi vai người đó đang mang.
    const current = items.filter((r) => r.memberIds.includes(member.userId)).map((r) => r.id)
    const next = on ? [...current, role.id] : current.filter((id) => id !== role.id)
    setMemberRoles.mutate({ userId: member.userId, roleIds: next })
  }
  const listError = setMemberRoles.error ?? remove.error

  return (
    <div className="flex flex-col gap-8">
      <div className="flex flex-col gap-1">
        <h2 className="text-lg font-semibold tracking-tight">Vai dữ liệu</h2>
        <p className="text-sm text-muted-foreground">
          Vai dữ liệu quyết định thành viên thấy dòng nào khi hỏi, xem tổng quan hay truy dòng nguồn. Các luật trong một
          vai nối bằng AND; người mang nhiều vai thấy dòng mà ít nhất một vai cho thấy (OR), chỉ tính vai có luật trên
          bảng đang đọc hay bảng nó trỏ tới. Lọc một bảng thì các bảng trỏ tới nó (đơn hàng của khách) cũng bị lọc theo.
          Thành viên không mang vai nào thấy mọi dòng; admin không bị lọc. Đổi luật có hiệu lực từ câu hỏi kế tiếp.
        </p>
      </div>

      <section aria-labelledby="new-role-heading" className="flex flex-col gap-3 rounded-xl border bg-card p-4 shadow-xs sm:px-6 sm:py-5">
        <h3 id="new-role-heading" className="text-[15px] font-semibold tracking-tight">
          Tạo vai
        </h3>
        <RoleNameForm
          key={createKey}
          submitLabel="Tạo vai"
          pending={create.isPending}
          error={create.error?.message}
          onSubmit={(name, description) =>
            create.mutate(
              { name, description: description || null },
              {
                onSuccess: (role) => {
                  setCreateKey((k) => k + 1)
                  open(role.id, 'rules')
                },
              },
            )
          }
        />
      </section>

      <section aria-labelledby="roles-heading" className="flex flex-col gap-3">
        <h3 id="roles-heading" className="text-[15px] font-semibold tracking-tight">
          Các vai{roles.data && ` (${roles.data.length})`}
        </h3>
        {roles.error && <FormMessage kind="error">{roles.error.message}</FormMessage>}
        {listError && <FormMessage kind="error">{listError.message}</FormMessage>}
        {notice.map((line) => (
          <FormMessage key={line} kind="success">
            {line}
          </FormMessage>
        ))}
        {roles.data === undefined && !roles.error && <p className="text-sm text-muted-foreground">Đang tải...</p>}
        {roles.data?.length === 0 && (
          <EmptyState title="Chưa có vai nào" description="Tạo vai, thêm luật, rồi tick thành viên mang vai." />
        )}
        <ul className="flex flex-col gap-4">
          {items.map((role) => (
            <RoleCard
              key={role.id}
              role={role}
              datasets={datasets}
              members={memberChoices}
              busyUserId={setMemberRoles.isPending ? setMemberRoles.variables?.userId : undefined}
              onToggleMember={(member, on) => toggleMember(role, member, on)}
              onEditRules={() => open(role.id, 'rules')}
              onRename={() => open(role.id, 'name')}
              onDelete={() => !remove.isPending && remove.mutate(role.id)}
            >
              {editing?.roleId === role.id && editing.kind === 'rules' && (
                <RuleEditor
                  roleName={role.name}
                  datasets={datasets}
                  initialRules={role.rules}
                  pickedTableId={pickedTableId}
                  onPickTable={setPickedTableId}
                  columns={columns.data}
                  columnsError={columns.error?.message}
                  pending={setPolicies.isPending}
                  error={setPolicies.error?.message}
                  onSave={(rules) => saveRules(role, rules)}
                  onCancel={() => setEditing(undefined)}
                />
              )}
              {editing?.roleId === role.id && editing.kind === 'name' && (
                <RoleNameForm
                  initialName={role.name}
                  initialDescription={role.description ?? ''}
                  submitLabel="Lưu tên"
                  pending={update.isPending}
                  error={update.error?.message}
                  onSubmit={(name, description) =>
                    update.mutate(
                      { id: role.id, name, description: description || null },
                      { onSuccess: () => setEditing(undefined) },
                    )
                  }
                  onCancel={() => setEditing(undefined)}
                />
              )}
            </RoleCard>
          ))}
        </ul>
      </section>
    </div>
  )
}
