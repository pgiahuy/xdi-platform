// SPDX-License-Identifier: Apache-2.0
/* components/ask/EvidenceBreakdown.tsx - tab Phân rã của pane bằng chứng: mỗi chiều một bảng đóng góp theo đúng thứ
tự backend gửi (chiều giải thích tốt nhất trước, nhóm đóng góp lớn trước; bảng của một chiều ở BreakdownDimension.tsx),
rồi phần lượng và giá nếu có. Đầu tab có ba dòng nói chung mỗi cột hệ thống tính thêm cho biết gì (ReadingGuide), viết
bằng lời thường cho người dùng nghiệp vụ: không ai nói thì họ không đọc được cột "Đóng góp". Lời giải thích và thanh
đóng góp đã ở thẻ trả lời (WhyOverview) ngay bên cạnh nên không lặp lại; chỉ chênh lệch được nhắc lại ở dòng Tổng của
mỗi bảng, để thấy cột đóng góp cộng lại đúng bằng nó.
Số là chuỗi chữ số của backend, chỉ nhóm nghìn trên chuỗi (formatDecimal), không qua Number() nên không làm tròn. */

import { Info } from 'lucide-react'

import { BreakdownDimension } from './BreakdownDimension'
import { ContributionCell, type SliceTotals } from './BreakdownRows'
import { formatDecimal } from './cellFormat'
import { directionWord } from './changeFormat'
import type { BreakdownView, DrillTarget, VolumePriceView } from './evidenceTypes'

type Props = {
  breakdown: BreakdownView
  onDrill: (target: DrillTarget) => void
}

export function EvidenceBreakdown({ breakdown, onDrill }: Props) {
  const totals: SliceTotals = breakdown
  return (
    <div className="flex flex-col gap-4">
      {breakdown.dimensions.length > 0 && <ReadingGuide breakdown={breakdown} />}
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

/** Thẻ xanh dương nói ý nghĩa chung của ba cột hệ thống tính thêm, viết cho người không làm tài chính: không chèn tên hay
 * số của câu hỏi, chỉ chữ chiều đổi theo phép so (tăng, giảm giữa hai kỳ; cao hơn, thấp hơn giữa hai nhóm). Màu xanh
 * dương (token --info ở index.css) để người dùng nhận ra đây là thông tin bổ sung, khác cảnh báo hổ phách. Dòng %
 * Thay đổi chỉ có khi ít nhất một bảng có cột đó (độ đo cộng được, xem BreakdownDimension.tsx). */
function ReadingGuide({ breakdown }: { breakdown: BreakdownView }) {
  const up = directionWord('up', breakdown.comparison)
  const down = directionWord('down', breakdown.comparison)
  const additive = breakdown.dimensions.some((dimension) => dimension.mixEffect === null)
  return (
    <aside
      aria-labelledby="breakdown-guide-heading"
      className="flex gap-2.5 rounded-[10px] border border-l-[3px] border-info-border border-l-info bg-info-soft px-3.5 py-3 text-xs text-foreground"
    >
      {/* Icon cạnh tiêu đề chỉ giúp nhận ra loại thẻ; tiêu đề đã nói đủ nên trình đọc màn hình bỏ qua icon. */}
      <Info aria-hidden className="mt-0.5 size-4 shrink-0 text-info-text" />
      <div className="flex flex-col gap-1.5">
        <p id="breakdown-guide-heading" className="font-semibold text-info-text">
          Cách đọc các cột tính thêm
        </p>
        <ul className="flex flex-col gap-1">
          <li>
            <span className="font-semibold">Đóng góp</span>: phần chênh lệch đến từ nhóm này. Số đỏ là nhóm làm kết
            quả {down}, số xanh là nhóm làm kết quả {up}.
          </li>
          <li>
            <span className="font-semibold">% Tác động</span>: nhóm này chiếm bao nhiêu phần trong toàn bộ chênh lệch.
            Cả cột cộng lại khoảng 100%, số càng lớn thì nhóm càng ảnh hưởng nhiều.
          </li>
          {additive && (
            <li>
              <span className="font-semibold">% Thay đổi</span>: riêng trong nhóm này, con số {up} hay {down} bao nhiêu
              phần trăm. Nhóm in đậm là nhóm đáng kể mà thay đổi khác hẳn mặt bằng chung, nên xem kỹ.
            </li>
          )}
        </ul>
        <p>Bấm vào số ở hai cột số đầu tiên của một nhóm để xem các dòng dữ liệu tạo nên số đó.</p>
      </div>
    </aside>
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
      <ContributionCell value={effect} />
    </tr>
  )
}
