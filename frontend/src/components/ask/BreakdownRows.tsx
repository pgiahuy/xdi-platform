// SPDX-License-Identifier: Apache-2.0
/* components/ask/BreakdownRows.tsx - các ô và dòng phụ của bảng phân rã một chiều (BreakdownDimension.tsx): ô chênh lệch
có màu theo chiều, hai ô phần trăm của một nhóm (% tác động, % thay đổi; số và cờ khác thường do backend tính), dòng
"Còn lại" gộp các nhóm không hiện, và dòng "Tổng" của cả lát cắt. Tách khỏi BreakdownDimension.tsx để mỗi file đọc được
trong một màn hình. Dòng "Còn lại" giữ màu chữ nhạt, không tô màu chiều: nó gộp nhiều nhóm nên không phải chỗ để đọc
chiều. Ô đầu mỗi dòng (tên nhóm) đứng yên khi bảng cuộn ngang (stickyCell), để pane hẹp vẫn biết dòng số là của nhóm
nào. */

import { cn } from '@/lib/utils'
import { sumDecimals, type RestGroups } from './breakdownGroups'
import {
  changePercent,
  formatChangePercent,
  formatShare,
  parsePercent,
  percentDirection,
  sharePercent,
} from './breakdownPercent'
import { formatDecimal, formatOptionalDecimal } from './cellFormat'
import { changeDirection, directionTextClass } from './changeFormat'
import { stickyCell } from './stickyCell'
import type { BreakdownView, GroupView } from './evidenceTypes'

/** Số của hai lát cắt mà bảng cần: chênh lệch để tính % tác động, hai tổng cho dòng "Tổng" và mức chung. */
export type SliceTotals = Pick<
  BreakdownView,
  'measure' | 'comparison' | 'baseLabel' | 'compareLabel' | 'baseValue' | 'compareValue' | 'change'
>

const numberCell = 'px-2 py-1 text-right tabular-nums'

/** Ô chênh lệch: số có màu theo chiều (giảm hổ phách, tăng xanh ngọc); muted giữ màu nhạt của dòng "Còn lại". */
export function ContributionCell({ value, muted = false }: { value: string | null; muted?: boolean }) {
  const color = value === null || muted ? '' : directionTextClass(changeDirection(value))
  return <td className={cn(numberCell, color)}>{formatOptionalDecimal(value)}</td>
}

type GroupPercentCellsProps = {
  group: GroupView
  totals: SliceTotals
  /** Độ đo tỷ lệ: không có ô % thay đổi. */
  ratio: boolean
  /** false khi chênh lệch tổng gần 0: bảng ẩn cả cột % tác động. */
  showShare: boolean
}

/** Ô % tác động, rồi ô % thay đổi khi độ đo cộng được, đọc đúng số backend gửi. Nhóm đi ngược chiều có % tác động âm
 * và chữ "kéo ngược"; nhóm chỉ có ở một bên hay đổi dấu không có % thay đổi mà có chữ thay vào. Nhóm khác thường in đậm,
 * kèm chữ cho trình đọc màn hình. */
export function GroupPercentCells({ group, totals, ratio, showShare }: GroupPercentCellsProps) {
  const share = parsePercent(group.shareOfChange)
  const change = parsePercent(group.percentChange)
  const note = changeNote(group, totals)
  return (
    <>
      {showShare && (
        <td className={numberCell}>
          {formatShare(share)}
          {share !== null && share < 0 && <span className="block text-[11px] text-muted-foreground">kéo ngược</span>}
        </td>
      )}
      {!ratio && (
        <td
          className={cn(
            numberCell,
            note === null && directionTextClass(percentDirection(change)),
            group.outlier && 'font-semibold',
          )}
        >
          {note ?? formatChangePercent(change)}
          {group.outlier && <span className="sr-only"> (khác thường)</span>}
        </td>
      )}
    </>
  )
}

// Chữ thay cho % thay đổi. Hai kỳ: "mới", "không còn". Hai nhóm không có trước sau nên nói nhóm có ở bên nào.
function changeNote(group: GroupView, totals: SliceTotals): string | null {
  if (group.signFlip) return 'đổi dấu'
  if (group.presence === 'both') return null
  if (totals.comparison === 'periods') return group.presence === 'new' ? 'mới' : 'không còn'
  return `chỉ ở ${group.presence === 'new' ? totals.compareLabel : totals.baseLabel}`
}

type RestRowProps = {
  rest: RestGroups
  totals: SliceTotals
  ratio: boolean
  showShare: boolean
}

/** Dòng gộp các nhóm không hiện: backend không có số cho nhóm gộp nên phần trăm tính ở đây (breakdownPercent.ts). */
export function RestRow({ rest, totals, ratio, showShare }: RestRowProps) {
  const change = ratio ? null : changePercent(rest.baseValue, rest.compareValue)
  return (
    <tr className="border-t text-muted-foreground">
      <th scope="row" className={cn('px-2 py-1 text-left font-normal', stickyCell('bg-card'))}>
        Còn lại ({rest.count} nhóm)
      </th>
      <td className={numberCell}>{formatOptionalDecimal(rest.baseValue)}</td>
      <td className={numberCell}>{formatOptionalDecimal(rest.compareValue)}</td>
      <ContributionCell value={rest.contribution} muted />
      {showShare && <td className={numberCell}>{formatShare(sharePercent(rest.contribution, totals.change))}</td>}
      {!ratio && <td className={numberCell}>{formatChangePercent(change)}</td>}
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
  /** Chỉ có với độ đo tỷ lệ: tổng phần do cơ cấu và phần do hiệu suất của mọi nhóm. */
  mix: string | null
  rate: string | null
  showShare: boolean
}

/** Dòng "Tổng": cả lát cắt, nên chênh lệch là chênh lệch chung, % tác động là 100% và % thay đổi là mức chung để so
 * với từng nhóm. Với độ đo tỷ lệ, hai ô số của hai lát cắt để trống (tỷ lệ các nhóm không cộng được) và chênh lệch là
 * tổng hai phần, bằng chênh lệch chung. */
export function TotalRow({ totals, mix, rate, showShare }: TotalRowProps) {
  const ratio = mix !== null && rate !== null
  const contribution = ratio ? sumDecimals([mix, rate]) : totals.change
  const overall = ratio ? null : changePercent(totals.baseValue, totals.compareValue)
  return (
    <tr className="border-t font-semibold">
      <th scope="row" className={cn('px-2 py-1 text-left', stickyCell('bg-card'))}>
        Tổng
      </th>
      <td className={numberCell}>{ratio ? '' : formatDecimal(totals.baseValue)}</td>
      <td className={numberCell}>{ratio ? '' : formatDecimal(totals.compareValue)}</td>
      <ContributionCell value={contribution} />
      {showShare && <td className={numberCell}>{formatShare(sharePercent(totals.change, totals.change))}</td>}
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
