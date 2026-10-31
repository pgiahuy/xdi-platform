// SPDX-License-Identifier: Apache-2.0
/* components/datasets/formatBytes.ts - dung lượng bảng theo Postgres báo (byte) thành chữ ngắn, cho màn kết nối
(SourceTablePicker.tsx) và bảng chỉ có cấu trúc trên thẻ database (DatasetTables.tsx). Dùng đơn vị 1024 như
pg_size_pretty, để số khớp với số admin thấy trong psql. */

const UNITS = ['byte', 'KB', 'MB', 'GB', 'TB']
const oneDecimal = new Intl.NumberFormat('vi-VN', { maximumFractionDigits: 1 })

export function formatBytes(bytes: number): string {
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024
    unit += 1
  }
  return `${oneDecimal.format(value)} ${UNITS[unit]}`
}
