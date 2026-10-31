// SPDX-License-Identifier: Apache-2.0
/* components/report/ReportMethod.tsx - phần "Cách tính" của một mục: độ đo, bộ lọc (gồm điều kiện phân quyền theo vai
dữ liệu), nhóm theo, kỳ, sắp xếp và giới hạn. Nhãn do backend dựng bằng code từ câu SQL vừa chạy, như thẻ của trang hỏi;
ở đây chỉ đọc, không đổi giả định được. Mục trống thì không hiện dòng đó. */

import type { MethodView } from './types'

type Props = {
  method: MethodView
}

export function ReportMethod({ method }: Props) {
  const rows: [string, string[]][] = [
    ['Độ đo', method.measures],
    ['Bộ lọc', method.filters],
    ['Nhóm theo', method.groupBy],
    ['Kỳ', method.timeRange ? [method.timeRange] : []],
    ['Sắp xếp, giới hạn', method.order],
  ]
  const shown = rows.filter(([, values]) => values.length > 0)
  if (shown.length === 0) return null
  return (
    <section className="flex flex-col gap-1 text-sm break-inside-avoid">
      <h4 className="font-semibold">Cách tính</h4>
      <dl className="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-4 gap-y-1">
        {shown.map(([name, values]) => (
          <div key={name} className="contents">
            <dt className="text-muted-foreground">{name}</dt>
            <dd>{values.join('; ')}</dd>
          </div>
        ))}
      </dl>
    </section>
  )
}
