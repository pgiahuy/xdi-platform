// SPDX-License-Identifier: Apache-2.0
/* pages/drillColumns.ts - thứ tự cột của bảng dòng nguồn trong tab Dòng nguồn (AskEvidence.tsx). Dòng nguồn có mọi cột
của dataset (DataCo hơn 50 cột) mà pane bằng chứng hẹp, nên chỉ thấy vài cột đầu; đưa lên đầu các cột giải thích được
con số vừa bấm: cột khóa của chỗ được bấm (cột nhóm của ô, cột của chiều phân rã) rồi cột nguồn của số đo, các cột
khác giữ thứ tự backend gửi. Chỉ đổi thứ tự hiển thị, request và response drill giữ nguyên.
Cột nguồn của số đo đọc từ biểu thức (understanding.measures[].sql như SUM("Sales"), hoặc tên cột kết quả như
sum(Sales)); chỉ nhận tên trùng đúng một cột của dòng nguồn, nên đọc sai thì không đưa cột nào lên, không đưa nhầm. */

import type { AskResponse } from '@/api/ask'
import type { DrillTarget } from '@/components/ask/evidenceTypes'
import type { ResultView } from '@/components/ask/types'

/** Tên các cột nên đứng đầu bảng dòng nguồn, theo thứ tự; có thể có tên không phải cột của dòng nguồn. */
export function buildLeadNames(target: DrillTarget, response: AskResponse): string[] {
  if (target.from === 'breakdown') return [target.column]
  const understanding = response.understanding
  const groupBy = understanding?.group_by.flatMap((item) => readNames(item.sql)) ?? []
  return [...target.keys.map((key) => key.column), ...groupBy, ...readMeasureNames(target.measure, response)]
}

// Cột số đo thứ i của kết quả ứng với measures[i] của thẻ "Cách tính" khi hai bên cùng số lượng (cùng đọc từ danh
// sách SELECT); lệch số lượng thì chỉ đọc tên cột kết quả.
function readMeasureNames(measure: string, response: AskResponse): string[] {
  const measureColumns = response.result?.columns.filter((column) => column.role === 'measure') ?? []
  const measures = response.understanding?.measures ?? []
  const index = measureColumns.findIndex((column) => column.name === measure)
  const sql = measures.length === measureColumns.length && index >= 0 ? readNames(measures[index].sql) : []
  return [...sql, ...readNames(measure)]
}

/** Tên có thể là cột trong một đoạn SQL: định danh trong ngoặc kép, và chữ trần trong ngoặc tròn như sum(Sales). */
function readNames(sql: string): string[] {
  const quoted = [...sql.matchAll(/"((?:[^"]|"")+)"/g)].map((m) => m[1].replaceAll('""', '"'))
  const bare = [...sql.matchAll(/\(\s*([^()"]+?)\s*\)/g)].map((m) => m[1])
  return [...quoted, sql, ...bare]
}

/** Bảng dòng nguồn với các cột trong leadNames lên đầu (so đúng tên, rồi không phân biệt hoa thường). */
export function buildLeadFirst(result: ResultView, leadNames: string[]): ResultView {
  const lead: number[] = []
  for (const name of leadNames) {
    const exact = result.columns.findIndex((column) => column.name === name)
    const index =
      exact >= 0 ? exact : result.columns.findIndex((column) => column.name.toLowerCase() === name.toLowerCase())
    if (index >= 0 && !lead.includes(index)) lead.push(index)
  }
  if (lead.length === 0) return result
  const order = [...lead, ...result.columns.map((_, index) => index).filter((index) => !lead.includes(index))]
  return {
    ...result,
    columns: order.map((index) => result.columns[index]),
    rows: result.rows.map((row) => order.map((index) => row[index])),
  }
}
