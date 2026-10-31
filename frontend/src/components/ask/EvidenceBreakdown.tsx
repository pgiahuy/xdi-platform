// SPDX-License-Identifier: Apache-2.0
/* components/ask/EvidenceBreakdown.tsx - tab Phân rã của pane bằng chứng: một dòng quy mô của hai bên (số dòng dữ liệu,
số đơn khi tách được lượng), lời nhắc bấm số, ghi chú các cách chia đã bỏ, rồi mỗi chiều một bảng theo đúng thứ tự
backend gửi (bảng có nhóm khác thường trước, bảng dàn đều sau; bảng của một chiều ở BreakdownDimension.tsx), cuối cùng
phần lượng và giá nếu có. Nghĩa của từng cột nằm ngay dưới tiêu đề cột trong bảng, không gom thành khối chú thích ở đầu
tab. Lời giải thích và thanh chênh lệch đã ở thẻ trả lời (WhyOverview) ngay bên cạnh nên không lặp lại; chỉ chênh lệch
được nhắc lại ở dòng Tổng của mỗi bảng, để thấy cột chênh lệch cộng lại đúng bằng nó.
Ghi chú cách chia đã bỏ có ở đây chứ không chỉ trong lời tóm tắt: lời của mô hình có thể không nhắc, và người đọc tab cần
biết vì sao không có bảng Type hay Department Id.
Số là chuỗi chữ số của backend, chỉ nhóm nghìn trên chuỗi (formatDecimal), không qua Number() nên không làm tròn. */

import { BreakdownDimension } from './BreakdownDimension'
import { ContributionCell, type SliceTotals } from './BreakdownRows'
import { drillSubject, droppedText } from './breakdownText'
import { formatDecimal } from './cellFormat'
import type { BreakdownView, DrillTarget, VolumePriceView } from './evidenceTypes'

type Props = {
  breakdown: BreakdownView
  onDrill: (target: DrillTarget) => void
}

export function EvidenceBreakdown({ breakdown, onDrill }: Props) {
  const totals: SliceTotals = breakdown
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-1 text-xs text-muted-foreground">
        <SliceSizes breakdown={breakdown} />
        {breakdown.dimensions.length > 0 && (
          <p>
            Bấm vào {drillSubject(breakdown.comparison, breakdown.baseLabel, breakdown.compareLabel)} trong bảng để xem
            các dòng dữ liệu tạo nên số đó.
          </p>
        )}
        {breakdown.dropped.length > 0 && <DroppedNote breakdown={breakdown} />}
      </div>
      {breakdown.dimensions.map((dimension) => (
        <BreakdownDimension key={dimension.column} dimension={dimension} totals={totals} onDrill={onDrill} />
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

/** Quy mô của hai bên: số dòng dữ liệu, và số đơn (lượng) khi backend tách được. Chênh lệch lớn có khi chỉ vì một bên
 * ít dòng hơn nhiều (CANCELED so với COMPLETE). Độ đo đếm dòng thì số dòng chính là số đo, không nhắc lại. */
function SliceSizes({ breakdown: b }: { breakdown: BreakdownView }) {
  const vp = b.volumePrice
  const parts = []
  if (!b.countsRows) {
    parts.push(
      `Số dòng dữ liệu: ${b.baseLabel} ${formatDecimal(b.baseRows)}, ${b.compareLabel} ${formatDecimal(b.compareRows)}.`,
    )
  }
  if (vp) {
    parts.push(
      `${vp.volumeLabel}: ${b.baseLabel} ${formatDecimal(vp.baseVolume)}, ${b.compareLabel} ${formatDecimal(vp.compareVolume)}.`,
    )
  }
  if (parts.length === 0) return null
  return <p className="tabular-nums">{parts.join(' ')}</p>
}

/** Các cách chia không có bảng (hay lời tóm tắt bỏ qua), mỗi cách một câu nói vì sao. */
function DroppedNote({ breakdown: b }: { breakdown: BreakdownView }) {
  return (
    <div>
      <p className="font-medium text-foreground">Cách chia đã bỏ</p>
      <ul className="list-disc pl-5">
        {b.dropped.map((dropped) => (
          <li key={dropped.column}>{droppedText(dropped, b.comparison, b.baseLabel, b.compareLabel)}</li>
        ))}
      </ul>
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
                Chênh lệch
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
      <ContributionCell value={effect} />
    </tr>
  )
}
