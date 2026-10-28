// SPDX-License-Identifier: Apache-2.0
/* components/overview/ColumnGroups.tsx - các cột của dataset trong khung tổng quan, xếp theo nhóm dễ hiểu thay vì theo
vai trò kỹ thuật: Số đo, Chia nhóm theo, Thời gian, Mã, Chữ khác. Mỗi cột ghi tên trong file, nhãn tiếng Việt trong
ngoặc, mô tả, đơn vị, % ô trống và số giá trị khác nhau; cột chia nhóm có thêm các giá trị nhiều dòng nhất.
Bấm tên cột hay một giá trị thì điền câu hỏi vào ô hỏi (onPick, câu dựng ở questions.ts). Nhóm dùng details của trình
duyệt: mở, đóng bằng bàn phím và trình đọc màn hình đọc được mà không cần thêm mã. Mọi nhóm đóng sẵn, dòng tiêu đề ghi
vài tên cột đầu: dataset vài chục cột mà mở hết thì biểu đồ bị đẩy xuống rất xa. */

import { useState } from 'react'
import { buildColumnQuestion, buildValueQuestion } from './questions'
import type { OverviewColumnView, OverviewRole, OverviewView } from './types'

type Props = {
  overview: OverviewView
  onPick: (question: string) => void
}

const groups: { role: OverviewRole; title: string }[] = [
  { role: 'measure', title: 'Số đo' },
  { role: 'dimension', title: 'Chia nhóm theo' },
  { role: 'time', title: 'Thời gian' },
  { role: 'key', title: 'Mã' },
  { role: 'text', title: 'Chữ khác' },
]

/** Số tên cột ghi ngay ở dòng tiêu đề của nhóm đang đóng. */
const PREVIEW_COLUMNS = 5

const unitLabels: Record<string, string> = {
  money: 'tiền',
  quantity: 'số lượng',
  percent: 'phần trăm',
  duration: 'thời lượng',
  other: 'đơn vị khác',
}

/** Số giá trị hiện sẵn của một cột chia nhóm; còn lại mở bằng nút. */
const VALUES_SHOWN = 6

const numberFormat = new Intl.NumberFormat('vi-VN')
const percentFormat = new Intl.NumberFormat('vi-VN', { maximumFractionDigits: 1 })

export function ColumnGroups({ overview, onPick }: Props) {
  return (
    <div className="flex flex-col gap-2">
      {groups.map((group) => {
        const columns = overview.columns.filter((column) => column.role === group.role)
        if (columns.length === 0) return null
        return (
          <details key={group.role} className="group rounded-lg border bg-background">
            <summary className="cursor-pointer px-3 py-2 text-sm font-medium">
              {group.title} <span className="font-normal text-muted-foreground">({columns.length} cột)</span>
              <span className="block truncate text-xs font-normal text-muted-foreground group-open:hidden">
                {buildPreview(columns)}
              </span>
            </summary>
            <ul className="grid border-t md:grid-cols-2">
              {columns.map((column) => (
                <ColumnItem key={column.name} column={column} overview={overview} onPick={onPick} />
              ))}
            </ul>
          </details>
        )
      })}
    </div>
  )
}

type ItemProps = {
  column: OverviewColumnView
  overview: OverviewView
  onPick: (question: string) => void
}

function ColumnItem({ column, overview, onPick }: ItemProps) {
  const [allValues, setAllValues] = useState(false)
  const values = allValues ? column.values : column.values.slice(0, VALUES_SHOWN)
  const facts = buildFacts(column)
  return (
    <li className="flex min-w-0 flex-col gap-1 border-b px-3 py-2">
      <button
        type="button"
        onClick={() => onPick(buildColumnQuestion(column, overview))}
        className="self-start text-left text-sm font-medium text-label underline-offset-2 hover:underline"
        title="Điền câu hỏi về cột này vào ô hỏi"
      >
        {column.name}
        {column.label && <span className="font-normal text-muted-foreground"> ({column.label})</span>}
      </button>
      {column.description && <p className="text-xs text-muted-foreground">{column.description}</p>}
      {facts && <p className="text-xs text-muted-foreground">{facts}</p>}
      {column.values.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {values.map((value) => (
            <button
              key={value.value}
              type="button"
              onClick={() => onPick(buildValueQuestion(column, value.value, overview))}
              className="rounded-md border bg-card px-2 py-0.5 text-xs hover:border-label hover:bg-label-soft"
              aria-label={`${value.value}, ${numberFormat.format(value.rows)} dòng: điền câu hỏi về giá trị này`}
            >
              {value.value}
              {value.label && value.label !== value.value && ` (${value.label})`}
              <span className="text-muted-foreground"> {numberFormat.format(value.rows)}</span>
            </button>
          ))}
          {column.values.length > VALUES_SHOWN && (
            <button
              type="button"
              onClick={() => setAllValues((shown) => !shown)}
              className="px-1 text-xs text-muted-foreground underline underline-offset-2"
            >
              {allValues ? 'Thu gọn' : `Hiện cả ${numberFormat.format(column.values.length)} giá trị`}
            </button>
          )}
        </div>
      )}
    </li>
  )
}

/** Vài tên cột đầu của nhóm (nhãn tiếng Việt khi có), để đọc được nhóm có gì mà không phải mở. */
function buildPreview(columns: OverviewColumnView[]): string {
  const names = columns.slice(0, PREVIEW_COLUMNS).map((column) => column.label ?? column.name)
  const more = columns.length > PREVIEW_COLUMNS ? `, và ${columns.length - PREVIEW_COLUMNS} cột khác` : ''
  return names.join(', ') + more
}

/** Một dòng chữ phụ: đơn vị, ô trống, số giá trị khác nhau, cờ nhạy cảm; null khi không có gì để ghi. */
function buildFacts(column: OverviewColumnView): string | null {
  const parts: string[] = []
  if (column.unit) parts.push(`đơn vị ${unitLabels[column.unit] ?? column.unit}`)
  if (column.nullPercent !== null && column.nullPercent > 0) {
    parts.push(`trống ${percentFormat.format(column.nullPercent)}%`)
  }
  if (column.distinctCount !== null) {
    parts.push(`${numberFormat.format(column.distinctCount)} giá trị khác nhau`)
  }
  if (column.sensitive) parts.push('nhạy cảm, không hiện giá trị')
  if (parts.length === 0) return null
  const text = parts.join(', ')
  return text.charAt(0).toUpperCase() + text.slice(1)
}
