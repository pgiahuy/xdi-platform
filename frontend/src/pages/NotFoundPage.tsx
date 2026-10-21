// SPDX-License-Identifier: Apache-2.0
/* pages/NotFoundPage.tsx - đường dẫn không có trang nào. */

import { Link } from 'react-router'
import { StatusScreen } from '@/components/StatusScreen'
import { Button } from '@/components/ui/button'

export function NotFoundPage() {
  return (
    <StatusScreen
      title="Không có trang này"
      description="Đường dẫn sai hoặc trang đã đổi chỗ."
      action={
        <Button variant="outline" asChild>
          <Link to="/">Về trang đầu</Link>
        </Button>
      }
    />
  )
}
