// SPDX-License-Identifier: Apache-2.0
/* components/datasets/DatasetCard.tsx - một dataset trên trang Dữ liệu (#257): một database đã kết nối hay một file.
Thẻ database ghi tên, máy chủ (chỉ admin thấy), số bảng, lần chụp gần nhất, nút "Chụp lại" (admin, chụp lại mọi bảng
trong một giao dịch); mở ra thì thấy bảng nhóm theo schema và quan hệ giữa các bảng (DatasetTables.tsx). Thẻ file ghi
tên, số dòng, lần nạp, các nút của bảng duy nhất của nó, kèm "Nạp phiên bản mới" cho admin.
Nút chỉ admin dùng được (onSnapshot, onAddVersion, onDeleteTable) chỉ truyền vào khi người xem là admin. */

import { useState } from 'react'
import { ChevronDownIcon, ChevronRightIcon } from 'lucide-react'
import { FormMessage } from '@/components/forms/FormMessage'
import type { LinkComponent } from '@/components/shell/types'
import { Button } from '@/components/ui/button'
import { DatasetTables, TableActions } from './DatasetTables'
import type { DatasetCardView, TableRow } from './types'

export type DatasetCardProps = {
  dataset: DatasetCardView
  link: LinkComponent
  /** Trang Tổng quan, màn Chỉ số của một bảng (route của bảng nhận id bảng). */
  overviewHref: (table: TableRow) => string
  metricsHref: (table: TableRow) => string
  onSnapshot?: () => void
  snapshotPending?: boolean
  onAddVersion?: (table: TableRow) => void
  onDeleteTable?: (table: TableRow) => void
  deletePending?: boolean
  /** Kết quả hay lỗi của lần chụp lại, xóa vừa làm trên thẻ này. */
  message?: { kind: 'error' | 'success'; text: string }
}

const dateFormat = new Intl.DateTimeFormat('vi-VN', { dateStyle: 'short', timeStyle: 'short' })
const numberFormat = new Intl.NumberFormat('vi-VN')

export function DatasetCard(props: DatasetCardProps) {
  const { dataset, onSnapshot, snapshotPending, message } = props
  const [open, setOpen] = useState(false)
  const file = dataset.kind === 'file' ? dataset.tables[0] : undefined
  const tablesId = `dataset-tables-${dataset.id}`
  return (
    <li className="flex flex-col gap-3 rounded-xl border bg-card px-4 py-3 shadow-xs">
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
        <div className="min-w-0 flex-1 basis-56">
          <p className="flex flex-wrap items-baseline gap-x-2 text-sm font-medium">
            <span className="break-all">{dataset.name}</span>
            <span className="text-xs font-normal text-muted-foreground">
              {dataset.kind === 'database' ? 'Database' : 'File'}
            </span>
          </p>
          {dataset.connection && (
            <p className="text-xs break-all text-muted-foreground">
              <code>{dataset.connection}</code>
            </p>
          )}
          <p className="text-xs text-muted-foreground">{buildSummary(dataset)}</p>
        </div>
        {file && <TableActions {...props} table={file} name={dataset.name} />}
        {dataset.kind === 'database' && onSnapshot && dataset.tables.length > 0 && (
          <Button
            variant="outline"
            size="sm"
            aria-label={`Chụp lại ${dataset.name}`}
            aria-disabled={snapshotPending}
            onClick={() => !snapshotPending && onSnapshot()}
          >
            {snapshotPending ? 'Đang chụp...' : 'Chụp lại'}
          </Button>
        )}
        {dataset.kind === 'database' && dataset.tables.length > 0 && (
          <Button
            variant="ghost"
            size="sm"
            aria-expanded={open}
            aria-controls={tablesId}
            onClick={() => setOpen(!open)}
          >
            {open ? <ChevronDownIcon aria-hidden="true" /> : <ChevronRightIcon aria-hidden="true" />}
            {open ? 'Ẩn bảng' : 'Xem bảng'}
          </Button>
        )}
      </div>
      {message && <FormMessage kind={message.kind}>{message.text}</FormMessage>}
      {open && (
        <div id={tablesId}>
          <DatasetTables {...props} />
        </div>
      )}
    </li>
  )
}

function buildSummary(dataset: DatasetCardView): string {
  const at = dataset.updatedAt ? dateFormat.format(new Date(dataset.updatedAt)) : null
  if (dataset.kind === 'file') {
    const rows = dataset.tables[0]?.rowCount
    const parts = [rows === null || rows === undefined ? 'Chưa có dữ liệu' : `${numberFormat.format(rows)} dòng`]
    if (at) parts.push(`nạp lúc ${at}`)
    return parts.join(', ')
  }
  if (dataset.tables.length === 0) return 'Đã kết nối, chưa chụp bảng nào.'
  const parts = [`${dataset.tables.length} bảng`]
  if (dataset.relations.length > 0) parts.push(`${dataset.relations.length} quan hệ`)
  if (at) parts.push(`lần chụp gần nhất ${at}`)
  return parts.join(', ')
}
