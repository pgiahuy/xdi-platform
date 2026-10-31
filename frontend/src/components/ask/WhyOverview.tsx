// SPDX-License-Identifier: Apache-2.0
/* components/ask/WhyOverview.tsx - phần đầu câu trả lời của câu "tại sao": insight (WhyInsight.tsx),
một hàng ba số chia đều ba cột (lát gốc, lát so sánh, chênh lệch) không khung riêng, để thẻ trả lời không thành nhiều
khung lồng nhau; số dòng của mỗi lát không hiện ở đây vì không giúp đọc câu trả lời (tab Phân rã vẫn có), rồi chênh lệch của từng nhóm theo chiều đầu tiên (ContributionBars.tsx). Mọi số lấy từ
explanation của response, cùng dữ liệu với tab Phân rã của pane bằng chứng; ở đây chỉ hiện gọn để đọc câu trả lời mà
không phải mở pane. Số là chuỗi chữ số của backend, chỉ nhóm nghìn (formatDecimal), không làm tròn.
Chênh lệch luôn kèm dấu và chữ chiều ("tăng", "giảm" giữa hai kỳ; "cao hơn", "thấp hơn" giữa hai nhóm): không để màu
một mình nói chiều thay đổi. */

import { TrendingDown, TrendingUp } from 'lucide-react'
import { cn } from '@/lib/utils'
import { WhyInsight } from './WhyInsight'
import { columnText } from './columnText'
import { formatDecimal } from './cellFormat'
import { ContributionBars } from './ContributionBars'
import {
  changeDirection,
  directionSoftClass,
  directionTextClass,
  directionWord,
  formatPercent,
  formatSigned,
} from './changeFormat'
import type { BreakdownView, Comparison } from './evidenceTypes'

type Props = {
  breakdown: BreakdownView
}

export function WhyOverview({ breakdown: b }: Props) {
  const first = b.dimensions[0]
  return (
    <>
      <WhyInsight title={b.summaryTitle} points={b.summaryPoints} byModel={b.byModel} warnings={b.warnings} />
      <div className="flex flex-col gap-1">
        {b.sliceColumn && (
          <p className="text-xs text-muted-foreground">So theo {columnText(b.sliceColumn, b.sliceColumnLabel)}</p>
        )}
        <dl className="grid grid-cols-1 gap-x-6 gap-y-3 tabular-nums sm:grid-cols-3">
          <Stat label={b.baseLabel} value={b.baseValue} />
          <Stat label={b.compareLabel} value={b.compareValue} />
          <ChangeStat change={b.change} percent={b.percentChange} baseLabel={b.baseLabel} comparison={b.comparison} />
        </dl>
      </div>
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
      <dd className="text-lg font-semibold tracking-tight break-all">{formatDecimal(value)}</dd>
    </div>
  )
}

type ChangeStatProps = { change: string; percent: string | null; baseLabel: string; comparison: Comparison }

function ChangeStat({ change, percent, baseLabel, comparison }: ChangeStatProps) {
  const direction = changeDirection(change)
  const Icon = direction === 'down' ? TrendingDown : TrendingUp
  const word = directionWord(direction, comparison)
  // Chữ chiều đã nói tăng hay giảm, nên phần trăm bỏ dấu: "giảm 13,6% so với Quý 2/2017", "cao hơn LATAM 25,3%".
  const amount = percent !== null && direction !== 'flat' ? formatPercent(percent.replace(/^-/, '')) : null
  const text =
    comparison === 'groups'
      ? [word, baseLabel, amount].filter(Boolean).join(' ')
      : [word, amount, 'so với', baseLabel].filter(Boolean).join(' ')
  const tone = directionTextClass(direction)
  return (
    <div className="flex flex-col gap-1">
      <dt className="text-xs font-medium text-muted-foreground">Chênh lệch</dt>
      <dd className={cn('text-lg font-semibold tracking-tight break-all', tone)}>{formatSigned(change)}</dd>
      {/* Nhãn "tăng 5,7% so với ..." là điều người đọc cần thấy trước nhất: chữ đậm, cỡ lớn hơn, nền nhạt cùng màu chiều. */}
      <dd>
        <span
          className={cn(
            'inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-sm font-semibold',
            tone,
            directionSoftClass(direction),
          )}
        >
          {direction !== 'flat' && <Icon aria-hidden className="size-4" />}
          {text}
        </span>
      </dd>
    </div>
  )
}
