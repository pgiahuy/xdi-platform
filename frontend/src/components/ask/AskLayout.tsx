// SPDX-License-Identifier: Apache-2.0
/* components/ask/AskLayout.tsx - bố cục trang hỏi: cột chính (ô hỏi, thẻ "Đã hiểu là", kết quả) và cột bằng chứng
bên phải trên màn rộng, xuống dưới cột chính trên màn hẹp. Cột bằng chứng là chỗ của pane bằng chứng (#52: SQL, phân
rã, dòng nguồn, các bước); trang truyền nội dung vào, không có thì cột không chiếm chỗ. */

import type { ReactNode } from 'react'

type Props = {
  children: ReactNode
  evidence?: ReactNode
}

export function AskLayout({ children, evidence }: Props) {
  return (
    <div className="grid grid-cols-1 gap-6 xl:grid-cols-[minmax(0,1fr)_20rem]">
      <div className="flex min-w-0 flex-col gap-6">{children}</div>
      {evidence && (
        <aside aria-labelledby="evidence-heading" className="flex min-w-0 flex-col gap-2 xl:border-l xl:pl-6">
          <h2 id="evidence-heading" className="text-sm font-semibold">
            Bằng chứng
          </h2>
          {evidence}
        </aside>
      )}
    </div>
  )
}
