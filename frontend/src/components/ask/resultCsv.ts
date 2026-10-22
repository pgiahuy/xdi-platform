// SPDX-License-Identifier: Apache-2.0
/* components/ask/resultCsv.ts - tải CSV các dòng kết quả đã nhận, làm ngay trong trình duyệt, không gọi backend. Tải đủ
mọi dòng khi kết quả bị cắt cần endpoint riêng theo query_id (ARCHITECTURE mục 8), làm sau; giao diện ghi rõ khi tệp
chỉ có một phần.
Ô ghi giá trị gốc, không theo chữ hiển thị: decimal giữ nguyên chuỗi chữ số, số dùng dấu chấm thập phân, ngày giờ ISO
8601, để mở bằng công cụ khác vẫn đọc đúng và không mất chữ số. */

import type { Cell, CellKind, ResultView } from './types'

/** Nội dung CSV theo RFC 4180: dòng tiêu đề là tên cột, xuống dòng CRLF, ô có dấu phẩy, ngoặc kép, xuống dòng thì
 * bọc ngoặc kép. */
export function buildResultCsv(result: ResultView): string {
  const header = result.columns.map((column) => quote(protectFormula(column.name)))
  const lines = result.rows.map((row) => row.map((cell, i) => quote(csvValue(cell, result.columns[i].kind))))
  return [header, ...lines].map((cells) => cells.join(',')).join('\r\n') + '\r\n'
}

function csvValue(cell: Cell, kind: CellKind): string {
  if (cell === null) return ''
  const text = String(cell)
  // Chỉ ô chữ mới cần chặn công thức; ô số như "-5" phải giữ nguyên để còn là số.
  return kind === 'text' || kind === 'other' ? protectFormula(text) : text
}

// Excel và LibreOffice chạy ô bắt đầu bằng = + - @ như công thức. Dataset do thành viên khác nạp có thể chứa ô như
// vậy, mở CSV là công thức chạy trên máy người tải. Thêm dấu nháy đơn phía trước để bảng tính hiện nguyên chữ.
function protectFormula(text: string): string {
  return /^[=+\-@\t\r]/.test(text) ? `'${text}` : text
}

function quote(text: string): string {
  return /[",\r\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text
}

/** Lưu chuỗi CSV thành file. Thêm BOM UTF-8 ở đầu: thiếu nó Excel đọc file theo bảng mã cũ, chữ tiếng Việt bị vỡ.
 * Viết BOM bằng escape \uFEFF, không gõ thẳng ký tự: ký tự này vô hình, người đọc tưởng chuỗi rỗng rồi xóa.
 * Dấu ngăn giữ dấu phẩy theo chuẩn CSV, không thêm dòng sep=: có dòng đó Excel bỏ qua BOM. */
export function downloadCsv(content: string, fileName: string): void {
  const blob = new Blob(['\uFEFF', content], { type: 'text/csv;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = fileName
  link.click()
  // Thu hồi sau vài giây: thu hồi ngay khi trình duyệt vừa bắt đầu tải thì có trình duyệt (Safari) hủy lượt tải.
  setTimeout(() => URL.revokeObjectURL(url), 10_000)
}
