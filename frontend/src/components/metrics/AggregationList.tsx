// SPDX-License-Identifier: Apache-2.0
/* components/metrics/AggregationList.tsx - cách gộp mặc định của từng cột số đo: tổng (tiền, số lượng hàng) hay trung
bình (giá trị mô tả một dòng như tuổi, điểm, số tín chỉ). Câu hỏi không nhắc chỉ số nào thì mô hình gộp cột theo cách
này, và câu "vì sao" trên cột trung bình so trung bình mỗi dòng. onChange chỉ truyền vào khi người xem là admin. */

import { NativeSelect } from './NativeSelect'
import type { Aggregation, ColumnItem } from './types'

const aggregationOptions: { value: Aggregation; label: string }[] = [
  { value: 'sum', label: 'Tổng' },
  { value: 'avg', label: 'Trung bình' },
]

type Props = {
  /** Cột measure của phiên bản hiện hành. */
  columns: ColumnItem[]
  onChange?: (column: string, aggregation: Aggregation) => void
  /** Cột đang lưu, để khóa ô của nó tới khi xong. */
  pendingColumn?: string
}

export function AggregationList({ columns, onChange, pendingColumn }: Props) {
  return (
    <ul className="divide-y rounded-xl border bg-card shadow-xs">
      {columns.map((column, i) => {
        const id = `aggregation-${i}`
        const value = column.aggregation ?? 'sum'
        return (
          <li key={column.name} className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3">
            <div className="min-w-0 flex-1 basis-56">
              <label htmlFor={onChange ? id : undefined} className="block truncate text-sm font-medium" title={column.name}>
                {column.name}
              </label>
              {column.description && <p className="text-xs text-muted-foreground">{column.description}</p>}
            </div>
            {onChange ? (
              <NativeSelect
                id={id}
                value={value}
                disabled={pendingColumn === column.name}
                onChange={(e) => onChange(column.name, e.target.value as Aggregation)}
                options={aggregationOptions}
              />
            ) : (
              <span className="text-sm">{aggregationOptions.find((o) => o.value === value)?.label}</span>
            )}
          </li>
        )
      })}
    </ul>
  )
}
