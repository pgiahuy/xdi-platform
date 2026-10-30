// SPDX-License-Identifier: Apache-2.0
/* components/overview/units.ts - đơn vị cột số đo trong khung tổng quan: chữ tiếng Việt cho người xem
(ColumnGroups.tsx, DataOverview.tsx) và cột nào cộng được để câu hỏi điền sẵn (questions.ts) nói "Tổng" hay "Trung bình".
Đơn vị do lớp ngữ nghĩa ghi (ColumnUnit ở backend/src/engine/store/columns.rs); null khi chưa có. */

const unitLabels: Record<string, string> = {
  money: 'tiền',
  quantity: 'số lượng',
  percent: 'phần trăm',
  duration: 'thời lượng',
}

/** "đơn vị tiền", "đơn vị khác"; đơn vị lạ thì ghi nguyên. */
export function buildUnitText(unit: string): string {
  if (unit === 'other') return 'đơn vị khác'
  return `đơn vị ${unitLabels[unit] ?? unit}`
}

/** Chỉ tiền và số lượng cộng qua các dòng ra số có nghĩa, như cách backend chọn độ đo chính (SUM của cột tiền hay số
 * lượng, service/overview/sql.rs). Cộng phần trăm, thời lượng hay cột chưa rõ đơn vị dễ ra số vô nghĩa, nên hỏi trung
 * bình. */
export function isAdditiveUnit(unit: string | null): boolean {
  return unit === 'money' || unit === 'quantity'
}
