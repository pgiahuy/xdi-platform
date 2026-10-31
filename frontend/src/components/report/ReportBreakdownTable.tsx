// SPDX-License-Identifier: Apache-2.0
/* components/report/ReportBreakdownTable.tsx - phân rã của một mục: chênh lệch giữa hai kỳ hay hai nhóm, và các nhóm của
chiều đầu tạo ra phần lớn chênh lệch đó (tối đa năm nhóm, backend chọn). Số là chuỗi chữ số thập phân như backend tính
(phần cộng lại bằng đúng chênh lệch), chỉ đổi cách hiện bằng formatDecimal. */

import { useId } from 'react'
import { formatDecimal, formatOptionalDecimal } from '@/components/ask/cellFormat'
import type { BreakdownView } from './types'

type Props = {
  breakdown: BreakdownView
}

export function ReportBreakdownTable({ breakdown }: Props) {
  const headingId = useId()
  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-2 break-inside-avoid">
      <h4 id={headingId} className="text-sm font-semibold">
        Phân rã {breakdown.measure}
      </h4>
      <p className="text-xs text-muted-foreground tabular-nums">
        {breakdown.compareLabel}: {formatDecimal(breakdown.compareValue)}, {breakdown.baseLabel}:{' '}
        {formatDecimal(breakdown.baseValue)}, chênh lệch {formatDecimal(breakdown.change)}
      </p>
      <div tabIndex={0} role="region" aria-labelledby={headingId} className="overflow-x-auto rounded-[10px] border">
        <table className="w-full text-sm tabular-nums">
          <thead className="bg-background text-xs text-muted-foreground">
            <tr>
              <th scope="col" className="px-3 py-2 text-left font-semibold">
                {breakdown.dimension}
              </th>
              <th scope="col" className="px-3 py-2 text-right font-semibold">
                {breakdown.baseLabel}
              </th>
              <th scope="col" className="px-3 py-2 text-right font-semibold">
                {breakdown.compareLabel}
              </th>
              <th scope="col" className="px-3 py-2 text-right font-semibold">
                Đóng góp
              </th>
              <th scope="col" className="px-3 py-2 text-right font-semibold">
                Phần của chênh lệch
              </th>
            </tr>
          </thead>
          <tbody>
            {breakdown.groups.map((group, index) => (
              <tr key={index} className="border-t">
                <td className="px-3 py-1.5">{group.name || '(trống)'}</td>
                <td className="px-3 py-1.5 text-right">{formatDecimal(group.baseValue)}</td>
                <td className="px-3 py-1.5 text-right">{formatDecimal(group.compareValue)}</td>
                <td className="px-3 py-1.5 text-right">{formatDecimal(group.contribution)}</td>
                <td className="px-3 py-1.5 text-right">{formatShare(group.shareOfChange)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  )
}

/** Phần của chênh lệch backend tính sẵn theo phần trăm, một chữ số lẻ ("42.5"). */
function formatShare(value: string | null): string {
  return value === null ? '' : `${formatOptionalDecimal(value)}%`
}
