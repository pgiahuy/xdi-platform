// SPDX-License-Identifier: Apache-2.0
/* components/report/ReportChecks.tsx - kết quả hai bước kiểm của lời trả lời một mục: kiểm số (code so mọi số và chiều
tăng giảm của lời với các ý đã tính) và kiểm nghĩa (một lần gọi mô hình riêng so nghĩa của lời với các ý đó). Icon đi
kèm chữ để quét nhanh, chữ vẫn nói đủ nghĩa khi in đen trắng.

Lời đang hiện là của mô hình thì ghi hai bước. Lời đang hiện là câu hệ thống tính thì ghi một dòng: câu đó không cần
kiểm, hay nó thay cho lời của mô hình đã trượt bước nào. Kết luận "không khớp" của kiểm nghĩa thuộc lời của mô hình đã bị
thay, nên không bao giờ hiện cạnh câu của hệ thống như thể câu đó sai. */

import { CircleCheck, CircleMinus, TriangleAlert, type LucideIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { MeaningCheckView, NumberCheckView } from './types'

type Props = {
  /** True khi lời đang hiện do mô hình viết. */
  byModel: boolean
  numberCheck: NumberCheckView | null
  meaningCheck: MeaningCheckView | null
}

type Shown = { label: string; icon: LucideIcon; tone: string }

const meaningShown: Record<MeaningCheckView, Shown> = {
  match: { label: 'Khớp', icon: CircleCheck, tone: 'text-label' },
  warning: { label: 'Có chỗ chưa rõ', icon: TriangleAlert, tone: 'text-highlight-text' },
  // Lời của mô hình đang hiện thì không có kết luận mismatch (backend đã thay nó); giữ cho đủ kiểu.
  mismatch: { label: 'Không đạt', icon: CircleMinus, tone: 'text-muted-foreground' },
}

const passed: Shown = { label: 'Đạt', icon: CircleCheck, tone: 'text-label' }
const notRun: Shown = { label: 'Không chạy', icon: CircleMinus, tone: 'text-muted-foreground' }

export function ReportChecks({ byModel, numberCheck, meaningCheck }: Props) {
  if (numberCheck === null) return null
  if (byModel) {
    return (
      <dl className="flex flex-wrap gap-x-6 gap-y-1 text-xs">
        <Check name="Kiểm số" shown={passed} />
        <Check name="Kiểm nghĩa" shown={meaningCheck ? meaningShown[meaningCheck] : notRun} />
      </dl>
    )
  }
  return (
    <p className="flex items-center gap-1 text-xs text-muted-foreground">
      <CircleMinus aria-hidden className="size-3.5 shrink-0" />
      {describeSystemText(numberCheck, meaningCheck)}
    </p>
  )
}

/** Một dòng cho câu hệ thống tính: không cần kiểm, hay nó thay lời của mô hình đã trượt bước nào. */
function describeSystemText(numberCheck: NumberCheckView, meaningCheck: MeaningCheckView | null): string {
  if (numberCheck === 'not_needed') return 'Câu do hệ thống viết từ số đã tính, không cần kiểm'
  const failedNumber = numberCheck === 'replaced'
  const failedMeaning = meaningCheck === 'mismatch'
  // Lời lưu trước khi backend ghi bước nào chặn lời có cả hai cờ: không biết bước nào.
  const step = failedNumber && failedMeaning ? 'kiểm số hoặc kiểm nghĩa' : failedNumber ? 'kiểm số' : 'kiểm nghĩa'
  return `Lời của mô hình không qua ${step}, đã thay bằng câu hệ thống tính`
}

function Check({ name, shown }: { name: string; shown: Shown }) {
  const Icon = shown.icon
  return (
    <div className="flex items-center gap-1.5">
      <dt className="font-semibold">{name}:</dt>
      <dd className="flex items-center gap-1">
        <Icon aria-hidden className={cn('size-3.5 shrink-0', shown.tone)} />
        {shown.label}
      </dd>
    </div>
  )
}
