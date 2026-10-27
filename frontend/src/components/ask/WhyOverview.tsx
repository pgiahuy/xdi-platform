// SPDX-License-Identifier: Apache-2.0
/* components/ask/WhyOverview.tsx - phần đầu câu trả lời của câu "tại sao": ba số lớn (kỳ gốc, kỳ so sánh, chênh lệch),
lời giải thích, rồi đóng góp của từng nhóm theo chiều giải thích tốt nhất (ContributionBars.tsx). Mọi số lấy từ
explanation của response, cùng dữ liệu với tab Phân rã của pane bằng chứng; ở đây chỉ hiện gọn để đọc câu trả lời mà
không phải mở pane. Số là chuỗi chữ số của backend, chỉ nhóm nghìn (formatDecimal), không làm tròn.
Chênh lệch luôn kèm dấu và chữ "tăng" hay "giảm": không để màu một mình nói chiều thay đổi. */

import { TrendingDown, TrendingUp } from 'lucide-react'
import { cn } from '@/lib/utils'
import { AnswerSummary } from './AnswerSummary'
import { formatDecimal } from './cellFormat'
import { ContributionBars } from './ContributionBars'
import { changeDirection, formatSigned } from './changeFormat'
import type { BreakdownView } from './evidenceTypes'

type Props = {
  breakdown: BreakdownView
}

export function WhyOverview({ breakdown: b }: Props) {
  const first = b.dimensions[0]
  return (
    <>
      <dl className="grid grid-cols-1 gap-3 tabular-nums sm:grid-cols-3">
        <Stat label={b.baseLabel} value={formatDecimal(b.baseValue)} note="Kỳ gốc" />
        <Stat label={b.compareLabel} value={formatDecimal(b.compareValue)} note="Kỳ so sánh" />
        <ChangeStat change={b.change} baseLabel={b.baseLabel} />
      </dl>
      <AnswerSummary summary={b.summary} byModel={b.summaryByModel} />
      {first && first.groups.length > 0 && (
        <ContributionBars dimension={first} baseLabel={b.baseLabel} compareLabel={b.compareLabel} />
      )}
    </>
  )
}

function Stat({ label, value, note }: { label: string; value: string; note: string }) {
  return (
    <div className="flex flex-col rounded-[10px] border bg-card px-4 py-3">
      <dt className="text-xs font-medium text-muted-foreground">{label}</dt>
      <dd className="mt-1 text-xl font-bold tracking-tight break-all">{value}</dd>
      <dd className="text-xs text-muted-foreground">{note}</dd>
    </div>
  )
}

function ChangeStat({ change, baseLabel }: { change: string; baseLabel: string }) {
  const direction = changeDirection(change)
  const Icon = direction === 'down' ? TrendingDown : TrendingUp
  const word = direction === 'down' ? 'giảm' : direction === 'up' ? 'tăng' : 'không đổi'
  return (
    <div className="flex flex-col rounded-[10px] border border-highlight-border bg-highlight-soft px-4 py-3">
      <dt className="text-xs font-medium text-muted-foreground">Chênh lệch</dt>
      <dd className="mt-1 text-xl font-bold tracking-tight break-all text-highlight-text">{formatSigned(change)}</dd>
      <dd className="mt-1">
        <span
          className={cn(
            'inline-flex items-center gap-1 rounded-full border px-2 text-xs font-semibold',
            direction === 'down'
              ? 'border-highlight-border bg-card text-highlight-text'
              : 'border-label-border bg-card text-label',
          )}
        >
          {direction !== 'flat' && <Icon aria-hidden className="size-3.5" />}
          {word} so với {baseLabel}
        </span>
      </dd>
    </div>
  )
}
