// SPDX-License-Identifier: Apache-2.0
/* components/ask/breakdownGroups.ts - rút gọn bảng đóng góp của một chiều (BreakdownDimension.tsx): chỉ hiện các nhóm
đóng góp lớn nhất, các nhóm sau gộp thành một dòng "Còn lại" có tổng, để bảng của chiều nhiều nhóm (Customer State
khoảng 60 nhóm) không đẩy các chiều sau ra xa. Chỉ đổi cách hiện: response vẫn đủ mọi nhóm, drill và CSV không đổi.
Backend đã xếp nhóm theo trị tuyệt đối của đóng góp giảm dần, nên các nhóm đầu là các nhóm lớn nhất. */

import type { GroupView } from './evidenceTypes'

/** Số nhóm hiện khi bảng thu gọn: đủ thấy các nhóm làm nên chênh lệch, vẫn vừa một màn hình cùng các chiều khác. */
export const SHOWN_GROUPS = 8

/** Dòng gộp các nhóm không hiện; số là tổng của các nhóm đó, chuỗi chữ số như backend gửi. Với độ đo tỷ lệ, tỷ lệ của
 * các nhóm không cộng được nên baseValue, compareValue là null; đóng góp và hai phần của nó vẫn cộng được. */
export type RestGroups = {
  count: number
  baseValue: string | null
  compareValue: string | null
  contribution: string | null
  mixEffect: string | null
  rateEffect: string | null
}

/** Chia nhóm thành phần hiện và dòng "Còn lại". Chỉ gộp khi bớt được ít nhất hai nhóm: gộp một nhóm thành "Còn lại (1
 * nhóm)" không gọn hơn mà còn che tên nhóm đó. */
export function splitGroups(groups: GroupView[]): { shown: GroupView[]; rest: RestGroups | null } {
  if (groups.length <= SHOWN_GROUPS + 1) return { shown: groups, rest: null }
  const hidden = groups.slice(SHOWN_GROUPS)
  const ratio = hidden.some((group) => group.mixEffect !== null)
  return {
    shown: groups.slice(0, SHOWN_GROUPS),
    rest: {
      count: hidden.length,
      baseValue: ratio ? null : sumDecimals(hidden.map((group) => group.baseValue)),
      compareValue: ratio ? null : sumDecimals(hidden.map((group) => group.compareValue)),
      contribution: sumDecimals(hidden.map((group) => group.contribution)),
      mixEffect: ratio ? sumDecimals(hidden.map((group) => group.mixEffect ?? '')) : null,
      rateEffect: ratio ? sumDecimals(hidden.map((group) => group.rateEffect ?? '')) : null,
    },
  }
}

/** Cộng các chuỗi chữ số ("-1234.50") bằng BigInt trên số đã nhân theo số chữ số lẻ lớn nhất, không qua Number(): Number
 * làm tròn số lớn và số thập phân, tổng "Còn lại" cộng với các nhóm đã hiện sẽ không còn khớp chênh lệch. Một chuỗi
 * không đúng dạng thì trả null, dòng "Còn lại" để trống ô đó thay vì hiện tổng sai. */
export function sumDecimals(values: string[]): string | null {
  const parts = []
  for (const value of values) {
    const match = /^(-?)(\d+)(?:\.(\d+))?$/.exec(value)
    if (!match) return null
    parts.push({ negative: match[1] === '-', whole: match[2], fraction: match[3] ?? '' })
  }
  const scale = Math.max(0, ...parts.map((part) => part.fraction.length))
  let total = 0n
  for (const part of parts) {
    const units = BigInt(part.whole + part.fraction.padEnd(scale, '0'))
    total += part.negative ? -units : units
  }
  const negative = total < 0n
  const digits = (negative ? -total : total).toString().padStart(scale + 1, '0')
  const whole = digits.slice(0, digits.length - scale)
  const fraction = digits.slice(digits.length - scale)
  const sign = negative ? '-' : ''
  return scale === 0 ? `${sign}${whole}` : `${sign}${whole}.${fraction}`
}
