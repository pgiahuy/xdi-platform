// SPDX-License-Identifier: Apache-2.0
/* components/forms/FormMessage.tsx - dòng báo kết quả của một form: lỗi (đỏ) hoặc đã xong.
role alert và status để trình đọc màn hình đọc ngay khi dòng này hiện, không cần người dùng tìm. */

type Props = {
  kind: 'error' | 'success'
  children: string
}

export function FormMessage({ kind, children }: Props) {
  if (kind === 'error') {
    return (
      <p role="alert" className="rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive">
        {children}
      </p>
    )
  }
  return (
    <p role="status" className="rounded-md bg-muted px-3 py-2 text-sm text-foreground">
      {children}
    </p>
  )
}
