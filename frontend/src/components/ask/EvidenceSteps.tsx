// SPDX-License-Identifier: Apache-2.0
/* components/ask/EvidenceSteps.tsx - tab Các bước của pane bằng chứng: những gì backend đã làm, theo đúng thứ tự, mỗi
bước một dòng có thời gian, đạt hay lỗi, chi tiết. Bước có gọi mô hình ngôn ngữ ghi rõ, để thấy mô hình chỉ viết SQL
và lời, còn số do code tính. Một bước có thể lặp lại (run trước và sau repair), nên khóa theo vị trí. */

import { CircleAlert, CircleCheck } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { StepView } from './evidenceTypes'

type Props = {
  steps: StepView[]
}

const msFormat = new Intl.NumberFormat('vi-VN')

export function EvidenceSteps({ steps }: Props) {
  return (
    <ol className="flex flex-col">
      {steps.map((step, index) => (
        <li key={index} className="flex gap-2 border-l py-1.5 pl-3">
          {step.ok ? (
            <CircleCheck aria-hidden className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
          ) : (
            <CircleAlert aria-hidden className="mt-0.5 size-4 shrink-0 text-destructive" />
          )}
          <div className="flex min-w-0 flex-col gap-0.5">
            <p className="flex flex-wrap items-baseline gap-x-2 text-sm">
              <span className={cn('font-medium', !step.ok && 'text-destructive')}>{step.label}</span>
              <span className="text-xs text-muted-foreground tabular-nums">{msFormat.format(step.durationMs)} ms</span>
              {/* Trạng thái bằng chữ, không chỉ bằng màu và icon: trình đọc màn hình đọc được, người mù màu phân biệt
              được. */}
              <span className={cn('text-xs', step.ok ? 'text-muted-foreground' : 'text-destructive')}>
                {step.ok ? 'đạt' : 'lỗi'}
              </span>
              {step.byModel && <span className="rounded border px-1 text-xs text-muted-foreground">có mô hình</span>}
            </p>
            {step.detail && <p className="text-xs break-words text-muted-foreground">{step.detail}</p>}
          </div>
        </li>
      ))}
    </ol>
  )
}
