// SPDX-License-Identifier: Apache-2.0
/* pages/resultView.ts - đổi ResultTable của hợp đồng sang ResultView của components/ask. Bảng kết quả (AskOutcome.tsx)
và bảng dòng nguồn (AskEvidence.tsx) cùng dạng nên dùng chung; để ở file .ts riêng vì file .tsx chỉ xuất component
(luật only-export-components của lint, cho Vite nạp lại nóng đúng). Ô nào bấm được để xem dòng nguồn cũng dựng ở đây,
vì bảng kết quả (AskOutcome) và tab Dòng nguồn (AskEvidence, để biết có gì để bấm) cùng cần. */

import type { AskResponse, ResultTable } from '@/api/ask'
import type { CellDrill, DrillTarget } from '@/components/ask/evidenceTypes'
import type { ResultView } from '@/components/ask/types'

export function buildResultView(result: ResultTable): ResultView {
  return {
    columns: result.columns.map((column) => ({ name: column.name, kind: column.kind })),
    rows: result.rows,
    totalRows: result.total_rows,
    truncated: result.truncated,
    limitReached: result.limit_reached,
  }
}

/** Ô nào của bảng kết quả bấm được để xem dòng nguồn (#34): ô của cột role measure, khóa là các cột dimension và time
 * của dòng. null khi không truy được: câu chưa lưu (không có query_id) hay không có cột số đo (mọi cột other, ví dụ
 * kết quả cũ trước khi có role, thì backend không biết cột nào là khóa). */
export function buildCellIndexes(response: AskResponse): { measures: number[]; keys: number[] } | null {
  if (!response.query_id || !response.result) return null
  const indexesOf = (roles: string[]) =>
    response.result?.columns.flatMap((column, index) => (roles.includes(column.role) ? [index] : [])) ?? []
  const measures = indexesOf(['measure'])
  if (measures.length === 0) return null
  return { measures, keys: indexesOf(['dimension', 'time']) }
}

export function buildCellDrill(response: AskResponse, onDrill: (target: DrillTarget) => void): CellDrill | undefined {
  const indexes = buildCellIndexes(response)
  return indexes ? { ...indexes, onDrill } : undefined
}
