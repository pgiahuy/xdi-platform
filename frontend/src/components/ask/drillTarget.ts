// SPDX-License-Identifier: Apache-2.0
/* components/ask/drillTarget.ts - chữ mô tả một chỗ được bấm để xem dòng nguồn (DrillTarget), ví dụ "Market = Europe,
revenue". Nhãn của nút trong ô bảng kết quả (ResultTable.tsx) và dòng "Đang xem" của tab Dòng nguồn (EvidenceRows.tsx)
dùng chung hàm này, để người dùng nghe và đọc cùng một câu ở hai chỗ. Giá trị hiện theo cellFormat, giá trị gửi drill
vẫn là giá trị gốc trong DrillTarget. */

import { formatCell } from './cellFormat'
import type { DrillTarget } from './evidenceTypes'
import type { Cell, CellKind } from './types'

/** sliceLabels: nhãn hai lát cắt của câu "tại sao", chỉ cần với dòng phân rã. */
export function describeDrillTarget(target: DrillTarget, sliceLabels = { base: '', compare: '' }): string {
  if (target.from === 'breakdown') {
    return `${target.column} = ${valueText(target.value, target.kind)}, ${sliceLabels[target.slice]}`
  }
  // Câu không nhóm (một dòng, không có khóa) thì ô là tổng của mọi dòng nguồn khớp bộ lọc của câu hỏi.
  if (target.keys.length === 0) return `${target.measure}, mọi dòng của câu hỏi`
  const keys = target.keys.map((key) => `${key.column} = ${valueText(key.value, key.kind)}`)
  return `${keys.join(', ')}, ${target.measure}`
}

function valueText(value: Cell, kind: CellKind): string {
  return value === null ? 'trống' : formatCell(value, kind)
}
