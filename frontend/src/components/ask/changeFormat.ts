// SPDX-License-Identifier: Apache-2.0
/* components/ask/changeFormat.ts - đọc chiều và viết số có dấu cho một chênh lệch là chuỗi chữ số của backend
("-1234.50"). Đọc trên chuỗi, không qua Number(), để số lớn không bị làm tròn (cùng lý do với cellFormat.formatDecimal). */

import { formatDecimal } from './cellFormat'

export type ChangeDirection = 'up' | 'down' | 'flat'

export function changeDirection(value: string): ChangeDirection {
  if (!/[1-9]/.test(value)) return 'flat'
  return value.startsWith('-') ? 'down' : 'up'
}

/** Số có dấu: "+5.107,10" khi tăng, "-1.152,24" khi giảm, không dấu khi bằng 0. */
export function formatSigned(value: string): string {
  return changeDirection(value) === 'up' ? `+${formatDecimal(value)}` : formatDecimal(value)
}
