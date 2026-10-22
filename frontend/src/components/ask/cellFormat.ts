// SPDX-License-Identifier: Apache-2.0
/* components/ask/cellFormat.ts - chữ hiển thị của một ô kết quả theo kind của cột, kiểu Việt Nam: nhóm nghìn bằng dấu
chấm, phần thập phân sau dấu phẩy, ngày dd/mm/yyyy. Chỉ đổi cách hiện; giá trị gốc vẫn nằm nguyên trong dữ liệu, tải
CSV dùng giá trị gốc (resultCsv.ts).
Ô decimal là chuỗi chữ số để giữ đúng giá trị (BIGINT vượt 2^53, DECIMAL); đổi sang Number để định dạng sẽ làm tròn,
nên nhóm nghìn thẳng trên chuỗi. Ngày giờ cũng tách trên chuỗi, không qua Date: Date đọc giờ không múi giờ theo múi giờ
của máy, hiện sai giờ đã lưu. */

import type { Cell, CellKind } from './types'

const numberFormat = new Intl.NumberFormat('vi-VN', { maximumFractionDigits: 6 })
// Số có trị tuyệt đối dưới 1 (tỉ lệ, xác suất) định dạng theo chữ số có nghĩa: theo số chữ số thập phân thì 0,0000001
// hiện thành 0, ô khác 0 lại hiện như 0.
const smallNumberFormat = new Intl.NumberFormat('vi-VN', { maximumSignificantDigits: 6 })

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
  return Math.abs(value) < 1 ? smallNumberFormat.format(value) : numberFormat.format(value)
}

/** "-1234567.50" thành "-1.234.567,50"; chuỗi không đúng dạng chữ số thì giữ nguyên. */
export function formatDecimal(value: string): string {
  const match = /^(-?)(\d+)(?:\.(\d+))?$/.exec(value)
  if (!match) return value
  const [, sign, whole, fraction] = match
  const grouped = whole.replace(/\B(?=(\d{3})+$)/g, '.')
  return fraction === undefined ? `${sign}${grouped}` : `${sign}${grouped},${fraction}`
}

function formatDate(value: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value)
  return match ? `${match[3]}/${match[2]}/${match[1]}` : value
}

// TIMESTAMPTZ có Z (backend đổi sang UTC): ghi rõ UTC thay vì đổi sang giờ máy, để giống số trong SQL.
function formatTimestamp(value: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}:\d{2}:\d{2}(?:\.\d+)?)(Z?)$/.exec(value)
  if (!match) return value
  const [, year, month, day, time, utc] = match
  return `${day}/${month}/${year} ${time}${utc ? ' UTC' : ''}`
}
