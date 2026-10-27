// SPDX-License-Identifier: Apache-2.0
/* pages/savedTurnView.ts - dựng các dòng "Đã hiểu là" của một lượt đã lưu từ TurnItem.understood. Backend viết chuỗi
này cho prompt của mô hình (build_understood_text trong backend/src/service/conversation/history.rs): các phần
"measures: ...; filters: ...; grouped by: ...; period: ..." nối bằng "; ", tên phần tiếng Anh. Hiện nguyên chuỗi thì
người dùng đọc tên phần tiếng Anh, nên tách theo đúng bốn tên phần đó và thay bằng nhãn của thẻ "Đã hiểu là". Chuỗi
không mở đầu bằng một tên phần đã biết (backend đổi cách viết) thì trả một dòng không nhãn với nguyên chuỗi, không bỏ
mất nội dung. */

const partLabels: Record<string, string> = {
  measures: 'Đo',
  filters: 'Lọc',
  'grouped by': 'Nhóm theo',
  period: 'Khoảng thời gian',
}

const partStart = /(?:^|; )(measures|filters|grouped by|period): /g

export type UnderstoodLine = { label: string | null; text: string }

export function buildUnderstoodLines(understood: string | null): UnderstoodLine[] {
  if (!understood) return []
  const starts = [...understood.matchAll(partStart)]
  if (starts.length === 0 || starts[0].index !== 0) return [{ label: null, text: understood }]
  return starts.map((match, i) => {
    const from = match.index + match[0].length
    const to = i + 1 < starts.length ? starts[i + 1].index : understood.length
    return { label: partLabels[match[1]], text: understood.slice(from, to) }
  })
}
