// SPDX-License-Identifier: Apache-2.0
/* components/ask/ContributionBars.tsx - biểu đồ thanh hai phía cho chênh lệch của từng nhóm (lát so sánh trừ lát gốc
trong nhóm, cộng lại bằng chênh lệch chung), theo chiều đầu tiên backend gửi. Tiêu đề ghi tên cột kèm nghĩa tiếng Việt
(columnText.ts). Thanh giảm sang trái màu hổ phách, thanh tăng sang phải màu xanh ngọc (cùng cặp màu với bảng phân rã,
changeFormat.directionTextClass), cùng một thang nên so được độ lớn bằng mắt. Nhóm như bảng phân rã: các nhóm lớn nhất
rồi một dòng "Còn lại" (breakdownGroups.ts).
Mỗi dòng ghi số có dấu (+ hay -), nên màu và hướng thanh chỉ giúp đọc nhanh; chữ chiều chỉ cho trình đọc màn hình
(nhãn chữ ở mọi dòng làm bảng rối mà không thêm ý), thanh ẩn với trình đọc màn hình. Độ dài thanh đổi số bằng
Number(): chỉ để vẽ, chấp nhận làm tròn; chữ số hiện ra vẫn là chuỗi của backend.
Độ đo tỷ lệ thêm một dòng tổng phần do cơ cấu và phần do hiệu suất của chiều này.
Vẽ bằng thẻ div thay vì Recharts: chỉ là các thanh ngang, và file này nằm trong chunk chính, không phải chờ tải Recharts. */

import { cn } from '@/lib/utils'
import { splitGroups } from './breakdownGroups'
import { columnText } from './columnText'
import { formatCell } from './cellFormat'
import {
  changeDirection,
  directionFillClass,
  directionTextClass,
  directionWord,
  formatSigned,
} from './changeFormat'
import type { Comparison, DimensionView } from './evidenceTypes'

type Props = {
  dimension: DimensionView
  baseLabel: string
  compareLabel: string
  comparison: Comparison
}

type Bar = { name: string; contribution: string; muted: boolean }

export function ContributionBars({ dimension, baseLabel, compareLabel, comparison }: Props) {
  const { shown, rest } = splitGroups(dimension.groups)
  const bars: Bar[] = shown.map((group) => ({
    name: group.value === null ? 'trống' : formatCell(group.value, dimension.kind),
    contribution: group.contribution,
    muted: group.value === null,
  }))
  if (rest?.contribution) {
    bars.push({ name: `Còn lại (${rest.count} nhóm)`, contribution: rest.contribution, muted: true })
  }
  const largest = Math.max(...bars.map((bar) => Math.abs(Number(bar.contribution)) || 0))

  return (
    <section aria-labelledby="contribution-heading" className="flex flex-col gap-3">
      <div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
        <h3 id="contribution-heading" className="text-sm font-semibold">
          Chênh lệch theo {columnText(dimension.column, dimension.label)}
        </h3>
        <p className="text-xs text-muted-foreground">
          {compareLabel} trừ {baseLabel}
        </p>
      </div>
      {dimension.mixEffect !== null && dimension.rateEffect !== null && (
        <p className="text-sm text-muted-foreground">
          Do cơ cấu (tỷ trọng các nhóm đổi){' '}
          <span className="font-semibold tabular-nums">{formatSigned(dimension.mixEffect)}</span>, do hiệu suất (tỷ lệ
          trong từng nhóm đổi) <span className="font-semibold tabular-nums">{formatSigned(dimension.rateEffect)}</span>.
        </p>
      )}
      <ul className="flex flex-col tabular-nums">
        {bars.map((bar, index) => (
          <ContributionRow key={index} bar={bar} largest={largest} comparison={comparison} />
        ))}
      </ul>
    </section>
  )
}

function ContributionRow({ bar, largest, comparison }: { bar: Bar; largest: number; comparison: Comparison }) {
  const direction = changeDirection(bar.contribution)
  // Nửa thanh là 50% bề ngang: nhóm lớn nhất chạm mép, các nhóm khác theo tỷ lệ.
  const half = largest > 0 ? (Math.abs(Number(bar.contribution)) / largest) * 50 : 0
  return (
    <li
      className={cn(
        'grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-3 gap-y-1 border-t py-1.5 first:border-t-0',
        'sm:grid-cols-[minmax(0,9rem)_minmax(0,1fr)_auto]',
      )}
    >
      <span
        className={cn('col-start-1 row-start-1 truncate text-sm font-medium', bar.muted && 'text-muted-foreground')}
        title={bar.name}
      >
        {bar.name}
      </span>
      <span
        aria-hidden="true"
        className="relative col-span-2 col-start-1 row-start-2 h-5 sm:col-span-1 sm:col-start-2 sm:row-start-1"
      >
        <span className="absolute inset-y-0 left-1/2 w-px bg-muted-foreground" />
        {direction !== 'flat' && (
          <span
            className={cn(
              'absolute inset-y-0.5',
              directionFillClass(direction),
              bar.muted && 'opacity-60',
            )}
            style={direction === 'down' ? { right: '50%', width: `${half}%` } : { left: '50%', width: `${half}%` }}
          />
        )}
      </span>
      <span className="col-start-2 row-start-1 flex items-center justify-end gap-2 text-sm sm:col-start-3">
        <span className={cn('font-semibold', directionTextClass(direction))}>
          {formatSigned(bar.contribution)}
        </span>
        <span className="sr-only">{directionWord(direction, comparison)}</span>
      </span>
    </li>
  )
}
