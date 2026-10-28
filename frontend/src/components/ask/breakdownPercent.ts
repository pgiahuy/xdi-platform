// SPDX-License-Identifier: Apache-2.0
/* components/ask/breakdownPercent.ts - phần trăm của bảng phân rã (BreakdownDimension.tsx), để người đọc thấy mỗi nhóm
tác động bao nhiêu tới chênh lệch chứ không chỉ con số tuyệt đối:
- % tác động: đóng góp của nhóm chia chênh lệch tổng. Mọi nhóm cộng lại ra 100%, vì đóng góp cộng lại bằng đúng chênh
  lệch: backend chỉ phân rã cộng thẳng độ đo cộng được (SUM, COUNT), còn độ đo tỷ lệ thì đóng góp là tổng hai phần cơ
  cấu và hiệu suất.
- % thay đổi: bên so sánh so với bên gốc trong nhóm. Đặt cạnh mức chung của cả lát cắt thì thấy nhóm nào khác thường
  (isOffTrend); nhóm có đóng góp lớn chỉ vì là nhóm lớn thì % thay đổi gần mức chung. Không tính cho độ đo tỷ lệ (số của
  hai bên đã là tỷ lệ) và khi bên gốc bằng 0 hay âm (phần trăm không có nghĩa).
Tính bằng Number(): chỉ để hiện một chữ số lẻ, nên làm tròn của số thực không đổi chữ hiện ra; số tuyệt đối vẫn là chuỗi
chữ số của backend (cellFormat.formatDecimal). */

/** Cách mức chung từ chừng này điểm phần trăm thì % thay đổi của nhóm được in đậm. Nhỏ hơn thì nhóm chỉ dao động thường
 * cũng bị đánh dấu; lớn hơn thì bỏ sót nhóm khác hẳn. Ví dụ DataCo, Corporate so với Consumer: mức chung -41,5%, các
 * hình thức thanh toán từ -42,4% tới -38,2% (không nhóm nào khác thường), các danh mục từ -67,4% tới -17,1%. */
export const OFF_TREND_POINTS = 10

/** Nhóm phải tạo ra ít nhất chừng này phần trăm của chênh lệch mới được in đậm. Nhóm nhỏ ít dòng nên tỷ lệ dao động
 * mạnh: cùng ví dụ trên, mọi danh mục lệch xa mức chung chỉ tác động 0,1% tới 0,3%, đánh dấu chúng là đưa người đọc đi
 * lạc; với ngưỡng này còn lại các bang như FL (tác động 4,2%, thay đổi -53,9%). */
export const MIN_SHARE_PERCENT = 1

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

/** Nhóm khác thường: % thay đổi lệch mức chung từ OFF_TREND_POINTS, và % tác động từ MIN_SHARE_PERCENT trở lên. */
export function isOffTrend(change: number | null, overall: number | null, share: number | null): boolean {
  if (change === null || overall === null || share === null) return false
  return Math.abs(change - overall) >= OFF_TREND_POINTS && Math.abs(share) >= MIN_SHARE_PERCENT
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
