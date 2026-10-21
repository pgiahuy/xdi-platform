// SPDX-License-Identifier: Apache-2.0
/* components/StatusScreen.tsx - cả màn hình cho lúc chưa hiện được khung giao diện: đang tải, chưa đăng nhập,
không có workspace, lỗi. footer nhận thêm nội dung phụ, ví dụ phiên bản máy chủ. */

import type { ReactNode } from 'react'
import { EmptyState } from './EmptyState'

type Props = {
  title: string
  description?: string
  action?: ReactNode
  footer?: ReactNode
  busy?: boolean
}

export function StatusScreen({ title, description, action, footer, busy = false }: Props) {
  return (
    <main aria-busy={busy} className="flex min-h-svh flex-col justify-center px-4 py-10">
      {/* React 19 đưa thẻ title lên head của trang: tab trình duyệt hiện đúng trạng thái. */}
      <title>{title}</title>
      <div className="mx-auto w-full max-w-md">
        <EmptyState heading="h1" title={title} description={description} action={action} />
        {footer && <div className="mt-10 border-t pt-4">{footer}</div>}
      </div>
    </main>
  )
}
