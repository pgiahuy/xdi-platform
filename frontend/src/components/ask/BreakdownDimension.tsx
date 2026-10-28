// SPDX-License-Identifier: Apache-2.0
/* components/ask/BreakdownDimension.tsx - bảng đóng góp của một chiều trong tab Phân rã (EvidenceBreakdown.tsx). Chiều
nhiều nhóm hiện SHOWN_GROUPS nhóm đầu và một dòng "Còn lại" có tổng (breakdownGroups.ts), nút "Xem tất cả" mở đủ mọi
nhóm. Bấm số của một lát cắt ở một nhóm thì trang mở tab Dòng nguồn với đúng lát cắt, cột và giá trị nhóm (onDrill);
dòng "Còn lại" gộp nhiều nhóm nên số của nó không bấm được.
Ngoài số tuyệt đối, mỗi nhóm có % Tác động và % Thay đổi (breakdownPercent.ts), đóng góp và % thay đổi tô màu theo chiều
(giảm đỏ, tăng xanh lá), dòng "Tổng" cuối bảng cho thấy cột đóng góp cộng lại đúng bằng chênh lệch. Ô và dòng phụ ở
BreakdownRows.tsx.
Độ đo tỷ lệ (dimension.mixEffect khác null) có hai cột "Do cơ cấu" và "Do hiệu suất" thay cho cột % Thay đổi, và một câu
giải thích hai phần; ô tỷ lệ của dòng "Còn lại" và dòng "Tổng" để trống vì tỷ lệ các nhóm không cộng được. */

import { useState, type ReactNode } from 'react'

import { Button } from '@/components/ui/button'
import { splitGroups } from './breakdownGroups'
import { changePercent } from './breakdownPercent'
import { ContributionCell, PercentCells, RestRow, TotalRow, type SliceTotals } from './BreakdownRows'
import { formatCell, formatDecimal, formatOptionalDecimal } from './cellFormat'
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
  const overall = ratio ? null : changePercent(totals.baseValue, totals.compareValue)
  const { baseLabel, compareLabel } = totals

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
  return (
    <div className="flex flex-col items-start gap-1">
      <div
        tabIndex={0}
        role="region"
        aria-label={`Phân rã theo ${dimension.column}`}
        className="w-full overflow-x-auto"
      >
        <table className="w-full text-xs">
          <caption className="pb-1 text-left text-sm font-semibold">
            {totals.measure} theo {dimension.column}
          </caption>
          <thead className="bg-background text-muted-foreground">
            <tr>
              <HeaderCell left>Nhóm</HeaderCell>
              <HeaderCell>{baseLabel} (gốc)</HeaderCell>
              <HeaderCell>{compareLabel}</HeaderCell>
              <HeaderCell>Đóng góp</HeaderCell>
              <HeaderCell>% Tác động</HeaderCell>
              {ratio ? (
                <>
                  <HeaderCell>Do cơ cấu</HeaderCell>
                  <HeaderCell>Do hiệu suất</HeaderCell>
                </>
              ) : (
                <HeaderCell>% Thay đổi</HeaderCell>
              )}
            </tr>
          </thead>
          <tbody>
            {groups.map((group, index) => (
              <tr key={index} className="border-t">
                <th scope="row" className="px-2 py-1 text-left font-normal">
                  <span className={group.value === null ? 'text-muted-foreground' : undefined}>
                    {groupName(group, dimension)}
                  </span>
                </th>
                <td className="px-2 py-1 text-right">{drillButton(group, 'base')}</td>
                <td className="px-2 py-1 text-right">{drillButton(group, 'compare')}</td>
                <ContributionCell value={group.contribution} />
                <PercentCells
                  contribution={group.contribution}
                  base={group.baseValue}
                  compare={group.compareValue}
                  totals={totals}
                  overall={overall}
                  ratio={ratio}
                />
                {ratio && (
                  <>
                    <td className="px-2 py-1 text-right tabular-nums">{formatOptionalDecimal(group.mixEffect)}</td>
                    <td className="px-2 py-1 text-right tabular-nums">{formatOptionalDecimal(group.rateEffect)}</td>
                  </>
                )}
              </tr>
            ))}
            {rest && !expanded && <RestRow rest={rest} totals={totals} overall={overall} ratio={ratio} />}
            <TotalRow totals={totals} overall={overall} mix={dimension.mixEffect} rate={dimension.rateEffect} />
          </tbody>
        </table>
      </div>
      {ratio && (
        <p className="text-xs text-muted-foreground">
          Số của hai bên là tỷ lệ của từng nhóm. Do cơ cấu: phần chênh lệch vì tỷ trọng của nhóm đổi. Do hiệu suất: phần
          chênh lệch vì tỷ lệ trong nhóm đổi. Hai phần cộng lại bằng đóng góp.
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

function HeaderCell({ children, left = false }: { children: ReactNode; left?: boolean }) {
  return (
    <th scope="col" className={`px-2 py-1.5 font-medium ${left ? 'text-left' : 'text-right'}`}>
      {children}
    </th>
  )
}

function groupName(group: GroupView, dimension: DimensionView): string {
  return group.value === null ? 'trống' : formatCell(group.value, dimension.kind)
}
