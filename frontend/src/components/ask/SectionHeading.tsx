// SPDX-License-Identifier: Apache-2.0
/* components/ask/SectionHeading.tsx - hàng tiêu đề nhỏ của một phần trong câu trả lời (thẻ "Đã hiểu là", bảng kết
quả) kèm nút mở hay thu gọn phần đó. Theo mẫu disclosure: nút nằm trong thẻ tiêu đề, mang aria-expanded và
aria-controls trỏ tới phần thân, nên trình đọc màn hình đọc được cả tên phần lẫn trạng thái mở. Cả hàng chữ là nút, không
chỉ mũi tên, để bấm dễ trên màn cảm ứng. Trạng thái mở giữ bằng useSectionOpen (sectionOpen.ts). */

import type { ReactNode } from 'react'
import { ChevronDown } from 'lucide-react'
import { cn } from '@/lib/utils'

type Props = {
  id: string
  open: boolean
  onToggle: () => void
  /** id của phần thân bị ẩn hay hiện. */
  controls: string
  className?: string
  children: ReactNode
}

export function SectionHeading({ id, open, onToggle, controls, className, children }: Props) {
  return (
    <h3 id={id} className={className}>
      <button
        type="button"
        aria-expanded={open}
        aria-controls={controls}
        onClick={onToggle}
        className="-mx-1 flex items-center gap-2 rounded px-1 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        {children}
        <ChevronDown aria-hidden className={cn('size-4 shrink-0 transition-transform', !open && '-rotate-90')} />
      </button>
    </h3>
  )
}
