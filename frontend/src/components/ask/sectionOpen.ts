// SPDX-License-Identifier: Apache-2.0
/* components/ask/sectionOpen.ts - trạng thái mở hay thu gọn của một phần trong câu trả lời (SectionHeading.tsx). Chưa
bấm thì theo mặc định trang đưa vào, và mặc định đó có thể đổi: lượt cuối thành lượt cũ khi có câu hỏi mới thì thẻ "Cách
tính" của nó tự thu gọn. Đã bấm thì giữ đúng lựa chọn của người dùng. */

import { useState } from 'react'

export function useSectionOpen(defaultOpen: boolean): [boolean, () => void] {
  const [picked, setPicked] = useState<boolean | null>(null)
  const open = picked ?? defaultOpen
  return [open, () => setPicked(!open)]
}
