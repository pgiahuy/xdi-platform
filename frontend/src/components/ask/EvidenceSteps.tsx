// SPDX-License-Identifier: Apache-2.0
/* components/ask/EvidenceSteps.tsx - tab Các bước của pane bằng chứng: những gì backend đã làm, theo đúng thứ tự, mỗi
bước một dòng có thời gian, đạt hay lỗi, chi tiết. Bước có gọi mô hình ngôn ngữ ghi rõ, để thấy mô hình chỉ viết SQL
và lời, còn số do code tính. Một bước có thể lặp lại (run trước và sau repair), nên khóa theo vị trí.
Vẽ như dòng thời gian: mỗi bước một chấm tròn xanh ngọc (đỏ khi lỗi) nối bằng vạch dọc. Dưới dòng thời gian là các lần
gọi mô hình của câu (#184): loại việc, thời gian, số token, để thấy thời gian chờ nằm ở mô hình hay ở backend. */

import { CircleAlert, CircleCheck } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { ModelCallView, StepView } from './evidenceTypes'

type Props = {
  steps: StepView[]
  calls: ModelCallView[]
}

const msFormat = new Intl.NumberFormat('vi-VN')

export function EvidenceSteps({ steps, calls }: Props) {
  return (
    <div className="flex flex-col gap-2">
      <StepList steps={steps} />
      {calls.length > 0 && <ModelCalls calls={calls} />}
    </div>
  )
}

function StepList({ steps }: { steps: StepView[] }) {
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
              step.ok ? 'border-label-border bg-label-soft text-label' : 'border-destructive/30 bg-destructive/10 text-destructive',
            )}
          >
            {step.ok ? <CircleCheck className="size-3.5" /> : <CircleAlert className="size-3.5" />}
          </span>
          <div className="flex min-w-0 flex-col gap-0.5 pt-0.5">
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

function buildTokenText(input: number | null, output: number | null): string | null {
  if (input === null || output === null) return null
  return `${msFormat.format(input)} token vào, ${msFormat.format(output)} token ra`
}

function ModelCalls({ calls }: { calls: ModelCallView[] }) {
  const totalMs = calls.reduce((sum, call) => sum + call.durationMs, 0)
  return (
    <section className="flex flex-col gap-1 border-t pt-2">
      <h3 className="text-sm font-medium">
        Lượt gọi mô hình: {calls.length}, tổng {msFormat.format(totalMs)} ms
      </h3>
      <ul className="flex flex-col gap-0.5">
        {calls.map((call, index) => {
          const tokens = buildTokenText(call.inputTokens, call.outputTokens)
          return (
            <li key={index} className="flex flex-wrap items-baseline gap-x-2 text-xs text-muted-foreground">
              <span className={cn('text-foreground', !call.ok && 'text-destructive')}>{call.label}</span>
              <span className="tabular-nums">{msFormat.format(call.durationMs)} ms</span>
              {!call.ok && <span className="text-destructive">lỗi</span>}
              {tokens && <span className="tabular-nums">{tokens}</span>}
            </li>
          )
        })}
      </ul>
    </section>
  )
}
