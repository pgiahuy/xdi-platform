// SPDX-License-Identifier: Apache-2.0
/* pages/relatedTablesView.ts - dựng phần chọn bảng chụp kèm (RelatedTablePicker) từ các lần gọi
POST /sources/{id}/relations (#165). Bảng bắt đầu là bước một; mỗi bảng bước một đã chọn mở thêm được một bước nữa, để
chọn nhóm hai bước như order_items -> orders -> customers. Không đi xa hơn hai bước: hỏi qua nhiều bảng (#213) chỉ đọc
bảng cách bảng hỏi tối đa hai bước.

Lựa chọn suy ra mỗi lần dựng từ lựa chọn tay của người dùng và mặc định (bảng mà bảng kia trỏ tới, nhiều-một, chụp kèm
được), không đặt lại trong effect. Bỏ chọn một bảng bước một thì các bảng chỉ thấy qua nó cũng ra khỏi nhóm: không chụp
bảng không nối với nhóm. */

import type { RelatedTable, RelationsResponse } from '@/api/sources'
import type { RelationNode, RelationNodeState, RelationOption } from '@/components/datasets/types'

/** Khớp MAX_SNAPSHOT_TABLES của backend (service/sources/snapshot.rs), tính cả bảng bắt đầu. */
export const MAX_SNAPSHOT_TABLES = 10

/** Lựa chọn của người dùng cho một bảng bắt đầu. Đổi bảng bắt đầu thì bắt đầu lại từ đầu. */
export type RelationChoice = {
  start: string
  /** Bảng người dùng tự đánh dấu (true) hay bỏ (false); bảng không có ở đây theo mặc định. */
  picked: Record<string, boolean>
  /** Bảng bước một đang mở thêm một bước. */
  expanded: string[]
}

/** Kết quả một lần đọc bảng liên quan, theo dạng của hook (useQueries). */
export type RelationsResult = {
  data?: RelationsResponse
  error: Error | null
}

export type RelatedView = {
  start: RelationNode
  /** Bảng liên quan của các bảng bước một đang mở, theo tên bảng. */
  nested: Record<string, RelationNodeState>
  /** Bảng bước một đang chọn: mở thêm được một bước. */
  expandable: string[]
  /** Bảng chụp kèm, không tính bảng bắt đầu; tối đa MAX_SNAPSHOT_TABLES - 1. */
  selected: string[]
}

/** "public.orders" hiện là "orders" khi bảng bắt đầu cũng ở schema public; khác schema thì giữ tên đầy đủ. */
export function displayName(table: string, start: string): string {
  const schema = (name: string) => name.slice(0, name.indexOf('.') + 1)
  const prefix = schema(start)
  return prefix !== '' && schema(table) === prefix ? table.slice(prefix.length) : table
}

/** Bỏ các bảng đã nằm ở bước trước (bảng bắt đầu, chính bảng đang mở) khỏi danh sách của một bảng. */
function toNode(response: RelationsResponse, start: string, exclude: string[]): RelationNode {
  const keep = (r: RelatedTable) => !exclude.includes(r.table)
  const option = (r: RelatedTable): RelationOption => ({
    key: `${r.from_table}:${r.constraint}`,
    table: r.table,
    label: displayName(r.table, start),
    columns: r.from_columns,
    addable: r.addable,
    reason: r.reason,
    columnCount: r.column_count,
  })
  return {
    table: response.table,
    label: displayName(response.table, start),
    parents: response.many_to_one.filter(keep).map(option),
    children: response.one_to_many.filter(keep).map(option),
  }
}

/** `start` là bảng liên quan của bảng bắt đầu; `results` theo thứ tự [bảng bắt đầu, ...choice.expanded], như các bảng
 * gửi cho useSourceRelations. */
export function buildRelatedView(start: RelationsResponse, results: RelationsResult[], choice: RelationChoice) {
  const startNode = toNode(start, choice.start, [choice.start])
  const selected: string[] = []
  // Đi theo thứ tự hiện ra: nhiều-một trước (mặc định chọn), một-nhiều sau (mặc định không). Đủ giới hạn thì dừng thêm,
  // kể cả bảng mặc định.
  const visit = (node: RelationNode) => {
    const groups: [RelationOption[], boolean][] = [
      [node.parents, true],
      [node.children, false],
    ]
    for (const [options, byDefault] of groups) {
      for (const option of options) {
        if (!option.addable || selected.includes(option.table)) continue
        const on = choice.picked[option.table] ?? byDefault
        if (on && selected.length + 1 < MAX_SNAPSHOT_TABLES) selected.push(option.table)
      }
    }
  }
  visit(startNode)
  const expandable = [...selected]

  const nested: Record<string, RelationNodeState> = {}
  choice.expanded.forEach((table, index) => {
    if (!expandable.includes(table)) return
    const result = results[index + 1]
    if (result?.data) {
      const node = toNode(result.data, choice.start, [choice.start, table])
      nested[table] = { kind: 'ready', node }
      visit(node)
    } else if (result?.error) {
      nested[table] = { kind: 'error', message: result.error.message }
    } else {
      nested[table] = { kind: 'loading' }
    }
  })
  return { start: startNode, nested, expandable, selected } satisfies RelatedView
}
