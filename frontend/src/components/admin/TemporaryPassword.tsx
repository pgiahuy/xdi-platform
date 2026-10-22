// SPDX-License-Identifier: Apache-2.0
/* components/admin/TemporaryPassword.tsx - mật khẩu tạm vừa đặt lại, hiện một lần. Backend không lưu dạng chữ nên
không xem lại được; ẩn đi rồi thì muốn có mật khẩu khác phải đặt lại lần nữa. */

import { useState } from 'react'
import { Button } from '@/components/ui/button'

type Props = {
  email: string
  password: string
  onDismiss: () => void
}

export function TemporaryPassword({ email, password, onDismiss }: Props) {
  const [copied, setCopied] = useState<boolean>()

  // Trình duyệt chỉ cho chép khi trang chạy qua https hoặc localhost; ngoài hai trường hợp đó navigator.clipboard không
  // có và gọi vào sẽ ném lỗi ngay. Bọc trong Promise để mọi lỗi đi vào cùng một nhánh: báo người dùng chọn chữ và chép
  // tay.
  const copy = () => {
    Promise.resolve()
      .then(() => navigator.clipboard.writeText(password))
      .then(
        () => setCopied(true),
        () => setCopied(false),
      )
  }

  return (
    <div role="status" className="flex flex-col gap-3 rounded-lg border bg-muted/50 p-4">
      <p className="text-sm">
        Mật khẩu tạm của <span className="font-medium break-all">{email}</span>. Chuyển cho người dùng; ẩn đi rồi thì
        không xem lại được.
      </p>
      <code className="rounded-md border bg-background px-3 py-2 font-mono text-sm break-all select-all">
        {password}
      </code>
      <div className="flex flex-wrap items-center gap-2">
        <Button variant="outline" size="sm" onClick={copy}>
          Chép
        </Button>
        <Button variant="ghost" size="sm" onClick={onDismiss}>
          Ẩn mật khẩu
        </Button>
        {copied !== undefined && (
          <span className="text-xs text-muted-foreground">
            {copied ? 'Đã chép.' : 'Không chép được, hãy chọn và chép tay.'}
          </span>
        )}
      </div>
    </div>
  )
}
