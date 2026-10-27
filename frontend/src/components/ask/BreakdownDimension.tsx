// SPDX-License-Identifier: Apache-2.0
/* components/ask/BreakdownDimension.tsx - bảng đóng góp của một chiều trong tab Phân rã (EvidenceBreakdown.tsx). Chiều
nhiều nhóm hiện SHOWN_GROUPS nhóm đầu và một dòng "Còn lại" có tổng (breakdownGroups.ts), nút "Xem tất cả" mở đủ mọi
nhóm. Bấm số của một lát cắt ở một nhóm thì trang mở tab Dòng nguồn với đúng lát cắt, cột và giá trị nhóm (onDrill);
dòng "Còn lại" gộp nhiều nhóm nên số của nó không bấm được.
Độ đo tỷ lệ (dimension.mixEffect khác null) thêm hai cột "Do cơ cấu" và "Do hiệu suất", dòng "Tổng" của hai phần, và một
câu giải thích hai phần; ô tỷ lệ của dòng "Còn lại" để trống vì tỷ lệ các nhóm không cộng được. */

import { useState } from 'react'

import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { splitGroups, sumDecimals, type RestGroups } from './breakdownGroups'
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
  const ratio = dimension.mixEffect !== null && dimension.rateEffect !== null

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
          <thead className="bg-background text-muted-foreground">
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
              {ratio && (
                <>
                  <th scope="col" className="px-2 py-1.5 text-right font-medium">
                    Do cơ cấu
                  </th>
                  <th scope="col" className="px-2 py-1.5 text-right font-medium">
                    Do hiệu suất
                  </th>
                </>
              )}
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
                {ratio && (
                  <>
                    <td className="px-2 py-1 text-right tabular-nums">{formatDecimal(group.mixEffect ?? '')}</td>
                    <td className="px-2 py-1 text-right tabular-nums">{formatDecimal(group.rateEffect ?? '')}</td>
                  </>
                )}
              </tr>
            ))}
            {rest && !expanded && <RestRow rest={rest} ratio={ratio} />}
            {ratio && <TotalRow mix={dimension.mixEffect ?? ''} rate={dimension.rateEffect ?? ''} />}
          </tbody>
        </table>
      </div>
      {ratio && (
        <p className="text-xs text-muted-foreground">
          Số của hai kỳ là tỷ lệ của từng nhóm. Do cơ cấu: phần chênh lệch vì tỷ trọng của nhóm đổi. Do hiệu suất: phần
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

function RestRow({ rest, ratio }: { rest: RestGroups; ratio: boolean }) {
  return (
    <tr className="border-t text-muted-foreground">
      <th scope="row" className="px-2 py-1 text-left font-normal">
        Còn lại ({rest.count} nhóm)
      </th>
      <td className="px-2 py-1 text-right tabular-nums">{formatOptional(rest.baseValue)}</td>
      <td className="px-2 py-1 text-right tabular-nums">{formatOptional(rest.compareValue)}</td>
      <td className="px-2 py-1 text-right tabular-nums">{formatOptional(rest.contribution)}</td>
      {ratio && (
        <>
          <td className="px-2 py-1 text-right tabular-nums">{formatOptional(rest.mixEffect)}</td>
          <td className="px-2 py-1 text-right tabular-nums">{formatOptional(rest.rateEffect)}</td>
        </>
      )}
    </tr>
  )
}

// Dòng tổng của độ đo tỷ lệ: tổng hai phần do backend tính; đóng góp là tổng của hai phần, bằng chênh lệch.
function TotalRow({ mix, rate }: { mix: string; rate: string }) {
  return (
    <tr className="border-t font-semibold">
      <th scope="row" className="px-2 py-1 text-left">
        Tổng
      </th>
      <td />
      <td />
      <td className="px-2 py-1 text-right tabular-nums">{formatOptional(sumDecimals([mix, rate]))}</td>
      <td className="px-2 py-1 text-right tabular-nums">{formatOptional(mix)}</td>
      <td className="px-2 py-1 text-right tabular-nums">{formatOptional(rate)}</td>
    </tr>
  )
}

function formatOptional(value: string | null): string {
  return value === null ? '' : formatDecimal(value)
}

function groupName(group: GroupView, dimension: DimensionView): string {
  return group.value === null ? 'trống' : formatCell(group.value, dimension.kind)
}
