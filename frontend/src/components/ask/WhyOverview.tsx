// SPDX-License-Identifier: Apache-2.0
/* components/ask/WhyOverview.tsx - phần đầu câu trả lời của câu "tại sao": một hàng ba số (kỳ gốc, kỳ so sánh, chênh
lệch) không khung riêng, để thẻ trả lời không thành nhiều khung lồng nhau,
lời giải thích, rồi đóng góp của từng nhóm theo chiều giải thích tốt nhất (ContributionBars.tsx). Mọi số lấy từ
explanation của response, cùng dữ liệu với tab Phân rã của pane bằng chứng; ở đây chỉ hiện gọn để đọc câu trả lời mà
không phải mở pane. Số là chuỗi chữ số của backend, chỉ nhóm nghìn (formatDecimal), không làm tròn.
Chênh lệch luôn kèm dấu và chữ chiều ("tăng", "giảm" giữa hai kỳ; "cao hơn", "thấp hơn" giữa hai nhóm): không để màu
một mình nói chiều thay đổi. */

import { TrendingDown, TrendingUp } from 'lucide-react'
import { cn } from '@/lib/utils'
import { AnswerSummary } from './AnswerSummary'
import { formatDecimal } from './cellFormat'
import { ContributionBars } from './ContributionBars'
import { changeDirection, directionWord, formatSigned } from './changeFormat'
import type { BreakdownView, Comparison } from './evidenceTypes'

type Props = {
  breakdown: BreakdownView
}

export function WhyOverview({ breakdown: b }: Props) {
  const first = b.dimensions[0]
  return (
    <>
      <dl className="flex flex-wrap gap-x-8 gap-y-3 tabular-nums">
        <Stat label={b.baseLabel} value={formatDecimal(b.baseValue)} />
        <Stat label={b.compareLabel} value={formatDecimal(b.compareValue)} />
        <ChangeStat change={b.change} baseLabel={b.baseLabel} comparison={b.comparison} />
      </dl>
      <AnswerSummary summary={b.summary} byModel={b.summaryByModel} />
      {first && first.groups.length > 0 && (
        <ContributionBars
          dimension={first}
          baseLabel={b.baseLabel}
          compareLabel={b.compareLabel}
          comparison={b.comparison}
        />
      )}
    </>
  )
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col">
      <dt className="text-xs font-medium text-muted-foreground">{label}</dt>
      <dd className="text-lg font-semibold tracking-tight break-all">{value}</dd>
    </div>
  )
}

type ChangeStatProps = { change: string; baseLabel: string; comparison: Comparison }

function ChangeStat({ change, baseLabel, comparison }: ChangeStatProps) {
  const direction = changeDirection(change)
  const Icon = direction === 'down' ? TrendingDown : TrendingUp
  const word = directionWord(direction, comparison)
  return (
    <div className="flex flex-col">
      <dt className="text-xs font-medium text-muted-foreground">Chênh lệch</dt>
      <dd className="flex flex-wrap items-baseline gap-x-2">
        <span className="text-lg font-semibold tracking-tight break-all text-highlight-text">{formatSigned(change)}</span>
        <span
          className={cn(
            'inline-flex items-center gap-1 text-xs',
            direction === 'down' ? 'text-highlight-text' : 'text-label',
          )}
        >
          {direction !== 'flat' && <Icon aria-hidden className="size-3.5" />}
          {comparison === 'groups' ? `${word} ${baseLabel}` : `${word} so với ${baseLabel}`}
        </span>
      </dd>
    </div>
  )
}
