// SPDX-License-Identifier: Apache-2.0
/* components/datasets/sheetGroups.ts - gom các sheet đã chọn theo bộ tên cột.
Backend gộp các sheet gửi cùng một lần thành một dataset và từ chối nếu chúng khác cột (so tên cột, không xét thứ tự,
phân biệt chữ hoa, chữ thường; engine/analytics.rs). Vì vậy giao diện gửi một lần nạp cho mỗi nhóm, mỗi nhóm thành
một dataset. So khác cách backend so thì một nhóm gửi lên sẽ bị từ chối, hoặc hai nhóm lẽ ra gộp được bị tách. */

import type { SheetOption } from './types'

export type SheetGroup = {
  columns: string[]
  /** Tên sheet theo thứ tự trong file. */
  sheets: string[]
}

export function groupSheets(sheets: SheetOption[], selected: Set<string>): SheetGroup[] {
  const groups = new Map<string, SheetGroup>()
  for (const sheet of sheets) {
    if (!selected.has(sheet.name) || sheet.columns.length === 0) continue
    const key = columnsKey(sheet.columns)
    const group = groups.get(key)
    if (group) group.sheets.push(sheet.name)
    else groups.set(key, { columns: sheet.columns, sheets: [sheet.name] })
  }
  return [...groups.values()]
}

/** Sheet chọn sẵn: các sheet đang hiện cùng cột với sheet có dữ liệu đầu tiên. File chỉ một bảng (một sheet, hoặc
mỗi năm một sheet cùng cột) thì không phải chọn gì; sheet ghi chú hay sheet khác cột thì người dùng tự chọn thêm. */
export function defaultSelection(sheets: SheetOption[]): Set<string> {
  const visible = sheets.filter((s) => !s.hidden && s.columns.length > 0)
  if (visible.length === 0) return new Set()
  const key = columnsKey(visible[0].columns)
  return new Set(visible.filter((s) => columnsKey(s.columns) === key).map((s) => s.name))
}

function columnsKey(columns: string[]): string {
  // Ký tự NUL không có trong tên cột đọc từ Excel, nên ghép bằng nó không làm hai bộ cột khác nhau trùng khóa.
  return [...columns].sort().join('\u0000')
}
