// SPDX-License-Identifier: Apache-2.0
/* components/auth/AuthScreen.tsx - bố cục của màn trước khi vào workspace: đăng nhập, thiết lập lần đầu.
Màn rộng chia hai: panel thương hiệu nền mực bên trái (cùng màu thanh bên) kể ngắn sản phẩm làm gì, form trong thẻ
trắng bên phải.
Màn hẹp xếp dọc: dải thương hiệu gọn ở trên, form dưới; dùng được từ 320px. footer nhận nội dung phụ, ví dụ phiên bản
máy chủ. */

import type { ReactNode } from 'react'
import { LineChart, MessageSquareText, ShieldCheck } from 'lucide-react'

type Props = {
  title: string
  description?: string
  children: ReactNode
  footer?: ReactNode
}

const highlights = [
  { icon: MessageSquareText, text: 'Hỏi dữ liệu bằng tiếng Việt tự nhiên' },
  { icon: LineChart, text: 'Mọi con số kèm bằng chứng, bấm là thấy dòng nguồn' },
  { icon: ShieldCheck, text: 'Guard SQL chỉ cho đọc, phân quyền tới từng dòng' },
]

export function AuthScreen({ title, description, children, footer }: Props) {
  return (
    <main className="grid min-h-svh grid-rows-[auto_1fr] lg:grid-cols-2 lg:grid-rows-1">
      {/* React 19 đưa thẻ title lên head của trang. */}
      <title>{title}</title>

      {/* Panel thương hiệu: màn hẹp chỉ còn một dải gọn ở trên. */}
      <div className="relative overflow-hidden bg-sidebar text-sidebar-foreground">
        <div className="relative flex items-center gap-2.5 p-6 lg:p-10">
          <img src="/favicon.svg" alt="" className="size-8" />
          <span className="text-lg font-semibold tracking-tight">xdi-platform</span>
        </div>
        <div className="relative hidden px-10 pb-16 lg:block">
          <p className="max-w-md text-3xl leading-snug font-semibold tracking-tight">
            Hỏi dữ liệu bằng ngôn ngữ thường, nhận câu trả lời có bằng chứng.
          </p>
          <ul className="mt-10 flex max-w-md flex-col gap-4">
            {highlights.map(({ icon: Icon, text }) => (
              <li key={text} className="flex items-center gap-3 text-sm text-sidebar-muted">
                <Icon className="size-5 shrink-0 text-sidebar-primary" aria-hidden />
                {text}
              </li>
            ))}
          </ul>
        </div>
      </div>

      <div className="flex flex-col items-center justify-center bg-background px-4 py-10">
        <div className="w-full max-w-sm rounded-xl border bg-card p-6 shadow-sm sm:p-8">
          <h1 className="text-xl font-semibold tracking-tight">{title}</h1>
          {description && <p className="mt-1 text-sm text-muted-foreground">{description}</p>}
          <div className="mt-6">{children}</div>
        </div>
        {footer && <div className="mt-6 w-full max-w-sm px-1 text-xs text-muted-foreground">{footer}</div>}
      </div>
    </main>
  )
}
