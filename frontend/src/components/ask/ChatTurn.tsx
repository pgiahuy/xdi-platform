// SPDX-License-Identifier: Apache-2.0
/* components/ask/ChatTurn.tsx - một lượt của khung trò chuyện trên trang hỏi (#164): bong bóng câu hỏi của người dùng
căn phải, khối trả lời của hệ thống căn trái và rộng hết cột. Câu hỏi là tiêu đề h2 của lượt, để trình đọc màn hình đi
từ lượt này sang lượt khác bằng phím tiêu đề như các thẻ khác của trang. Nội dung khối trả lời do trang truyền vào
(câu trả lời, lượt đang chờ, lượt đã lưu).
Nút "Bằng chứng" chỉ có ở lượt có câu trả lời của phiên này; lượt đang chọn có viền màu chính để biết pane bằng chứng
đang nói về lượt nào. */

import { useId, type ReactNode, type Ref } from 'react'
import { ShieldCheck } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

type Props = {
  question: string
  children: ReactNode
  /** Pane bằng chứng đang hiện của lượt này: viền màu chính, nút bằng chứng ở trạng thái nhấn. */
  selected?: boolean
  onShowEvidence?: () => void
  /** Trang cuộn tới lượt mới bằng ref của khung lượt (React 19 nhận ref như một prop thường). */
  ref?: Ref<HTMLElement>
}

export function ChatTurn({ question, children, selected = false, onShowEvidence, ref }: Props) {
  const headingId = useId()
  return (
    <article ref={ref} aria-labelledby={headingId} className="flex scroll-mt-20 flex-col gap-3">
      <div className="flex justify-end">
        <h2
          id={headingId}
          className="max-w-[85%] rounded-2xl rounded-br-md bg-sidebar px-4 py-2.5 text-[15px] leading-snug font-medium wrap-break-word whitespace-pre-wrap text-sidebar-foreground"
        >
          {question}
        </h2>
      </div>
      <div
        className={cn(
          'flex min-w-0 flex-col gap-4 rounded-xl border bg-card p-4 text-card-foreground shadow-xs sm:px-6 sm:py-5',
          selected && 'ring-2 ring-primary/70',
        )}
      >
        {children}
        {onShowEvidence && (
          <div className="flex justify-end">
            <Button variant="outline" size="sm" aria-pressed={selected} onClick={onShowEvidence}>
              <ShieldCheck aria-hidden />
              Bằng chứng
            </Button>
          </div>
        )}
      </div>
    </article>
  )
}
