// SPDX-License-Identifier: Apache-2.0
/* components/overview/TableIndex.tsx - danh sách bảng của một dataset trên trang Tổng quan dữ liệu (#257): bảng nhóm
theo schema, mỗi bảng ghi số dòng và mô tả ghi trong database nguồn. Bấm một bảng thì trang mở tổng quan của bảng đó
(khung tổng quan theo bảng có sẵn, DataOverview.tsx); bảng đang mở có aria-current. */

import { DescriptionText } from '@/components/DescriptionText'
import { cn } from '@/lib/utils'
import type { TableRow } from '../datasets/types'
import { buildSchemaGroups } from '../tableGroups'

type Props = {
  tables: TableRow[]
  selectedId: string | undefined
  onSelect: (tableId: string) => void
}

const numberFormat = new Intl.NumberFormat('vi-VN')

export function TableIndex({ tables, selectedId, onSelect }: Props) {
  return (
    <nav aria-label="Bảng của dataset" className="flex flex-col gap-3 rounded-xl border bg-card p-3 shadow-xs">
      {buildSchemaGroups(tables).map((group) => (
        <section key={group.schema ?? ''} className="flex flex-col gap-1">
          {group.schema && (
            <h2 className="px-2 text-xs font-semibold tracking-wider text-label uppercase">Schema {group.schema}</h2>
          )}
          <ul className="flex flex-col">
            {group.tables.map((table) => {
              const selected = table.id === selectedId
              return (
                <li key={table.id}>
                  <button
                    type="button"
                    aria-current={selected ? 'true' : undefined}
                    onClick={() => onSelect(table.id)}
                    className={cn(
                      'flex w-full flex-col gap-0.5 rounded-md px-2 py-1.5 text-left',
                      'hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none',
                      selected && 'bg-accent',
                    )}
                  >
                    <span className="flex flex-wrap items-baseline gap-x-2 text-sm">
                      <code className={cn('break-all', selected && 'font-semibold')}>{table.name}</code>
                      <span className="text-xs text-muted-foreground">
                        {table.rowCount === null ? 'chưa có dữ liệu' : `${numberFormat.format(table.rowCount)} dòng`}
                      </span>
                    </span>
                    {table.description && <DescriptionText text={table.description} source="database" as="span" />}
                  </button>
                </li>
              )
            })}
          </ul>
        </section>
      ))}
    </nav>
  )
}
