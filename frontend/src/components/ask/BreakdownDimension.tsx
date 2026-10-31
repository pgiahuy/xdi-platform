// SPDX-License-Identifier: Apache-2.0
/* components/ask/BreakdownDimension.tsx - bảng chênh lệch của một chiều trong tab Phân rã (EvidenceBreakdown.tsx). Chiều
nhiều nhóm hiện SHOWN_GROUPS nhóm đầu và một dòng "Còn lại" có tổng (breakdownGroups.ts), nút "Xem tất cả" mở đủ mọi
nhóm. Bấm số của một lát cắt ở một nhóm thì trang mở tab Dòng nguồn với đúng lát cắt, cột và giá trị nhóm (onDrill);
dòng "Còn lại" gộp nhiều nhóm nên số của nó không bấm được.
Cột theo thiết kế của #186: Nhóm, hai lát cắt, Chênh lệch (lát so sánh trừ lát gốc), % tác động, % thay đổi; nghĩa của
mỗi cột là dòng chữ nhỏ ngay dưới tiêu đề cột. % tác động, % thay đổi và cờ khác thường của từng nhóm do backend tính
(cùng luật với lời tóm tắt), nên nhãn "Có nhóm khác thường" hay "Dàn đều" của bảng và nhóm in đậm luôn khớp lời. Chênh
lệch tổng gần 0 thì backend không tính % tác động (số phình ra vô nghĩa), bảng ẩn cả cột đó và không gắn nhãn. Ô và dòng
phụ ở BreakdownRows.tsx.
Độ đo tỷ lệ (dimension.mixEffect khác null) có hai cột "Do cơ cấu" và "Do hiệu suất" thay cho cột % thay đổi; ô tỷ lệ
của dòng "Còn lại" và dòng "Tổng" để trống vì tỷ lệ các nhóm không cộng được. */

import { useState, type ReactNode } from 'react'

import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { splitGroups } from './breakdownGroups'
import { ContributionCell, GroupPercentCells, RestRow, TotalRow, type SliceTotals } from './BreakdownRows'
import { formatCell, formatDecimal, formatOptionalDecimal } from './cellFormat'
import { columnText } from './columnText'
import { stickyCell } from './stickyCell'
import type { DimensionView, DrillTarget, GroupView, Slice } from './evidenceTypes'

type Props = {
  dimension: DimensionView
  totals: SliceTotals
  onDrill: (target: DrillTarget) => void
}

export function BreakdownDimension({ dimension, totals, onDrill }: Props) {
  const [expanded, setExpanded] = useState(false)
  const { shown, rest } = splitGroups(dimension.groups)
  const groups = expanded ? dimension.groups : shown
  const ratio = dimension.mixEffect !== null && dimension.rateEffect !== null
  const showShare = dimension.groups.some((group) => group.shareOfChange !== null)
  const { baseLabel, compareLabel } = totals
  const title = `${totals.measure} theo ${columnText(dimension.column, dimension.label)}`

  const drillButton = (group: GroupView, slice: Slice) => {
    const label = slice === 'base' ? baseLabel : compareLabel
    const number = slice === 'base' ? group.baseValue : group.compareValue
    const name = `${dimension.column} = ${groupName(group, dimension)}`
    return (
      <Button
        variant="link"
        size="xs"
        className="h-auto px-0 tabular-nums"
        aria-label={`Xem dòng nguồn: ${name}, ${label}`}
        onClick={() =>
          onDrill({ from: 'breakdown', slice, column: dimension.column, value: group.value, kind: dimension.kind })
        }
      >
        {formatDecimal(number)}
      </Button>
    )
  }

  // Các cột số không vừa bề ngang hẹp thì cuộn ngang trong khung, trang không cuộn; khung nhận focus để cuộn bằng phím.
  // Tên bảng và nhãn nằm ngoài khung cuộn để không trôi đi khi cuộn; caption của bảng giữ tên cho trình đọc màn hình.
  return (
    <div className="flex flex-col items-start gap-1">
      <p aria-hidden className="text-sm font-semibold">
        {title}
        {showShare && <EvenTag hasOutliers={dimension.hasOutliers} />}
      </p>
      <div
        tabIndex={0}
        role="region"
        aria-label={`Phân rã theo ${dimension.column}`}
        className="w-full overflow-x-auto"
      >
        <table className="w-full text-xs">
          <caption className="sr-only">
            {title}
            {showShare && (dimension.hasOutliers ? ', có nhóm khác thường' : ', dàn đều')}
          </caption>
          <thead className="bg-background text-muted-foreground">
            <tr>
              <HeaderCell left>Nhóm</HeaderCell>
              <HeaderCell>{baseLabel} (gốc)</HeaderCell>
              <HeaderCell>{compareLabel}</HeaderCell>
              <HeaderCell note="so sánh trừ gốc">Chênh lệch</HeaderCell>
              {showShare && <HeaderCell note="phần của tổng">% tác động</HeaderCell>}
              {ratio ? (
                <>
                  <HeaderCell note="tỷ trọng đổi">Do cơ cấu</HeaderCell>
                  <HeaderCell note="tỷ lệ đổi">Do hiệu suất</HeaderCell>
                </>
              ) : (
                <HeaderCell note="so với gốc">% thay đổi</HeaderCell>
              )}
            </tr>
          </thead>
          <tbody>
            {groups.map((group, index) => (
              <tr key={index} className="border-t">
                <th
                  scope="row"
                  className={cn(
                    'px-2 py-1 text-left',
                    stickyCell('bg-card'),
                    group.outlier ? 'font-semibold' : 'font-normal',
                  )}
                >
                  <span className={group.value === null ? 'text-muted-foreground' : undefined}>
                    {groupName(group, dimension)}
                  </span>
                </th>
                <td className="px-2 py-1 text-right">{drillButton(group, 'base')}</td>
                <td className="px-2 py-1 text-right">{drillButton(group, 'compare')}</td>
                <ContributionCell value={group.contribution} />
                <GroupPercentCells group={group} totals={totals} ratio={ratio} showShare={showShare} />
                {ratio && (
                  <>
                    <td className="px-2 py-1 text-right tabular-nums">{formatOptionalDecimal(group.mixEffect)}</td>
                    <td className="px-2 py-1 text-right tabular-nums">{formatOptionalDecimal(group.rateEffect)}</td>
                  </>
                )}
              </tr>
            ))}
            {rest && !expanded && <RestRow rest={rest} totals={totals} ratio={ratio} showShare={showShare} />}
            <TotalRow totals={totals} mix={dimension.mixEffect} rate={dimension.rateEffect} showShare={showShare} />
          </tbody>
        </table>
      </div>
      {ratio && (
        <p className="text-xs text-muted-foreground">
          Số của hai bên là tỷ lệ của từng nhóm. Phần do cơ cấu cộng phần do hiệu suất bằng chênh lệch của nhóm.
        </p>
      )}
      {rest && (
        <Button
          variant="link"
          size="xs"
          className="h-auto px-0"
          aria-expanded={expanded}
          onClick={() => setExpanded(!expanded)}
        >
          {expanded ? 'Thu gọn' : `Xem tất cả ${dimension.groups.length} nhóm`}
        </Button>
      )}
    </div>
  )
}

/** Nhãn của bảng: có nhóm khác thường (nền hổ phách nhạt, đọc bảng này trước) hay dàn đều (mọi nhóm đổi gần như mức
 * chung, nhóm chênh nhiều chỉ vì là nhóm lớn). */
function EvenTag({ hasOutliers }: { hasOutliers: boolean }) {
  return (
    <span
      className={cn(
        'ml-2 inline-block rounded-full px-2 py-0.5 align-middle text-[11px] font-medium',
        hasOutliers ? 'bg-highlight-soft text-highlight-text' : 'bg-muted text-muted-foreground',
      )}
    >
      {hasOutliers ? 'Có nhóm khác thường' : 'Dàn đều'}
    </span>
  )
}

function HeaderCell({ children, left = false, note }: { children: ReactNode; left?: boolean; note?: string }) {
  return (
    <th
      scope="col"
      className={cn(
        'px-2 py-1.5 align-bottom font-medium',
        left ? cn('text-left', stickyCell('bg-background')) : 'text-right',
      )}
    >
      {children}
      {note && <span className="block text-[11px] font-normal whitespace-nowrap">{note}</span>}
    </th>
  )
}

function groupName(group: GroupView, dimension: DimensionView): string {
  return group.value === null ? 'trống' : formatCell(group.value, dimension.kind)
}
