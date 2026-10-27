// SPDX-License-Identifier: Apache-2.0
/* components/ask/EvidenceBreakdown.tsx - tab Phân rã của pane bằng chứng: mỗi chiều một bảng đóng góp theo đúng thứ tự backend gửi (chiều giải thích tốt nhất trước, nhóm đóng góp lớn
trước; bảng của một chiều ở BreakdownDimension.tsx), rồi phần lượng và giá nếu có. Hai kỳ, chênh lệch và lời giải
thích đã ở thẻ trả lời (WhyOverview) ngay bên cạnh; nhắc lại ở đây làm hai cột nói cùng một điều.
Số là chuỗi chữ số của backend, chỉ nhóm nghìn trên chuỗi (formatDecimal), không qua Number() nên không làm tròn. */

import { BreakdownDimension } from './BreakdownDimension'
import { formatDecimal } from './cellFormat'
import type { BreakdownView, DrillTarget, VolumePriceView } from './evidenceTypes'

type Props = {
  breakdown: BreakdownView
  onDrill: (target: DrillTarget) => void
}

export function EvidenceBreakdown({ breakdown, onDrill }: Props) {
  return (
    <div className="flex flex-col gap-4">
      {breakdown.dimensions.length > 0 && (
        <p className="text-xs text-muted-foreground">Bấm số của một kỳ để xem dòng nguồn của nhóm đó.</p>
      )}
      {breakdown.dimensions.map((dimension) => (
        <BreakdownDimension
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
          <thead className="bg-background text-muted-foreground">
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
