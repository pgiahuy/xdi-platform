// SPDX-License-Identifier: Apache-2.0
/* pages/sourceTablesView.ts - dựng danh sách bảng của màn kết nối (SourceTablePicker, #247) từ GET /sources/{id}/tables:
nhóm theo schema, bảng nào đang tick, câu cảnh báo khi bỏ tick một bảng danh mục, và thân của POST /sources/{id}/snapshot.
Lựa chọn của admin giữ ở trang dưới dạng các bảng tự bỏ tick hay tự tick thêm, còn lại theo mặc định, để danh sách đọc
lại từ database ngoài (bảng mới, bảng mất quyền) không làm mất lựa chọn đã có. */

import type { SnapshotRequest, SnapshotResponse, SourceTable } from '@/api/sources'
import type { SnapshotSummary, SourceTableGroup, SourceTableRow } from '@/components/datasets/types'

/** Bảng admin tự tick (true) hay tự bỏ (false); bảng không có ở đây theo mặc định. */
export type TickChoice = Record<string, boolean>

/** Nguồn chưa chụp bảng nào: tick sẵn mọi bảng. Đã chụp rồi: tick sẵn đúng các bảng nguồn đang giữ, để bảng admin đã bỏ
 * lúc trước không tự quay lại khi mở danh sách để chụp thêm. */
function tickedByDefault(tables: SourceTable[]): (table: SourceTable) => boolean {
  const held = tables.some((t) => t.state !== null)
  return (t) => !held || t.state !== null
}

export function isTicked(tables: SourceTable[], choice: TickChoice): (table: SourceTable) => boolean {
  const byDefault = tickedByDefault(tables)
  return (t) => choice[t.table] ?? byDefault(t)
}

/** Nguồn chưa có bảng nào trong dataset: danh sách mở sẵn, vì chụp là việc tiếp theo của admin. */
export function isFirstSnapshot(tables: SourceTable[]): boolean {
  return tables.every((t) => t.state === null)
}

/** "edu.courses" thành "courses" khi cùng schema với bảng đang nói tới; khác schema thì giữ tên đầy đủ. */
function shortName(table: string, schema: string): string {
  return table.startsWith(`${schema}.`) ? table.slice(schema.length + 1) : table
}

/** Bỏ tick bảng mà bảng đang tick trỏ tới: các bảng đó mất quan hệ, nên câu hỏi chia theo danh mục chỉ còn chia được
 * theo mã. Bảng trỏ tới nó cũng bị bỏ tick thì không có gì để báo. Các bảng trỏ tới bằng cùng bộ cột gom một cụm, để
 * khóa nhiều cột không lặp lại cho từng bảng. */
function untickWarning(t: SourceTable, ticked: (table: string) => boolean): string | null {
  if (!t.is_referenced) return null
  const byColumns = new Map<string, string[]>()
  for (const link of t.referenced_by) {
    if (link.from_table === t.table || !ticked(link.from_table)) continue
    const columns = link.from_columns.length === 1 ? link.from_columns[0] : `(${link.from_columns.join(', ')})`
    const tables = byColumns.get(columns) ?? []
    const name = shortName(link.from_table, t.schema)
    if (!tables.includes(name)) tables.push(name)
    byColumns.set(columns, tables)
  }
  if (byColumns.size === 0) return null
  const users = [...byColumns].map(([columns, tables]) => `cột ${columns} của bảng ${joinNames(tables)}`)
  return `Bảng ${t.name} chứa tên cho ${users.join('; ')}; bỏ thì câu hỏi theo danh mục chỉ chia được theo mã.`
}

function joinNames(names: string[]): string {
  return names.length <= 1 ? names.join('') : `${names.slice(0, -1).join(', ')} và ${names.at(-1)}`
}

/** Backend trả bảng theo schema rồi theo tên, nên chỉ gom các bảng liền nhau. */
export function buildSourceTableGroups(tables: SourceTable[], choice: TickChoice): SourceTableGroup[] {
  const tickedOf = isTicked(tables, choice)
  const tickedNames = new Set(tables.filter(tickedOf).map((t) => t.table))
  const groups: SourceTableGroup[] = []
  for (const t of tables) {
    const ticked = tickedNames.has(t.table)
    const row: SourceTableRow = {
      table: t.table,
      name: t.name,
      isView: t.kind === 'view',
      ticked,
      estimatedRows: t.estimated_rows,
      sizeBytes: t.size_bytes,
      columnCount: t.column_count,
      comment: t.comment,
      overLimit: t.over_limit,
      state: t.state,
      reason: t.reason,
      warning: ticked ? null : untickWarning(t, (name) => tickedNames.has(name)),
    }
    const last = groups.at(-1)
    if (last?.schema === t.schema) last.rows.push(row)
    else groups.push({ schema: t.schema, rows: [row] })
  }
  return groups
}

/** Tick hết thì gửi thân rỗng (backend chụp mọi bảng đọc được, kể cả bảng vừa thêm bên database); còn lại gửi đúng các
 * bảng được tick. */
export function buildSnapshotRequest(tables: SourceTable[], choice: TickChoice): SnapshotRequest {
  const ticked = tables.filter(isTicked(tables, choice)).map((t) => t.table)
  return ticked.length === tables.length ? {} : { tables: ticked }
}

/** Kết quả chụp cho SourcePanel và thẻ database: mỗi bảng chép được một dòng, bảng chỉ lưu cấu trúc kèm lý do. */
export function buildSnapshotSummary(result: SnapshotResponse): SnapshotSummary {
  return {
    tables: result.datasets.map((d) => ({
      datasetName: d.dataset.name,
      created: d.created,
      version: d.version.version,
      rowCount: d.version.row_count,
    })),
    relationCount: result.relations.length,
    metadataOnly: result.metadata_only.map((t) => ({ table: t.table, reason: t.reason })),
    unreadable: result.unreadable,
  }
}
