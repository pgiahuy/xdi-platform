// SPDX-License-Identifier: Apache-2.0
/* components/ask/breakdownText.ts - câu chữ của tab Phân rã viết theo phép so (comparison): lời nhắc bấm số để xem dòng
nguồn và ghi chú cách chia đã bỏ. Dùng chung cho EvidenceBreakdown.tsx và lời nhắc ở tab Dòng nguồn (pages/AskEvidence.tsx),
để hai chỗ gọi cùng một cách: so hai kỳ thì "số của một kỳ", so hai nhóm thì nêu tên hai nhóm, không gọi chúng là "kỳ". */

import { columnText } from './columnText'
import type { Comparison, DroppedView } from './evidenceTypes'

/** Số nào bấm được: "con số của một kỳ" hay "con số của Consumer hay Corporate". */
export function drillSubject(comparison: Comparison, baseLabel: string, compareLabel: string): string {
  return comparison === 'periods' ? 'con số của một kỳ' : `con số của ${baseLabel} hay ${compareLabel}`
}

/** Số giá trị nêu tên trong một ghi chú; nhiều hơn thì câu dài mà không thêm ý. */
const SHOWN_VALUES = 3

/** Một câu cho một cách chia đã bỏ, ví dụ "Type (Loại): COMPLETE chỉ có DEBIT, CANCELED chỉ có TRANSFER, nên cột này chỉ
 * là tên khác của hai nhóm đang so." */
export function droppedText(
  dropped: DroppedView,
  comparison: Comparison,
  baseLabel: string,
  compareLabel: string,
): string {
  const name = columnText(dropped.column, dropped.label)
  if (dropped.reason === 'duplicates') {
    return `${name}: chia nhóm giống hệt ${dropped.duplicateOf ?? 'một cột khác'}.`
  }
  if (dropped.reason === 'follows_slices') {
    const compared = dropped.duplicateOf ?? 'cột đang so'
    return `${name}: mỗi giá trị của ${compared} chỉ có một giá trị của cột này, nên cột này chỉ chia lại các nhóm đang so.`
  }
  const sides = comparison === 'periods' ? 'hai kỳ' : 'hai nhóm'
  return (
    `${name}: ${baseLabel} chỉ có ${listValues(dropped.baseOnly)}, ${compareLabel} chỉ có ` +
    `${listValues(dropped.compareOnly)}, nên cột này chỉ là tên khác của ${sides} đang so.`
  )
}

function listValues(values: string[]): string {
  const shown = values.slice(0, SHOWN_VALUES).join(', ')
  const rest = values.length - SHOWN_VALUES
  return rest > 0 ? `${shown} và ${rest} giá trị khác` : shown
}
