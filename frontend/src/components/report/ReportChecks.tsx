// SPDX-License-Identifier: Apache-2.0
/* components/report/ReportChecks.tsx - kết quả hai bước kiểm của lời trả lời một mục: kiểm số (code so mọi số và chiều
tăng giảm của lời với các ý đã tính) và kiểm nghĩa (một lần gọi mô hình riêng so nghĩa của lời với các ý đó). Icon đi
kèm chữ để quét nhanh, chữ vẫn nói đủ nghĩa khi in đen trắng. */

import { CircleCheck, CircleMinus, CircleX, TriangleAlert, type LucideIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { MeaningCheckView, NumberCheckView } from './types'

type Props = {
  numberCheck: NumberCheckView | null
  meaningCheck: MeaningCheckView | null
}

type Shown = { label: string; icon: LucideIcon; tone: string }

const numberShown: Record<NumberCheckView, Shown> = {
  passed: { label: 'Đạt', icon: CircleCheck, tone: 'text-label' },
  replaced: { label: 'Lời của mô hình không đạt, đã thay bằng câu của hệ thống', icon: CircleX, tone: 'text-destructive' },
  not_needed: { label: 'Không cần: câu do hệ thống viết từ số đã tính', icon: CircleMinus, tone: 'text-muted-foreground' },
}

const meaningShown: Record<MeaningCheckView, Shown> = {
  match: { label: 'Khớp', icon: CircleCheck, tone: 'text-label' },
  warning: { label: 'Có chỗ chưa rõ', icon: TriangleAlert, tone: 'text-highlight-text' },
  mismatch: { label: 'Không khớp', icon: CircleX, tone: 'text-destructive' },
}

const notRun: Shown = { label: 'Không chạy', icon: CircleMinus, tone: 'text-muted-foreground' }

export function ReportChecks({ numberCheck, meaningCheck }: Props) {
  if (numberCheck === null) return null
  return (
    <dl className="flex flex-wrap gap-x-6 gap-y-1 text-xs">
      <Check name="Kiểm số" shown={numberShown[numberCheck]} />
      <Check name="Kiểm nghĩa" shown={meaningCheck ? meaningShown[meaningCheck] : notRun} />
    </dl>
  )
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
