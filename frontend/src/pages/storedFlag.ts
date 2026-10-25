// SPDX-License-Identifier: Apache-2.0
/* pages/storedFlag.ts - một cờ bật, tắt của giao diện nhớ qua các lần tải trang bằng localStorage (ví dụ pane bằng
chứng đang mở hay thu gọn). Chỉ là tiện lợi: localStorage có thể bị chặn (chế độ riêng tư, chính sách trình duyệt) và
khi đó getItem, setItem ném lỗi; bắt lỗi và dùng giá trị trong bộ nhớ, trang vẫn chạy như thường. */

import { useState } from 'react'

export function useStoredFlag(key: string, fallback: boolean): [boolean, (value: boolean) => void] {
  const [value, setValue] = useState(() => readFlag(key, fallback))
  const update = (next: boolean) => {
    setValue(next)
    try {
      localStorage.setItem(key, next ? '1' : '0')
    } catch {
      // Không lưu được thì chỉ mất phần nhớ qua lần tải trang sau; giá trị trong trang vẫn đổi.
    }
  }
  return [value, update]
}

function readFlag(key: string, fallback: boolean): boolean {
  try {
    const stored = localStorage.getItem(key)
    return stored === null ? fallback : stored === '1'
  } catch {
    return fallback
  }
}
