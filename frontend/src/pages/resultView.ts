// SPDX-License-Identifier: Apache-2.0
/* pages/resultView.ts - đổi ResultTable của hợp đồng sang ResultView của components/ask. Bảng kết quả (AskOutcome.tsx)
và bảng dòng nguồn (AskEvidence.tsx) cùng dạng nên dùng chung; để ở file .ts riêng vì file .tsx chỉ xuất component
(luật only-export-components của lint, cho Vite nạp lại nóng đúng). */

import type { ResultTable } from '@/api/ask'
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
