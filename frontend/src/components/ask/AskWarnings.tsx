// SPDX-License-Identifier: Apache-2.0
/* components/ask/AskWarnings.tsx - cảnh báo đi kèm câu trả lời (kiểm hợp lý #36, lựa chọn không áp dụng được...). Cảnh
báo không che kết quả: hiện thành một khối riêng phía trên, kết quả vẫn hiện bên dưới. Lời do backend viết. */

import { TriangleAlert } from 'lucide-react'

type Props = {
  messages: string[]
}

export function AskWarnings({ messages }: Props) {
  if (messages.length === 0) return null
  return (
    <section aria-label="Lưu ý" className="flex gap-2 rounded-lg border px-3 py-2 text-sm">
      {/* Icon trạng thái đứng cạnh chữ, chữ đã nói đủ nên trình đọc màn hình bỏ qua icon. */}
      <TriangleAlert aria-hidden className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
      <ul className="flex flex-col gap-1">
        {/* key theo vị trí: hai cảnh báo có thể trùng chữ. Danh sách chỉ thay cả khối theo từng response. */}
        {messages.map((message, i) => (
          <li key={i}>{message}</li>
        ))}
      </ul>
    </section>
  )
}
