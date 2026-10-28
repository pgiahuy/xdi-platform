// SPDX-License-Identifier: Apache-2.0
/* components/ask/changeFormat.ts - đọc chiều và viết số có dấu cho một chênh lệch là chuỗi chữ số của backend
("-1234.50") hay phần trăm ("-13.6"). Đọc trên chuỗi, không qua Number(), để số lớn không bị làm tròn (cùng lý do với cellFormat.formatDecimal). */

import { formatDecimal } from './cellFormat'
import type { Comparison } from './evidenceTypes'

export type ChangeDirection = 'up' | 'down' | 'flat'

export function changeDirection(value: string): ChangeDirection {
  if (!/[1-9]/.test(value)) return 'flat'
  return value.startsWith('-') ? 'down' : 'up'
}

/** Chữ chiều: giữa hai kỳ là tăng, giảm; giữa hai nhóm (không có trước sau) là cao hơn, thấp hơn, bằng. */
export function directionWord(direction: ChangeDirection, comparison: Comparison): string {
  if (comparison === 'groups') return direction === 'down' ? 'thấp hơn' : direction === 'up' ? 'cao hơn' : 'bằng'
  return direction === 'down' ? 'giảm' : direction === 'up' ? 'tăng' : 'không đổi'
}

/** Số có dấu: "+5.107,10" khi tăng, "-1.152,24" khi giảm, không dấu khi bằng 0. */
export function formatSigned(value: string): string {
  return changeDirection(value) === 'up' ? `+${formatDecimal(value)}` : formatDecimal(value)
}

/** Phần trăm của backend ("13.6", một chữ số lẻ) thành "13,6%". */
export function formatPercent(value: string): string {
  return `${formatDecimal(value)}%`
}
