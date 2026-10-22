// SPDX-License-Identifier: Apache-2.0
/* pages/SessionStatus.tsx - màn hiện khi chưa lấy được người dùng hiện tại: đang tải, lỗi; chưa đăng nhập thì chuyển
sang màn đăng nhập, nhớ trang đang mở để quay lại. RootPage và WorkspaceLayout cùng dùng, để hai nơi xử lý giống nhau.
Phiên bản máy chủ hiện ở chân màn lỗi, để người cài kiểm được backend có đang chạy không. */

import { Navigate, useLocation } from 'react-router'
import { isUnauthorized } from '@/api/queryClient'
import { useVersion } from '@/api/system'
import { ServerInfo } from '@/components/shell/ServerInfo'
import { StatusScreen } from '@/components/StatusScreen'
import { Button } from '@/components/ui/button'
import { loginHref } from './authNav'

type Props = {
  error: Error | null
  onRetry: () => void
}

export function SessionStatus({ error, onRetry }: Props) {
  const version = useVersion()
  const location = useLocation()
  const footer = version.data ? <ServerInfo info={version.data} /> : undefined

  if (!error) {
    return <StatusScreen busy title="Đang tải..." />
  }
  if (isUnauthorized(error)) {
    return <Navigate replace to={loginHref(location.pathname + location.search)} />
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
