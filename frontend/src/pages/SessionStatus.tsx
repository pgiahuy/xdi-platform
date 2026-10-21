// SPDX-License-Identifier: Apache-2.0
/* pages/SessionStatus.tsx - màn hiện khi chưa lấy được người dùng hiện tại: đang tải, chưa đăng nhập, hoặc lỗi.
RootPage và WorkspaceLayout cùng dùng, để hai nơi báo giống nhau. Phiên bản máy chủ vẫn hiện ở chân màn, để người cài
kiểm được backend đang chạy kể cả khi chưa đăng nhập. */

import { ApiError } from '@/api/client'
import { useVersion } from '@/api/system'
import { ServerInfo } from '@/components/shell/ServerInfo'
import { StatusScreen } from '@/components/StatusScreen'
import { Button } from '@/components/ui/button'

type Props = {
  error: Error | null
  onRetry: () => void
}

export function SessionStatus({ error, onRetry }: Props) {
  const version = useVersion()
  const footer = version.data ? <ServerInfo info={version.data} /> : undefined

  if (!error) {
    return <StatusScreen busy title="Đang tải..." />
  }
  // Màn đăng nhập thuộc #48; tới lúc đó, chỗ này chuyển sang trang đăng nhập thay vì chỉ báo.
  if (error instanceof ApiError && error.status === 401) {
    return (
      <StatusScreen
        title="Bạn chưa đăng nhập"
        description="Phiên đăng nhập đã hết hạn hoặc bạn chưa đăng nhập trên trình duyệt này."
        footer={footer}
      />
    )
  }
  return (
    <StatusScreen
      title="Không tải được thông tin tài khoản"
      description={error.message}
      action={
        <Button variant="outline" onClick={onRetry}>
          Thử lại
        </Button>
      }
      footer={footer}
    />
  )
}
