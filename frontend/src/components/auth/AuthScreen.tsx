// SPDX-License-Identifier: Apache-2.0
/* components/auth/AuthScreen.tsx - bố cục của màn trước khi vào workspace: đăng nhập, thiết lập lần đầu.
Thẻ giữa màn trên nền muted nhạt, có dấu hiệu nhận diện sản phẩm (cùng favicon.svg). Dùng được từ bề ngang 320px.
footer nhận nội dung phụ, ví dụ phiên bản máy chủ. */

import type { ReactNode } from 'react'

type Props = {
  title: string
  description?: string
  children: ReactNode
  footer?: ReactNode
}

export function AuthScreen({ title, description, children, footer }: Props) {
  return (
    <main className="flex min-h-svh flex-col items-center justify-center bg-muted/40 px-4 py-10">
      {/* React 19 đưa thẻ title lên head của trang. */}
      <title>{title}</title>
      <div className="mb-6 flex items-center gap-2.5">
        {/* Dấu hiệu nhận diện: cùng file với favicon để không phải vẽ lại. */}
        <img src="/favicon.svg" alt="" className="size-8" />
        <span className="text-lg font-semibold tracking-tight">xdi-platform</span>
      </div>
      <div className="w-full max-w-sm rounded-xl border bg-card p-6 shadow-sm sm:p-8">
        <h1 className="text-lg font-semibold">{title}</h1>
        {description && <p className="mt-1 text-sm text-muted-foreground">{description}</p>}
        <div className="mt-6">{children}</div>
      </div>
      {footer && <div className="mt-6 text-center text-xs text-muted-foreground">{footer}</div>}
    </main>
  )
}
