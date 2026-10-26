// SPDX-License-Identifier: Apache-2.0
/* components/ask/BreakdownDimension.tsx - bảng đóng góp của một chiều trong tab Phân rã (EvidenceBreakdown.tsx). Chiều
nhiều nhóm hiện SHOWN_GROUPS nhóm đầu và một dòng "Còn lại" có tổng (breakdownGroups.ts), nút "Xem tất cả" mở đủ mọi
nhóm. Bấm số của một lát cắt ở một nhóm thì trang mở tab Dòng nguồn với đúng lát cắt, cột và giá trị nhóm (onDrill);
dòng "Còn lại" gộp nhiều nhóm nên số của nó không bấm được. */

import { useState } from 'react'

import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { splitGroups, type RestGroups } from './breakdownGroups'
import { formatCell, formatDecimal } from './cellFormat'
import type { DimensionView, DrillTarget, GroupView, Slice } from './evidenceTypes'

type Props = {
  dimension: DimensionView
  baseLabel: string
  compareLabel: string
  onDrill: (target: DrillTarget) => void
}

export function BreakdownDimension({ dimension, baseLabel, compareLabel, onDrill }: Props) {
  const [expanded, setExpanded] = useState(false)
  const { shown, rest } = splitGroups(dimension.groups)
  const groups = expanded ? dimension.groups : shown

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

  // Bốn cột số không vừa bề ngang hẹp thì cuộn ngang trong khung, trang không cuộn; khung nhận focus để cuộn bằng phím.
  return (
    <div className="flex flex-col items-start gap-1">
      <div tabIndex={0} role="region" aria-label={`Phân rã theo ${dimension.column}`} className="w-full overflow-x-auto">
        <table className="w-full text-xs">
          <caption className="pb-1 text-left text-sm font-semibold">Theo {dimension.column}</caption>
          <thead className="bg-muted/50">
            <tr>
              <th scope="col" className="px-2 py-1.5 text-left font-medium">
                Nhóm
              </th>
              <th scope="col" className="px-2 py-1.5 text-right font-medium">
                {baseLabel}
              </th>
              <th scope="col" className="px-2 py-1.5 text-right font-medium">
                {compareLabel}
              </th>
              <th scope="col" className="px-2 py-1.5 text-right font-medium">
                Đóng góp
              </th>
            </tr>
          </thead>
          <tbody>
            {groups.map((group, index) => (
              <tr key={index} className="border-t">
                <th scope="row" className="px-2 py-1 text-left font-normal">
                  <span className={cn(group.value === null && 'text-muted-foreground')}>
                    {groupName(group, dimension)}
                  </span>
                </th>
                <td className="px-2 py-1 text-right">{drillButton(group, 'base')}</td>
                <td className="px-2 py-1 text-right">{drillButton(group, 'compare')}</td>
                <td className="px-2 py-1 text-right tabular-nums">{formatDecimal(group.contribution)}</td>
              </tr>
            ))}
            {rest && !expanded && <RestRow rest={rest} />}
          </tbody>
        </table>
      </div>
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

function RestRow({ rest }: { rest: RestGroups }) {
  const number = (value: string | null) => (value === null ? '' : formatDecimal(value))
  return (
    <tr className="border-t text-muted-foreground">
      <th scope="row" className="px-2 py-1 text-left font-normal">
        Còn lại ({rest.count} nhóm)
      </th>
      <td className="px-2 py-1 text-right tabular-nums">{number(rest.baseValue)}</td>
      <td className="px-2 py-1 text-right tabular-nums">{number(rest.compareValue)}</td>
      <td className="px-2 py-1 text-right tabular-nums">{number(rest.contribution)}</td>
    </tr>
  )
}

function groupName(group: GroupView, dimension: DimensionView): string {
  return group.value === null ? 'trống' : formatCell(group.value, dimension.kind)
}
