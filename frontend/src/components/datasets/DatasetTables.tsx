// SPDX-License-Identifier: Apache-2.0
/* components/datasets/DatasetTables.tsx - phần mở ra của thẻ database trên trang Dữ liệu (DatasetCard.tsx, #257): bảng
nhóm theo schema, mỗi bảng ghi số dòng, mô tả ghi trong database nguồn (kèm nguồn của mô tả, #245) và các nút của bảng
(Tổng quan, Chỉ số, Xóa); bảng chỉ có cấu trúc (#247) nằm cùng schema, có nhãn "Chỉ có cấu trúc", lý do và nút chụp
riêng bảng đó cho admin; dưới cùng là quan hệ khóa ngoại giữa các bảng, bảng nào trỏ tới bảng nào.
TableActions dùng chung cho thẻ file (một bảng) và từng bảng của database. */

import { ConfirmDialog } from '@/components/ConfirmDialog'
import { DescriptionText } from '@/components/DescriptionText'
import { Button, buttonVariants } from '@/components/ui/button'
import { buildSchemaGroups } from '../tableGroups'
import type { DatasetCardProps } from './DatasetCard'
import type { MetadataTableRow, RelationRow, TableRow } from './types'

type Entry = { schema: string | null; name: string } & (
  | { kind: 'table'; table: TableRow }
  | { kind: 'metadata'; table: MetadataTableRow }
)

/** Bảng đã chụp và bảng chỉ có cấu trúc xếp chung theo schema rồi theo tên, để admin thấy đủ bảng của mỗi schema. */
function buildEntries(tables: TableRow[], metadataOnly: MetadataTableRow[]): Entry[] {
  const entries: Entry[] = [
    ...tables.map((t): Entry => ({ kind: 'table', schema: t.schema, name: t.name, table: t })),
    ...metadataOnly.map((t): Entry => ({ kind: 'metadata', schema: t.schema, name: t.name, table: t })),
  ]
  const key = (e: Entry) => `${e.schema ?? ''}.${e.name}`
  return entries.sort((a, b) => key(a).localeCompare(key(b)))
}

const numberFormat = new Intl.NumberFormat('vi-VN')

export function DatasetTables(props: DatasetCardProps) {
  const { dataset } = props
  const groups = buildSchemaGroups(buildEntries(dataset.tables, dataset.metadataOnly))
  return (
    <div className="flex flex-col gap-4 border-t pt-3">
      {groups.map((group) => (
        <section key={group.schema ?? ''} className="flex flex-col gap-1">
          {group.schema && (
            <h3 className="text-xs font-semibold tracking-wider text-label uppercase">Schema {group.schema}</h3>
          )}
          <ul className="divide-y">
            {group.tables.map((entry) =>
              entry.kind === 'table' ? (
                <li key={entry.table.id} className="flex flex-wrap items-center gap-x-4 gap-y-1 py-2">
                  <div className="min-w-0 flex-1 basis-48">
                    <p className="text-sm">
                      <code className="break-all">{entry.table.name}</code>{' '}
                      <span className="text-xs whitespace-nowrap text-muted-foreground">{rowText(entry.table)}</span>
                    </p>
                    {entry.table.description && <DescriptionText text={entry.table.description} source="database" />}
                  </div>
                  <TableActions {...props} table={entry.table} name={entry.table.name} />
                </li>
              ) : (
                <MetadataTableItem key={entry.table.table} {...props} table={entry.table} />
              ),
            )}
          </ul>
        </section>
      ))}
      {dataset.relations.length > 0 && <RelationList relations={dataset.relations} />}
    </div>
  )
}

function MetadataTableItem(props: DatasetCardProps & { table: MetadataTableRow }) {
  const { table, onSnapshotTable, snapshotTablePending } = props
  const pending = snapshotTablePending === table.table
  const rows = table.rowCount === null ? '' : `khoảng ${numberFormat.format(table.rowCount)} dòng, `
  return (
    <li className="flex flex-wrap items-center gap-x-4 gap-y-1 py-2">
      <div className="min-w-0 flex-1 basis-48">
        <p className="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm">
          <code className="break-all">{table.name}</code>
          <span className="rounded-full border border-highlight-border bg-highlight-soft px-2 py-0.5 text-[11px] leading-none whitespace-nowrap text-highlight-text">
            Chỉ có cấu trúc
          </span>
          <span className="text-xs whitespace-nowrap text-muted-foreground">
            {rows}
            {table.columnCount} cột
          </span>
        </p>
        <p className="text-xs break-words text-muted-foreground">{table.reason}</p>
        {table.description && <DescriptionText text={table.description} source="database" />}
      </div>
      {onSnapshotTable && (
        <Button
          variant="outline"
          size="sm"
          aria-label={`Chụp bảng ${table.table}`}
          aria-disabled={snapshotTablePending !== undefined}
          onClick={() => snapshotTablePending === undefined && onSnapshotTable(table)}
        >
          {pending ? 'Đang chụp...' : 'Chụp bảng này'}
        </Button>
      )}
    </li>
  )
}

function RelationList({ relations }: { relations: RelationRow[] }) {
  return (
    <section className="flex flex-col gap-1">
      <h3 className="text-xs font-semibold tracking-wider text-label uppercase">Quan hệ giữa các bảng</h3>
      <ul className="flex flex-col gap-0.5 text-xs text-muted-foreground">
        {relations.map((r) => (
          <li key={r.key} className="break-words">
            <code>{columnsText(r.from, r.fromColumns)}</code> trỏ tới <code>{columnsText(r.to, r.toColumns)}</code>
          </li>
        ))}
      </ul>
    </section>
  )
}

function columnsText(table: string, columns: string[]): string {
  return columns.length === 1 ? `${table}.${columns[0]}` : `${table}(${columns.join(', ')})`
}

function rowText(table: TableRow): string {
  return table.rowCount === null ? 'chưa có dữ liệu' : `${numberFormat.format(table.rowCount)} dòng`
}

type TableActionsProps = DatasetCardProps & {
  table: TableRow
  /** Tên trong aria-label của nút: tên file với thẻ file, tên bảng với bảng của database. */
  name: string
}

export function TableActions(props: TableActionsProps) {
  const { dataset, table, name, link: Link, onAddVersion, onDeleteTable, deletePending } = props
  const file = dataset.kind === 'file'
  return (
    <div className="flex flex-wrap items-center gap-2">
      {/* Mỗi dòng một nút cùng chữ; aria-label thêm tên để trình đọc màn hình phân biệt, chữ hiện ra đứng đầu để
          người dùng điều khiển bằng giọng nói vẫn gọi được nút. */}
      <Link
        href={props.overviewHref(table)}
        aria-label={`Tổng quan của ${name}`}
        className={buttonVariants({ variant: 'outline', size: 'sm' })}
      >
        Tổng quan
      </Link>
      <Link
        href={props.metricsHref(table)}
        aria-label={`Chỉ số của ${name}`}
        className={buttonVariants({ variant: 'outline', size: 'sm' })}
      >
        Chỉ số
      </Link>
      {file && onAddVersion && (
        <Button variant="outline" size="sm" aria-label={`Nạp phiên bản mới cho ${name}`} onClick={() => onAddVersion(table)}>
          Nạp phiên bản mới
        </Button>
      )}
      {onDeleteTable && (
        <ConfirmDialog
          trigger={
            <Button variant="outline" size="sm" aria-label={`Xóa ${name}`} aria-disabled={deletePending}>
              Xóa
            </Button>
          }
          title={`Xóa ${name}?`}
          description={
            file
              ? 'Dataset, mọi phiên bản, file và lịch sử câu hỏi của nó bị xóa, kể cả các cuộc trò chuyện. Không hoàn tác được.'
              : 'Bảng, mọi phiên bản và lịch sử câu hỏi bắt đầu từ nó bị xóa, kể cả các cuộc trò chuyện; quan hệ với các ' +
                'bảng khác bị xóa theo, các bảng đó vẫn giữ nguyên. Không hoàn tác được.'
          }
          confirmLabel={file ? 'Xóa dataset' : 'Xóa bảng'}
          onConfirm={() => onDeleteTable(table)}
        />
      )}
    </div>
  )
}
