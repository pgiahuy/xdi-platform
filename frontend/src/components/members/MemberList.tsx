// SPDX-License-Identifier: Apache-2.0
/* components/members/MemberList.tsx - danh sách thành viên của workspace. Ai cũng xem được; canManage (admin) thì mỗi
dòng có ô đổi vai và nút xóa. Dòng của chính mình có nút rời workspace thay cho nút xóa, với mọi vai.
Admin tự hạ vai mình thì hỏi lại trước: hạ xong là mất quyền quản lý, không tự lấy lại được.
Vai dữ liệu của từng người chỉ hiện để đọc (gán ở màn Vai dữ liệu).
Mỗi dòng là một khối tự xuống hàng, không dùng bảng: ở bề ngang điện thoại phần nút nằm dưới tên.
Nút không bị disabled lúc chờ (trang chặn bấm trùng): disabled làm focus rơi khỏi nút, người dùng bàn phím mất chỗ. */

import { useState } from 'react'
import { Trash2Icon } from 'lucide-react'
import { ConfirmDialog } from '@/components/ConfirmDialog'
import { RoleSelect } from '@/components/forms/RoleSelect'
import { roleLabel } from '@/components/shell/labels'
import { Button } from '@/components/ui/button'
import type { MemberItem } from './types'

type Props = {
  members: MemberItem[]
  currentUserId: string
  canManage: boolean
  /** Dòng đang chờ backend trả lời, kèm vai vừa chọn nếu đang đổi vai: ô chọn hiện vai mới trong lúc chờ. */
  busy?: { userId: string; role?: string }
  workspaceName: string
  onRoleChange: (member: MemberItem, role: string) => void
  onRemove: (member: MemberItem) => void
  onLeave: () => void
}

const roleSelectId = (userId: string) => `member-role-${userId}`

export function MemberList(props: Props) {
  const { members, currentUserId, canManage, busy, workspaceName } = props
  // Vai mình vừa chọn cho chính mình, đang chờ xác nhận. Giữ riêng với trạng thái mở của hộp: hộp còn mờ dần sau khi
  // đóng, xóa vai ngay lúc đóng thì tiêu đề thoáng hiện thiếu chữ.
  const [selfDemotion, setSelfDemotion] = useState<string>()
  const [demotionOpen, setDemotionOpen] = useState(false)
  const self = members.find((m) => m.userId === currentUserId)

  const handleSelect = (member: MemberItem, role: string) => {
    if (member.userId === currentUserId && member.role === 'admin' && role !== 'admin') {
      setSelfDemotion(role)
      setDemotionOpen(true)
      return
    }
    props.onRoleChange(member, role)
  }

  return (
    <>
      <ul className="divide-y rounded-xl border bg-card shadow-xs">
        {members.map((member) => {
          const isSelf = member.userId === currentUserId
          const name = member.name || member.email
          const rowBusy = member.userId === busy?.userId
          return (
            <li key={member.userId} className="flex flex-wrap items-center gap-x-4 gap-y-2 px-3 py-2.5">
              <div className="min-w-0 flex-1 basis-48">
                <p className="truncate text-sm font-medium" title={name}>
                  {name}
                  {isSelf && <span className="font-normal text-muted-foreground"> (bạn)</span>}
                </p>
                {member.name && (
                  <p className="truncate text-xs text-muted-foreground" title={member.email}>
                    {member.email}
                  </p>
                )}
                {member.dataRoles && member.dataRoles.length > 0 && (
                  <p className="text-xs text-muted-foreground">Vai dữ liệu: {member.dataRoles.join(', ')}</p>
                )}
              </div>
              <div className="flex items-center gap-2">
                {canManage ? (
                  <RoleSelect
                    id={roleSelectId(member.userId)}
                    aria-label={`Vai của ${name}`}
                    aria-busy={rowBusy}
                    value={(rowBusy && busy?.role) || member.role}
                    onChange={(e) => handleSelect(member, e.target.value)}
                  />
                ) : (
                  <span className="text-sm text-muted-foreground">{roleLabel(member.role)}</span>
                )}
                {isSelf ? (
                  <ConfirmDialog
                    trigger={
                      <Button variant="outline" size="sm" aria-disabled={rowBusy}>
                        Rời workspace
                      </Button>
                    }
                    title={`Rời ${workspaceName}?`}
                    description="Bạn sẽ không còn thấy dữ liệu của workspace này. Muốn vào lại thì nhờ admin thêm bạn."
                    confirmLabel="Rời workspace"
                    onConfirm={props.onLeave}
                  />
                ) : (
                  canManage && (
                    <ConfirmDialog
                      trigger={
                        <Button
                          variant="ghost"
                          size="icon"
                          aria-disabled={rowBusy}
                          aria-label={`Xóa ${name} khỏi workspace`}
                        >
                          <Trash2Icon aria-hidden="true" />
                        </Button>
                      }
                      title={`Xóa ${name} khỏi workspace?`}
                      description="Người này không còn thấy dữ liệu của workspace. Tài khoản của họ vẫn còn."
                      confirmLabel="Xóa khỏi workspace"
                      onConfirm={() => props.onRemove(member)}
                    />
                  )
                )}
              </div>
            </li>
          )
        })}
      </ul>
      {self && (
        <ConfirmDialog
          open={demotionOpen}
          onOpenChange={setDemotionOpen}
          // Hộp này mở từ ô chọn vai, không có trigger; đóng thì đưa focus về lại ô đó.
          onCloseAutoFocus={(event) => {
            event.preventDefault()
            document.getElementById(roleSelectId(self.userId))?.focus()
          }}
          title={`Hạ vai của bạn xuống ${roleLabel(selfDemotion ?? '')}?`}
          description={
            `Bạn sẽ không còn thêm, xóa thành viên hay đổi tên, xóa ${workspaceName}. ` +
            'Muốn lấy lại vai Admin thì nhờ một admin khác.'
          }
          confirmLabel="Hạ vai"
          onConfirm={() => selfDemotion && props.onRoleChange(self, selfDemotion)}
        />
      )}
    </>
  )
}
