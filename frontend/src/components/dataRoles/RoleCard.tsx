// SPDX-License-Identifier: Apache-2.0
/* components/dataRoles/RoleCard.tsx - một vai dữ liệu: tên, mô tả, luật, và danh sách thành viên tick để gán hay gỡ vai.
Nút sửa luật, đổi tên, xóa gọi lên trang; khung sửa (RuleEditor, RoleNameForm) do trang đặt vào children. Vai chưa có
luật thì ô tick bị khóa, vì backend không cho gán vai không luật. Admin không bị lọc dù mang vai, nên dòng của admin
ghi rõ điều đó. Nút không bị disabled lúc chờ (trang chặn bấm trùng), như components/members/MemberList.tsx. */

import type { ReactNode } from 'react'
import { PencilIcon, Trash2Icon } from 'lucide-react'
import { ConfirmDialog } from '@/components/ConfirmDialog'
import { Button } from '@/components/ui/button'
import { RuleText } from './RuleText'
import { type DatasetChoice, type MemberChoice, type RoleItem, tableLabel } from './types'

type Props = {
  role: RoleItem
  datasets: DatasetChoice[]
  members: MemberChoice[]
  /** Thành viên đang chờ backend lưu vai. */
  busyUserId?: string
  onToggleMember: (member: MemberChoice, on: boolean) => void
  onEditRules: () => void
  onRename: () => void
  onDelete: () => void
  /** Khung sửa đang mở cho vai này. */
  children?: ReactNode
}

export function RoleCard({ role, datasets, members, busyUserId, children, ...props }: Props) {
  const noRules = role.rules.length === 0
  return (
    <li className="flex flex-col gap-3 rounded-xl border bg-card p-4 shadow-xs sm:px-5">
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="min-w-0">
          <h3 className="text-[15px] font-semibold tracking-tight">{role.name}</h3>
          {role.description && <p className="text-sm text-muted-foreground">{role.description}</p>}
        </div>
        <div className="flex items-center gap-1">
          <Button variant="outline" size="sm" onClick={props.onEditRules}>
            Sửa luật
          </Button>
          <Button variant="ghost" size="icon" aria-label={`Đổi tên vai ${role.name}`} onClick={props.onRename}>
            <PencilIcon aria-hidden="true" />
          </Button>
          <ConfirmDialog
            trigger={
              <Button variant="ghost" size="icon" aria-label={`Xóa vai ${role.name}`}>
                <Trash2Icon aria-hidden="true" />
              </Button>
            }
            title={`Xóa vai ${role.name}?`}
            description="Luật của vai bị xóa theo. Vai đang gán cho ai thì phải gỡ khỏi họ trước."
            confirmLabel="Xóa vai"
            onConfirm={props.onDelete}
          />
        </div>
      </div>

      {noRules ? (
        <p className="text-sm text-muted-foreground">Chưa có luật nào.</p>
      ) : (
        <ul className="flex flex-col gap-1 text-sm">
          {role.rules.map((rule) => (
            <li key={`${rule.tableId}/${rule.column}`}>
              <RuleText rule={rule} table={tableLabel(datasets, rule.tableId)} />
            </li>
          ))}
        </ul>
      )}
      {children}

      <fieldset className="flex flex-col gap-1.5">
        <legend className="mb-1 text-sm font-medium">Người mang vai</legend>
        {noRules && <p className="text-xs text-muted-foreground">Thêm luật trước khi gán vai.</p>}
        <div className="flex flex-wrap gap-x-5 gap-y-1.5">
          {members.map((member) => (
            <label key={member.userId} className="flex items-center gap-1.5 text-sm">
              <input
                type="checkbox"
                className="size-4 accent-primary"
                checked={role.memberIds.includes(member.userId)}
                disabled={noRules && !role.memberIds.includes(member.userId)}
                aria-busy={busyUserId === member.userId}
                onChange={(e) => props.onToggleMember(member, e.target.checked)}
              />
              {member.name}
              {member.isAdmin && <span className="text-xs text-muted-foreground">(admin, không bị lọc)</span>}
            </label>
          ))}
        </div>
      </fieldset>
    </li>
  )
}
