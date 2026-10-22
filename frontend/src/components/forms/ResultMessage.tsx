// SPDX-License-Identifier: Apache-2.0
/* components/forms/ResultMessage.tsx - dòng báo kết quả của một việc làm trên danh sách, nhận focus khi hiện.
Dùng khi nút vừa bấm biến mất sau khi xong (xóa thành viên làm mất cả dòng; khóa tài khoản đổi nút Khóa thành Mở khóa):
không chuyển focus thì focus rơi về đầu trang, người dùng bàn phím và trình đọc màn hình mất chỗ đang đứng. Trang gắn
key theo từng kết quả để mỗi kết quả mới gắn lại component và nhận focus lại. */

import { useEffect, useRef } from 'react'
import { FormMessage } from './FormMessage'

type Props = {
  kind: 'error' | 'success'
  children: string
}

export function ResultMessage({ kind, children }: Props) {
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    ref.current?.focus()
  }, [])
  return (
    <div ref={ref} tabIndex={-1} className="rounded-md focus:outline-none focus-visible:ring-2 focus-visible:ring-ring">
      <FormMessage kind={kind}>{children}</FormMessage>
    </div>
  )
}
