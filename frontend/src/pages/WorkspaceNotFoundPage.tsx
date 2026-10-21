// SPDX-License-Identifier: Apache-2.0
/* pages/WorkspaceNotFoundPage.tsx - đường dẫn sai bên trong một workspace, ví dụ /w/<workspace>/sai. Hiện trong khung
workspace để người dùng vẫn thấy thanh bên và đi tiếp, thay vì rơi ra màn lỗi cả trang. Tiêu đề trang (h1 và tên
tab) đặt ở handle của route trong router.tsx, vì h1 nằm trên thanh trên của khung. */

import { Link, useParams } from 'react-router'
import { EmptyState } from '@/components/EmptyState'
import { Button } from '@/components/ui/button'
import { workspaceHref } from './workspaceNav'

export function WorkspaceNotFoundPage() {
  const { ws = '' } = useParams()
  return (
    <EmptyState
      title="Đường dẫn sai hoặc trang đã đổi chỗ"
      action={
        <Button variant="outline" asChild>
          <Link to={workspaceHref(ws, '')}>Về trang đầu của workspace</Link>
        </Button>
      }
    />
  )
}
