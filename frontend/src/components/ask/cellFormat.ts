// SPDX-License-Identifier: Apache-2.0
/* components/ask/cellFormat.ts - chữ hiển thị của một ô kết quả theo kind của cột, kiểu Việt Nam: nhóm nghìn bằng dấu
chấm, phần thập phân sau dấu phẩy, ngày dd/mm/yyyy. Chỉ đổi cách hiện; giá trị gốc vẫn nằm nguyên trong dữ liệu, tải
CSV dùng giá trị gốc (resultCsv.ts).
Ô decimal là chuỗi chữ số để giữ đúng giá trị (BIGINT vượt 2^53, DECIMAL); đổi sang Number để định dạng sẽ làm tròn,
nên nhóm nghìn thẳng trên chuỗi. Ngày giờ cũng tách trên chuỗi, không qua Date: Date đọc giờ không múi giờ theo múi giờ
của máy, hiện sai giờ đã lưu. */

import type { Cell, CellKind } from './types'

// Ô number có phần lẻ (SUM, AVG trên cột số thực) thường ra hàng chục chữ số lẻ như 6929653,690574; hai chữ số lẻ là đủ
// để đọc và so, luôn đủ hai chữ số để cột thẳng hàng (3.667.633,20, không phải 3.667.633,2); giá trị đủ chữ số vẫn
// ở CSV. Số nguyên giữ nguyên mọi chữ số.
const integerFormat = new Intl.NumberFormat('vi-VN', { maximumFractionDigits: 0 })
const fractionFormat = new Intl.NumberFormat('vi-VN', { minimumFractionDigits: 2, maximumFractionDigits: 2 })
// Số có trị tuyệt đối dưới 1 (tỉ lệ, xác suất) định dạng theo hai chữ số có nghĩa: theo hai chữ số lẻ thì 0,004 hiện
// thành 0, ô khác 0 lại hiện như 0. Với số từ 0,01 trở lên kết quả giống hai chữ số lẻ (0,1234 thành 0,12).
const smallNumberFormat = new Intl.NumberFormat('vi-VN', { maximumSignificantDigits: 2 })

export function formatCell(value: Cell, kind: CellKind): string {
  if (value === null) return ''
  switch (kind) {
    case 'number':
      return typeof value === 'number' ? formatNumber(value) : String(value)
    case 'decimal':
      return formatDecimal(String(value))
    case 'boolean':
      return value ? 'Đúng' : 'Sai'
    case 'date':
      return formatDate(String(value))
    case 'timestamp':
      return formatTimestamp(String(value))
    default:
      return String(value)
  }
}

/** Giá trị số để vẽ biểu đồ: decimal đổi bằng Number (chỉ để vẽ, chấp nhận làm tròn); ô không phải số thì null. */
export function toChartNumber(value: Cell): number | null {
  if (typeof value === 'number') return value
  if (typeof value === 'string' && value.trim() !== '') {
    const number = Number(value)
    return Number.isFinite(number) ? number : null
  }
  return null
}

function formatNumber(value: number): string {
  // value === 0 đúng cả với -0; Intl hiện -0 thành "-0".
  if (value === 0) return '0'
  if (Number.isInteger(value)) return integerFormat.format(value)
  return Math.abs(value) < 1 ? smallNumberFormat.format(value) : fractionFormat.format(value)
}

/** Như formatDecimal; null (ô không tính được, ví dụ tỷ lệ của dòng "Còn lại") thành chuỗi rỗng. */
export function formatOptionalDecimal(value: string | null): string {
  return value === null ? '' : formatDecimal(value)
}

/** "-1234567.50" thành "-1.234.567,50"; chuỗi không đúng dạng chữ số thì giữ nguyên. */
export function formatDecimal(value: string): string {
  const match = /^(-?)(\d+)(?:\.(\d+))?$/.exec(value)
  if (!match) return value
  const [, sign, whole, fraction] = match
  const grouped = whole.replace(/\B(?=(\d{3})+$)/g, '.')
  return fraction === undefined ? `${sign}${grouped}` : `${sign}${grouped},${fraction}`
}

/** Kind dùng để hiện một cột: cột timestamp mà mọi ô đều rơi vào nửa đêm (date_trunc theo ngày, tháng, quý) hiện như
 * cột ngày, vì " 00:00:00" lặp ở mọi dòng chỉ làm rối. Cột có ô mang giờ thật thì giữ đủ giờ ở mọi ô cho thẳng hàng. */
export function buildDisplayKind(kind: CellKind, values: Cell[]): CellKind {
  if (kind !== 'timestamp') return kind
  const midnight = values.every((value) => value === null || /T00:00:00$/.test(String(value)))
  return midnight ? 'date' : kind
}

function formatDate(value: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})(?:T00:00:00)?$/.exec(value)
  return match ? `${match[3]}/${match[2]}/${match[1]}` : value
}

// TIMESTAMPTZ có Z (backend đổi sang UTC): ghi rõ UTC thay vì đổi sang giờ máy, để giống số trong SQL.
function formatTimestamp(value: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}:\d{2}:\d{2}(?:\.\d+)?)(Z?)$/.exec(value)
  if (!match) return value
  const [, year, month, day, time, utc] = match
  return `${day}/${month}/${year} ${time}${utc ? ' UTC' : ''}`
}
