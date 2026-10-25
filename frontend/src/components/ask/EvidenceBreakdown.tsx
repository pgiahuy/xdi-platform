// SPDX-License-Identifier: Apache-2.0
/* components/ask/EvidenceBreakdown.tsx - tab Phân rã của pane bằng chứng: số của hai lát cắt và chênh lệch, lời giải
thích, mỗi chiều một bảng đóng góp theo đúng thứ tự backend gửi (chiều giải thích tốt nhất trước, nhóm đóng góp lớn
trước), rồi phần lượng và giá nếu có. Bấm số của một lát cắt trong bảng thì trang mở tab Dòng nguồn với đúng lát cắt,
cột và giá trị nhóm đó (onDrill).
Số là chuỗi chữ số của backend, chỉ nhóm nghìn trên chuỗi (formatDecimal), không qua Number() nên không làm tròn. */

import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { formatCell, formatDecimal } from './cellFormat'
import type { BreakdownView, DimensionView, DrillTarget, GroupView, Slice, VolumePriceView } from './evidenceTypes'

type Props = {
  breakdown: BreakdownView
  onDrill: (target: DrillTarget) => void
}

export function EvidenceBreakdown({ breakdown, onDrill }: Props) {
  return (
    <div className="flex flex-col gap-4">
      <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 text-sm">
        <dt className="text-muted-foreground">{breakdown.baseLabel}</dt>
        <dd className="text-right tabular-nums">{formatDecimal(breakdown.baseValue)}</dd>
        <dt className="text-muted-foreground">{breakdown.compareLabel}</dt>
        <dd className="text-right tabular-nums">{formatDecimal(breakdown.compareValue)}</dd>
        <dt className="text-muted-foreground">Chênh lệch</dt>
        <dd className="text-right font-medium tabular-nums">{formatDecimal(breakdown.change)}</dd>
      </dl>
      <div className="flex flex-col gap-1">
        <p>{breakdown.summary}</p>
        {!breakdown.summaryByModel && (
          <p className="text-xs text-muted-foreground">
            Câu này viết theo khuôn từ các số đã tính, không phải lời của mô hình.
          </p>
        )}
      </div>
      {breakdown.dimensions.length > 0 && (
        <p className="text-xs text-muted-foreground">
          {breakdown.measure} theo từng nhóm; chiều đầu tiên giải thích chênh lệch tốt nhất. Bấm số của một kỳ để xem
          dòng nguồn của nhóm đó.
        </p>
      )}
      {breakdown.dimensions.map((dimension) => (
        <DimensionTable
          key={dimension.column}
          dimension={dimension}
          baseLabel={breakdown.baseLabel}
          compareLabel={breakdown.compareLabel}
          onDrill={onDrill}
        />
      ))}
      {breakdown.volumePrice && (
        <VolumePriceTable
          volumePrice={breakdown.volumePrice}
          measure={breakdown.measure}
          baseLabel={breakdown.baseLabel}
          compareLabel={breakdown.compareLabel}
        />
      )}
    </div>
  )
}

type DimensionProps = {
  dimension: DimensionView
  baseLabel: string
  compareLabel: string
  onDrill: (target: DrillTarget) => void
}

function DimensionTable({ dimension, baseLabel, compareLabel, onDrill }: DimensionProps) {
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
        onClick={() => onDrill({ slice, column: dimension.column, value: group.value, kind: dimension.kind })}
      >
        {formatDecimal(number)}
      </Button>
    )
  }

  // Bốn cột số không vừa bề ngang hẹp thì cuộn ngang trong khung, trang không cuộn; khung nhận focus để cuộn bằng phím.
  return (
    <div tabIndex={0} role="region" aria-label={`Phân rã theo ${dimension.column}`} className="overflow-x-auto">
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
          {dimension.groups.map((group, index) => (
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
        </tbody>
      </table>
    </div>
  )
}

function groupName(group: GroupView, dimension: DimensionView): string {
  return group.value === null ? 'trống' : formatCell(group.value, dimension.kind)
}

type VolumePriceProps = {
  volumePrice: VolumePriceView
  measure: string
  baseLabel: string
  compareLabel: string
}

function VolumePriceTable({ volumePrice: vp, measure, baseLabel, compareLabel }: VolumePriceProps) {
  return (
    <div className="flex flex-col gap-2">
      <div tabIndex={0} role="region" aria-label="Lượng và giá" className="overflow-x-auto">
        <table className="w-full text-xs">
          <caption className="pb-1 text-left text-sm font-semibold">Lượng và giá</caption>
          <thead className="bg-muted/50">
            <tr>
              <th scope="col" className="px-2 py-1.5 text-left font-medium">
                <span className="sr-only">Chỉ số</span>
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
            <VolumePriceRow
              label={vp.volumeLabel}
              base={vp.baseVolume}
              compare={vp.compareVolume}
              effect={vp.volumeEffect}
            />
            <VolumePriceRow
              label={vp.priceLabel}
              base={vp.basePrice}
              compare={vp.comparePrice}
              effect={vp.priceEffect}
            />
          </tbody>
        </table>
      </div>
      <p className="text-xs text-muted-foreground">
        Nếu giữ {vp.volumeLabel} như {baseLabel}: {measure} của {compareLabel} là{' '}
        <span className="tabular-nums">{formatDecimal(vp.ifVolumeUnchanged)}</span>.
      </p>
    </div>
  )
}

type VolumePriceRowProps = { label: string; base: string; compare: string; effect: string }

function VolumePriceRow({ label, base, compare, effect }: VolumePriceRowProps) {
  return (
    <tr className="border-t">
      <th scope="row" className="px-2 py-1 text-left font-normal">
        {label}
      </th>
      <td className="px-2 py-1 text-right tabular-nums">{formatDecimal(base)}</td>
      <td className="px-2 py-1 text-right tabular-nums">{formatDecimal(compare)}</td>
      <td className="px-2 py-1 text-right tabular-nums">{formatDecimal(effect)}</td>
    </tr>
  )
}
