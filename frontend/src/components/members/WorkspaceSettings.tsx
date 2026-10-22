// SPDX-License-Identifier: Apache-2.0
/* components/members/WorkspaceSettings.tsx - admin đổi tên và xóa workspace. Đổi tên không đổi đường dẫn (slug giữ
nguyên ở backend), nên link đã chia sẻ vẫn mở được. Xóa thì hỏi lại trước, vì mất cả dataset và file.
Nút không bị disabled lúc chờ (chặn bấm trùng ở handler và ở trang): disabled làm focus rơi khỏi nút. */

import { useState, type FormEvent } from 'react'
import { ConfirmDialog } from '@/components/ConfirmDialog'
import { FormField } from '@/components/forms/FormField'
import { FormMessage } from '@/components/forms/FormMessage'
import { Button } from '@/components/ui/button'

type Props = {
  name: string
  onRename: (name: string) => void
  renamePending: boolean
  renameError?: string
  renameSuccess?: string
  onDelete: () => void
  deletePending: boolean
  deleteError?: string
}

export function WorkspaceSettings(props: Props) {
  const [name, setName] = useState(props.name)

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (props.renamePending) return
    props.onRename(name.trim())
  }

  return (
    <div className="flex max-w-xl flex-col gap-6">
      <form onSubmit={handleSubmit} className="flex flex-col gap-3">
        <div className="flex flex-wrap items-end gap-3">
          <div className="min-w-0 flex-1 basis-56">
            <FormField
              id="workspace-name"
              label="Tên workspace"
              required
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>
          <Button type="submit" variant="outline" className="h-9" aria-disabled={props.renamePending}>
            {props.renamePending ? 'Đang lưu...' : 'Đổi tên'}
          </Button>
        </div>
        {props.renameError && <FormMessage kind="error">{props.renameError}</FormMessage>}
        {props.renameSuccess && !props.renameError && <FormMessage kind="success">{props.renameSuccess}</FormMessage>}
      </form>

      <div className="flex flex-col items-start gap-2">
        <p className="text-sm text-muted-foreground">
          Xóa workspace là xóa mọi dataset, file và thành viên của nó. Không hoàn tác được.
        </p>
        <ConfirmDialog
          trigger={
            <Button variant="destructive" aria-disabled={props.deletePending}>
              {props.deletePending ? 'Đang xóa...' : 'Xóa workspace'}
            </Button>
          }
          title={`Xóa ${props.name}?`}
          description="Mọi dataset, file và thành viên của workspace này bị xóa. Không hoàn tác được."
          confirmLabel="Xóa workspace"
          onConfirm={props.onDelete}
        />
        {props.deleteError && <FormMessage kind="error">{props.deleteError}</FormMessage>}
      </div>
    </div>
  )
}
