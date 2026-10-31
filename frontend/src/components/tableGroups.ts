// SPDX-License-Identifier: Apache-2.0
/* components/tableGroups.ts - nhóm bảng của một dataset theo schema của database nguồn (#257), cho ô chọn dataset ở
trang hỏi, thẻ dataset ở trang Dữ liệu và danh sách bảng ở trang Tổng quan. Backend đã xếp bảng theo schema rồi theo tên
(DatasetSummary.tables), nên chỉ gom liền nhau, giữ nguyên thứ tự. */

export type SchemaGroup<T> = {
  /** null với file, hay bảng không có schema. */
  schema: string | null
  tables: T[]
}

export function buildSchemaGroups<T extends { schema: string | null }>(tables: T[]): SchemaGroup<T>[] {
  const groups: SchemaGroup<T>[] = []
  for (const table of tables) {
    const last = groups.at(-1)
    if (last && last.schema === table.schema) last.tables.push(table)
    else groups.push({ schema: table.schema, tables: [table] })
  }
  return groups
}
