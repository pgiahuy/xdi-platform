// SPDX-License-Identifier: Apache-2.0
/* components/ConfirmDialog.tsx - hỏi lại trước một việc không hoàn tác được: xóa workspace, xóa thành viên, rời
workspace, tự hạ vai, khóa tài khoản. Hai cách dùng: có trigger (nút mở hộp hỏi), hoặc không có trigger mà trang tự mở
qua open và onOpenChange (ví dụ khi đổi ô chọn vai). onConfirm chạy khi người dùng bấm nút xác nhận. Lỗi của việc đó
hiện ở trang, không ở đây, vì hộp đóng ngay khi bấm. */

import type { ReactNode } from 'react'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog'

type Props = {
  trigger?: ReactNode
  open?: boolean
  onOpenChange?: (open: boolean) => void
  /** Không có trigger thì Radix không biết trả focus về đâu khi đóng; trang dùng chỗ này để đặt focus. */
  onCloseAutoFocus?: (event: Event) => void
  title: string
  description: string
  confirmLabel: string
  onConfirm: () => void
}

export function ConfirmDialog(props: Props) {
  const { trigger, title, description, confirmLabel, onConfirm } = props
  return (
    <AlertDialog open={props.open} onOpenChange={props.onOpenChange}>
      {trigger && <AlertDialogTrigger asChild>{trigger}</AlertDialogTrigger>}
      <AlertDialogContent onCloseAutoFocus={props.onCloseAutoFocus}>
        <AlertDialogHeader>
          <AlertDialogTitle>{title}</AlertDialogTitle>
          <AlertDialogDescription>{description}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Thôi</AlertDialogCancel>
          <AlertDialogAction variant="destructive" onClick={onConfirm}>
            {confirmLabel}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
