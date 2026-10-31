// SPDX-License-Identifier: Apache-2.0
/* components/datasets/SourceTablePicker.tsx - danh sách mọi bảng, view tài khoản nguồn đọc được trên màn kết nối
(#247): nhóm theo schema, mỗi bảng ghi số dòng ước lượng, dung lượng, số cột, comment, trạng thái (đã chụp, chỉ có cấu
trúc) và nhãn khi bảng quá trần dòng; tick, bỏ tick từng bảng hay cả schema; bỏ tick bảng danh mục thì hiện câu báo
hệ quả ngay dưới bảng đó; một nút "Chụp N bảng". Nhóm, lựa chọn và câu báo do trang dựng (pages/sourceTablesView.ts). */

import { useId } from 'react'
import { TriangleAlert } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { formatBytes } from './formatBytes'
import type { SourceTableGroup, SourceTableRow } from './types'

type Props = {
  groups: SourceTableGroup[]
  tickedCount: number
  onToggle: (table: string, ticked: boolean) => void
  onToggleSchema: (schema: string, ticked: boolean) => void
  onSnapshot: () => void
  busy: boolean
}

const numberFormat = new Intl.NumberFormat('vi-VN')

export function SourceTablePicker({ groups, tickedCount, onToggle, onToggleSchema, onSnapshot, busy }: Props) {
  const baseId = useId()
  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(event) => {
        event.preventDefault()
        onSnapshot()
      }}
    >
      <fieldset className="flex flex-col gap-4" disabled={busy}>
        <legend className="mb-1 text-sm font-medium">Bảng sẽ chụp</legend>
        {groups.map((group, groupIndex) => (
          <SchemaSection
            key={group.schema}
            id={`${baseId}-${groupIndex}`}
            group={group}
            onToggle={onToggle}
            onToggleSchema={onToggleSchema}
          />
        ))}
      </fieldset>
      <div className="flex flex-wrap items-center gap-3">
        <Button type="submit" disabled={busy || tickedCount === 0}>
          {busy ? `Đang chụp ${tickedCount} bảng...` : `Chụp ${tickedCount} bảng`}
        </Button>
        {tickedCount === 0 && <span className="text-sm text-muted-foreground">Tick ít nhất một bảng để chụp.</span>}
      </div>
    </form>
  )
}

type SectionProps = {
  id: string
  group: SourceTableGroup
  onToggle: Props['onToggle']
  onToggleSchema: Props['onToggleSchema']
}

function SchemaSection({ id, group, onToggle, onToggleSchema }: SectionProps) {
  const ticked = group.rows.filter((r) => r.ticked).length
  const all = ticked === group.rows.length
  return (
    <section aria-labelledby={`${id}-heading`} className="flex flex-col gap-1">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
        <h3 id={`${id}-heading`} className="text-xs font-semibold tracking-wider text-label uppercase">
          Schema {group.schema}
        </h3>
        <span className="text-xs text-muted-foreground">
          {ticked}/{group.rows.length} bảng
        </span>
        <div className="ml-auto flex gap-1">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            aria-label={`Chọn hết bảng của schema ${group.schema}`}
            disabled={all}
            onClick={() => onToggleSchema(group.schema, true)}
          >
            Chọn hết
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            aria-label={`Bỏ hết bảng của schema ${group.schema}`}
            disabled={ticked === 0}
            onClick={() => onToggleSchema(group.schema, false)}
          >
            Bỏ hết
          </Button>
        </div>
      </div>
      <ul className="divide-y rounded-md border">
        {group.rows.map((row, index) => (
          <TableRowItem key={row.table} id={`${id}-${index}`} row={row} onToggle={onToggle} />
        ))}
      </ul>
    </section>
  )
}

function TableRowItem({ id, row, onToggle }: { id: string; row: SourceTableRow; onToggle: Props['onToggle'] }) {
  return (
    <li className="flex flex-col gap-1 px-3 py-2">
      <div className="flex items-start gap-3">
        <input
          id={id}
          type="checkbox"
          className="mt-1 size-4 shrink-0 accent-primary"
          checked={row.ticked}
          aria-describedby={`${id}-detail`}
          onChange={(event) => onToggle(row.table, event.target.checked)}
        />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
            <label htmlFor={id} className="text-sm font-medium break-all">
              <code>{row.name}</code>
              {row.isView && <span className="ml-1 text-xs font-normal text-muted-foreground">(view)</span>}
            </label>
            <StateBadge row={row} />
          </div>
          <p id={`${id}-detail`} className="text-xs break-words text-muted-foreground">
            {detailText(row)}
          </p>
          {row.comment && <p className="line-clamp-2 text-xs text-muted-foreground">{row.comment}</p>}
          {row.state === 'metadata_only' && row.reason && (
            <p className="text-xs break-words text-muted-foreground">{row.reason}</p>
          )}
        </div>
      </div>
      {row.warning && (
        <p
          role="note"
          className="ml-7 flex gap-2 rounded-md border border-highlight-border bg-highlight-soft px-2 py-1.5 text-xs"
        >
          <TriangleAlert aria-hidden className="mt-0.5 size-3.5 shrink-0 text-highlight-text" />
          <span>{row.warning}</span>
        </p>
      )}
    </li>
  )
}

const badgeClass = 'rounded-full border px-2 py-0.5 text-[11px] leading-none whitespace-nowrap'

function StateBadge({ row }: { row: SourceTableRow }) {
  if (row.state === 'snapshotted') return <span className={`${badgeClass} text-muted-foreground`}>Đã chụp</span>
  if (row.state === 'metadata_only') {
    return (
      <span className={`${badgeClass} border-highlight-border bg-highlight-soft text-highlight-text`}>
        Chỉ có cấu trúc
      </span>
    )
  }
  if (row.overRowLimit) {
    return (
      <span className={`${badgeClass} border-highlight-border bg-highlight-soft text-highlight-text`}>
        Quá trần dòng, chỉ lưu cấu trúc
      </span>
    )
  }
  return null
}

function detailText(row: SourceTableRow): string {
  // Số dòng là ước lượng của Postgres (lần ANALYZE gần nhất), ghi "khoảng" để không ai lấy làm số dòng thật.
  const rows = row.estimatedRows === null ? 'chưa rõ số dòng' : `khoảng ${numberFormat.format(row.estimatedRows)} dòng`
  const parts = [rows]
  if (row.sizeBytes !== null) parts.push(formatBytes(row.sizeBytes))
  parts.push(`${row.columnCount} cột`)
  return parts.join(', ')
}
