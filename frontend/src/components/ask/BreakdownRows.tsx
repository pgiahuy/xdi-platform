// SPDX-License-Identifier: Apache-2.0
/* components/ask/BreakdownRows.tsx - các ô và dòng phụ của bảng phân rã một chiều (BreakdownDimension.tsx): ô đóng góp
có màu theo chiều, hai ô phần trăm (% tác động, % thay đổi; cách tính ở breakdownPercent.ts), dòng "Còn lại" gộp các
nhóm không hiện, và dòng "Tổng" của cả lát cắt. Tách khỏi BreakdownDimension.tsx để mỗi file đọc được trong một màn
hình. Dòng "Còn lại" giữ màu chữ nhạt, không tô màu chiều: nó gộp nhiều nhóm nên không phải chỗ để đọc chiều. */

import { cn } from '@/lib/utils'
import { sumDecimals, type RestGroups } from './breakdownGroups'
import {
  changePercent,
  formatChangePercent,
  formatShare,
  isOffTrend,
  percentDirection,
  sharePercent,
} from './breakdownPercent'
import { formatDecimal, formatOptionalDecimal } from './cellFormat'
import { changeDirection, directionTextClass } from './changeFormat'
import type { BreakdownView } from './evidenceTypes'

/** Số của hai lát cắt mà bảng cần: chênh lệch để tính % tác động, hai tổng cho dòng "Tổng" và mức chung. */
export type SliceTotals = Pick<
  BreakdownView,
  'measure' | 'baseLabel' | 'compareLabel' | 'baseValue' | 'compareValue' | 'change'
>

const numberCell = 'px-2 py-1 text-right tabular-nums'

/** Ô đóng góp: số có màu theo chiều (giảm đỏ, tăng xanh lá); muted giữ màu nhạt của dòng "Còn lại". */
export function ContributionCell({ value, muted = false }: { value: string | null; muted?: boolean }) {
  const color = value === null || muted ? '' : directionTextClass(changeDirection(value))
  return <td className={cn(numberCell, color)}>{formatOptionalDecimal(value)}</td>
}

type PercentCellsProps = {
  contribution: string | null
  base: string | null
  compare: string | null
  totals: SliceTotals
  /** % thay đổi của cả lát cắt, để đánh dấu nhóm khác thường. */
  overall: number | null
  /** Độ đo tỷ lệ: không có ô % thay đổi (breakdownPercent.ts). */
  ratio: boolean
  muted?: boolean
}

/** Ô % tác động, rồi ô % thay đổi khi độ đo cộng được. Nhóm khác thường (isOffTrend) in đậm, kèm chữ cho trình đọc màn
 * hình. */
export function PercentCells(props: PercentCellsProps) {
  const { contribution, base, compare, totals, overall, ratio, muted = false } = props
  const share = sharePercent(contribution, totals.change)
  const change = ratio ? null : changePercent(base, compare)
  const offTrend = !muted && isOffTrend(change, overall, share)
  return (
    <>
      <td className={numberCell}>{formatShare(share)}</td>
      {!ratio && (
        <td
          className={cn(
            numberCell,
            !muted && directionTextClass(percentDirection(change)),
            offTrend && 'font-semibold',
          )}
        >
          {formatChangePercent(change)}
          {offTrend && <span className="sr-only"> (khác xa mức chung)</span>}
        </td>
      )}
    </>
  )
}

type RestRowProps = { rest: RestGroups; totals: SliceTotals; overall: number | null; ratio: boolean }

export function RestRow({ rest, totals, overall, ratio }: RestRowProps) {
  return (
    <tr className="border-t text-muted-foreground">
      <th scope="row" className="px-2 py-1 text-left font-normal">
        Còn lại ({rest.count} nhóm)
      </th>
      <td className={numberCell}>{formatOptionalDecimal(rest.baseValue)}</td>
      <td className={numberCell}>{formatOptionalDecimal(rest.compareValue)}</td>
      <ContributionCell value={rest.contribution} muted />
      <PercentCells
        contribution={rest.contribution}
        base={rest.baseValue}
        compare={rest.compareValue}
        totals={totals}
        overall={overall}
        ratio={ratio}
        muted
      />
      {ratio && (
        <>
          <td className={numberCell}>{formatOptionalDecimal(rest.mixEffect)}</td>
          <td className={numberCell}>{formatOptionalDecimal(rest.rateEffect)}</td>
        </>
      )}
    </tr>
  )
}

type TotalRowProps = {
  totals: SliceTotals
  overall: number | null
  /** Chỉ có với độ đo tỷ lệ: tổng phần do cơ cấu và phần do hiệu suất của mọi nhóm. */
  mix: string | null
  rate: string | null
}

/** Dòng "Tổng": cả lát cắt, nên đóng góp là chênh lệch và % tác động là 100%. Với độ đo tỷ lệ, hai ô số của hai lát cắt
 * để trống (tỷ lệ các nhóm không cộng được) và đóng góp là tổng hai phần, bằng chênh lệch. */
export function TotalRow({ totals, overall, mix, rate }: TotalRowProps) {
  const ratio = mix !== null && rate !== null
  const contribution = ratio ? sumDecimals([mix, rate]) : totals.change
  return (
    <tr className="border-t font-semibold">
      <th scope="row" className="px-2 py-1 text-left">
        Tổng
      </th>
      <td className={numberCell}>{ratio ? '' : formatDecimal(totals.baseValue)}</td>
      <td className={numberCell}>{ratio ? '' : formatDecimal(totals.compareValue)}</td>
      <ContributionCell value={contribution} />
      <td className={numberCell}>{formatShare(sharePercent(totals.change, totals.change))}</td>
      {ratio ? (
        <>
          <td className={numberCell}>{formatOptionalDecimal(mix)}</td>
          <td className={numberCell}>{formatOptionalDecimal(rate)}</td>
        </>
      ) : (
        <td className={cn(numberCell, directionTextClass(percentDirection(overall)))}>
          {formatChangePercent(overall)}
        </td>
      )}
    </tr>
  )
}
