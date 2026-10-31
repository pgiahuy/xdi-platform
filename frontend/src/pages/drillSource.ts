// SPDX-License-Identifier: Apache-2.0
/* pages/drillSource.ts - phần "dòng đến từ đâu, cộng ra bao nhiêu" của tab Dòng nguồn (AskEvidence.tsx dựng,
components/ask/EvidenceRows.tsx hiện): bảng của các dòng và cột lấy từ bảng nối (DrillResponse.source_table,
joined_columns), cùng độ đo của chỗ được bấm tính lại trên mọi dòng nguồn (DrillResponse.measures, hợp đồng ở
backend/src/service/drill/contract.rs). Ô của bảng kết quả thì so được với giá trị tính lại; dòng phân rã thì không
so, vì số của phân rã có thể là trung bình mỗi nhóm còn lại hay tỷ lệ chứ không phải tổng các dòng. */

import type { DrillResponse } from '@/api/drill'
import type { DrillTarget, RowsMeasure, RowsSource } from '@/components/ask/evidenceTypes'
import type { CellKind } from '@/components/ask/types'

export function buildRowsSource(target: DrillTarget, data: DrillResponse): RowsSource {
  return {
    table: data.source_table ?? null,
    joinedColumns: data.joined_columns,
    measure: buildRowsMeasure(target, data),
  }
}

function buildRowsMeasure(target: DrillTarget, data: DrillResponse): RowsMeasure | null {
  // Ô thì độ đo cùng tên cột với ô được bấm; dòng phân rã thì backend chỉ gửi độ đo của lát cắt được bấm.
  const measure =
    target.from === 'cell' ? data.measures.find((m) => m.column === target.measure) : data.measures[0]
  if (!measure) return null
  const rowKind = data.result.columns.find((column) => column.name === measure.row_column)?.kind
  const kind: CellKind = target.from === 'cell' ? target.kind : (rowKind ?? 'decimal')
  return {
    column: measure.column,
    rule: measure.rule,
    value: measure.value,
    kind,
    rowColumn: measure.row_column ?? null,
    clicked: target.from === 'cell' ? target.value : null,
    compared: target.from === 'cell',
  }
}
