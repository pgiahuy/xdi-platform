// SPDX-License-Identifier: Apache-2.0
/* components/ask/EvidenceSteps.tsx - tab Các bước của pane bằng chứng: những gì backend đã làm, theo đúng thứ tự, mỗi
bước một dòng có thời gian, đạt hay lỗi, chi tiết. Bước có gọi mô hình ngôn ngữ ghi rõ, để thấy mô hình chỉ viết SQL
và lời, còn số do code tính. Bước phân loại và bước viết lời thay nhãn "có mô hình" bằng cờ màu: loại câu
(Quantitative, Causal) và kết luận hai cổng kiểm (#201: match, warning, mismatch); mô hình chấm không đúng tuyệt
đối, nên người đọc cần thấy nó đã kết luận gì. Một bước có thể lặp lại (run trước và sau repair), nên khóa theo vị trí.
Vẽ như dòng thời gian: mỗi bước một chấm tròn có dấu tick xanh lá (đỏ khi lỗi) nối bằng vạch dọc. */

import { CircleAlert, CircleCheck } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { StepFlag, StepView } from './evidenceTypes'

type Props = {
  steps: StepView[]
}

const msFormat = new Intl.NumberFormat('vi-VN')

// Cờ nhận ra nhanh bằng màu: loại câu xanh ngọc (Quantitative) hay xanh dương (Causal); kết luận đối soát xanh lá
// (match), hổ phách (warning), đỏ (mismatch). Chữ trên cờ kết luận là đúng tên mô hình chấm trả, để đối chiếu với log.
const flags: Record<StepFlag, { text: string; className: string }> = {
  quantitative: { text: 'Quantitative', className: 'border-label-border bg-label-soft text-label' },
  causal: { text: 'Causal', className: 'border-info-border bg-info-soft text-info-text' },
  match: { text: 'match', className: 'border-increase/40 bg-increase/10 text-increase' },
  warning: { text: 'warning', className: 'border-highlight-border bg-highlight-soft text-highlight-text' },
  mismatch: { text: 'mismatch', className: 'border-destructive/30 bg-destructive/10 text-destructive' },
}

export function EvidenceSteps({ steps }: Props) {
  return (
    <ol className="flex flex-col">
      {steps.map((step, index) => (
        <li
          key={index}
          className="relative flex gap-2.5 pb-3.5 before:absolute before:top-6 before:bottom-0.5 before:left-[11px] before:w-0.5 before:bg-label-border last:before:hidden"
        >
          <span
            aria-hidden
            className={cn(
              'flex size-6 shrink-0 items-center justify-center rounded-full border',
              step.ok ? 'border-increase/40 bg-increase/10 text-increase' : 'border-destructive/30 bg-destructive/10 text-destructive',
            )}
          >
            {step.ok ? <CircleCheck className="size-3.5" /> : <CircleAlert className="size-3.5" />}
          </span>
          <div className="flex min-w-0 flex-col gap-0.5 pt-0.5">
            <p className="flex flex-wrap items-baseline gap-x-2 text-sm">
              <span className={cn('font-medium', !step.ok && 'text-destructive')}>{step.label}</span>
              <span className="text-xs text-muted-foreground tabular-nums">{msFormat.format(step.durationMs)} ms</span>
              {/* Bước đạt chỉ cần dấu tick xanh lá; chữ "đạt" vẫn có cho trình đọc màn hình. Bước lỗi giữ chữ hiện ra, để
              người mù màu phân biệt được với bước đạt. */}
              {step.ok ? (
                <span className="sr-only">đạt</span>
              ) : (
                <span className="text-xs text-destructive">lỗi</span>
              )}
              {step.flag ? (
                <span className={cn('rounded border px-1.5 text-xs font-medium', flags[step.flag].className)}>
                  {flags[step.flag].text}
                </span>
              ) : (
                step.byModel && <span className="rounded border px-1 text-xs text-muted-foreground">có mô hình</span>
              )}
            </p>
            {step.detail && <p className="text-xs break-words text-muted-foreground">{step.detail}</p>}
          </div>
        </li>
      ))}
    </ol>
  )
}
