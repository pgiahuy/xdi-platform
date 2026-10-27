// SPDX-License-Identifier: Apache-2.0
/* components/shell/CreateWorkspaceDialog.tsx - hộp thoại nhập tên để tạo workspace mới. Mở từ mục cuối của menu đổi
workspace (WorkspaceSwitcher qua AppShell), hoặc từ nút trên màn của người chưa thuộc workspace nào (RootPage).
Hai cách dùng như ConfirmDialog: có trigger (nút mở hộp), hoặc không có trigger mà trang tự mở qua open và onOpenChange.
Hộp không gọi API: trang truyền onCreate, trạng thái chờ và lỗi từ backend; tạo xong trang đóng hộp và chuyển trang. */

import { useState, type FormEvent, type ReactNode } from 'react'
import { FormField } from '@/components/forms/FormField'
import { FormMessage } from '@/components/forms/FormMessage'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog'

type Props = {
  trigger?: ReactNode
  open?: boolean
  onOpenChange?: (open: boolean) => void
  onCreate: (name: string) => void
  pending: boolean
  /** Lỗi backend trả về, hiện nguyên lời, ví dụ tên quá dài. */
  error?: string
}

export function CreateWorkspaceDialog({ trigger, open, onOpenChange, ...formProps }: Props) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {trigger && <DialogTrigger asChild>{trigger}</DialogTrigger>}
      {/* Nút đóng góc trên của shadcn có chữ tiếng Anh cho trình đọc màn hình; dùng nút Hủy ở chân hộp thay cho nó. */}
      <DialogContent showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>Tạo workspace</DialogTitle>
          <DialogDescription>Bạn là admin của workspace mới: nạp dữ liệu và thêm thành viên ở đó.</DialogDescription>
        </DialogHeader>
        {/* Form nằm trong DialogContent, mà Radix gỡ nội dung khi hộp đóng: mỗi lần mở lại là ô tên trống, không còn
            tên hay lỗi của lần trước. */}
        <CreateWorkspaceForm {...formProps} />
      </DialogContent>
    </Dialog>
  )
}

const NAME_ID = 'create-workspace-name'

function CreateWorkspaceForm({ onCreate, pending, error }: Omit<Props, 'trigger' | 'open' | 'onOpenChange'>) {
  const [name, setName] = useState('')
  const [nameError, setNameError] = useState<string>()

  // Nút không bị disabled lúc chờ (chặn bấm trùng ở đây): disabled làm focus rơi khỏi nút.
  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (pending) return
    const trimmed = name.trim()
    // Chỉ kiểm tên trống ở đây; độ dài tối đa và các quy tắc khác do backend kiểm và báo lời.
    if (!trimmed) {
      setNameError('Nhập tên workspace')
      document.getElementById(NAME_ID)?.focus()
      return
    }
    setNameError(undefined)
    onCreate(trimmed)
  }

  return (
    <form onSubmit={handleSubmit} noValidate className="flex flex-col gap-4">
      <FormField
        id={NAME_ID}
        label="Tên workspace"
        required
        autoComplete="off"
        error={nameError}
        value={name}
        onChange={(e) => setName(e.target.value)}
      />
      {error && <FormMessage kind="error">{error}</FormMessage>}
      <DialogFooter>
        <DialogClose asChild>
          <Button type="button" variant="outline">
            Hủy
          </Button>
        </DialogClose>
        <Button type="submit" aria-disabled={pending}>
          {pending ? 'Đang tạo...' : 'Tạo workspace'}
        </Button>
      </DialogFooter>
    </form>
  )
}
