// SPDX-License-Identifier: Apache-2.0
/* components/ask/breakdownPercent.ts - phần trăm của bảng phân rã (BreakdownDimension.tsx). Với từng nhóm, % tác động,
% thay đổi và cờ khác thường do backend tính (explain/annotate.rs, cùng luật với lời tóm tắt); file này chỉ đọc và viết
chúng ra. Dòng "Còn lại" gộp nhiều nhóm nên backend không có số cho nó: hai hàm tính ở đây dùng cho dòng đó và cho mức
chung ở dòng "Tổng":
- % tác động: đóng góp chia chênh lệch tổng. Mọi nhóm cộng lại ra 100%, vì đóng góp cộng lại bằng đúng chênh lệch.
- % thay đổi: bên so sánh so với bên gốc. Không tính cho độ đo tỷ lệ (số của hai bên đã là tỷ lệ) và khi bên gốc bằng 0
  hay âm (phần trăm không có nghĩa).
Tính bằng Number(): chỉ để hiện một chữ số lẻ, nên làm tròn của số thực không đổi chữ hiện ra; số tuyệt đối vẫn là chuỗi
chữ số của backend (cellFormat.formatDecimal). */

const percentFormat = new Intl.NumberFormat('vi-VN', { maximumFractionDigits: 1 })

/** Đóng góp của nhóm chia chênh lệch tổng, theo phần trăm; null khi không tính được (chênh lệch bằng 0). */
export function sharePercent(contribution: string | null, change: string): number | null {
  const total = Number(change)
  if (contribution === null || !Number.isFinite(total) || total === 0) return null
  const part = Number(contribution)
  return Number.isFinite(part) ? (part / total) * 100 : null
}

/** Bên so sánh so với bên gốc, theo phần trăm; null khi bên gốc không dương hay một bên trống. */
export function changePercent(base: string | null, compare: string | null): number | null {
  if (base === null || compare === null) return null
  const from = Number(base)
  const to = Number(compare)
  if (!Number.isFinite(from) || !Number.isFinite(to) || from <= 0) return null
  return ((to - from) / from) * 100
}

/** Phần trăm backend gửi ("-41.5") thành số để hiện; null giữ null. Chỉ để hiện một chữ số lẻ như backend đã làm tròn. */
export function parsePercent(text: string | null): number | null {
  if (text === null) return null
  const value = Number(text)
  return Number.isFinite(value) ? value : null
}

/** "39,1%"; chuỗi rỗng khi không tính được. */
export function formatShare(value: number | null): string {
  return value === null ? '' : `${percentFormat.format(roundToShown(value))}%`
}

/** "+5,2%", "-42,2%", "0%": luôn có dấu khi khác 0, vì đây là chiều thay đổi. */
export function formatChangePercent(value: number | null): string {
  if (value === null) return ''
  const shown = roundToShown(value)
  return `${shown > 0 ? '+' : ''}${percentFormat.format(shown)}%`
}

/** Chiều của một phần trăm như số hiện ra: -0,04 hiện là 0% nên không tô màu giảm. */
export function percentDirection(value: number | null): 'up' | 'down' | 'flat' {
  const shown = value === null ? 0 : roundToShown(value)
  return shown > 0 ? 'up' : shown < 0 ? 'down' : 'flat'
}

// Làm tròn tới một chữ số lẻ như percentFormat, và đổi -0 thành 0 để không hiện "-0%".
function roundToShown(value: number): number {
  const rounded = Math.round(value * 10) / 10
  return rounded === 0 ? 0 : rounded
}
