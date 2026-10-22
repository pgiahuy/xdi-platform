// SPDX-License-Identifier: Apache-2.0
/* components/auth/AuthScreen.tsx - bố cục của màn trước khi vào workspace: đăng nhập, thiết lập lần đầu.
Một cột hẹp giữa màn, dùng được từ bề ngang 320px. footer nhận nội dung phụ, ví dụ phiên bản máy chủ. */

import type { ReactNode } from 'react'

type Props = {
  title: string
  description?: string
  children: ReactNode
  footer?: ReactNode
}

export function AuthScreen({ title, description, children, footer }: Props) {
  return (
    <main className="flex min-h-svh flex-col justify-center px-4 py-10">
      {/* React 19 đưa thẻ title lên head của trang. */}
      <title>{title}</title>
      <div className="mx-auto w-full max-w-sm">
        <h1 className="text-lg font-semibold">{title}</h1>
        {description && <p className="mt-1 text-sm text-muted-foreground">{description}</p>}
        <div className="mt-6">{children}</div>
        {footer && <div className="mt-10 border-t pt-4">{footer}</div>}
      </div>
    </main>
  )
}
