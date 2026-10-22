// SPDX-License-Identifier: Apache-2.0
/* pages/RouteErrorPage.tsx - màn hiện khi một trang ném lỗi lúc vẽ, lúc chuyển trang hoặc lúc tải phần trang tải riêng
(errorElement ở gốc router.tsx). Không có màn này thì React Router hiện màn lỗi mặc định bằng tiếng Anh kèm stack trace.
Chi tiết lỗi vẫn ghi ở console của trình duyệt cho người sửa lỗi. */

import { useEffect } from 'react'
import { useRouteError } from 'react-router'
import { StatusScreen } from '@/components/StatusScreen'
import { Button } from '@/components/ui/button'

export function RouteErrorPage() {
  const error = useRouteError()
  useEffect(() => {
    console.error(error)
  }, [error])
  return (
    <StatusScreen
      title="Không mở được trang này"
      description={
        'Trang gặp lỗi khi hiện. Về trang đầu rồi thử lại; nếu vẫn lỗi, ghi lại đường dẫn và báo người quản trị.'
      }
      action={
        // Thẻ a thường, không dùng Link: tải lại cả trang, bỏ trạng thái đang lỗi.
        <Button variant="outline" asChild>
          <a href="/">Về trang đầu</a>
        </Button>
      }
    />
  )
}
