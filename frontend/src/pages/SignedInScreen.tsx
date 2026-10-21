// SPDX-License-Identifier: Apache-2.0
/* pages/SignedInScreen.tsx - màn cả trang cho người đã đăng nhập nhưng chưa vào được workspace nào: chưa thuộc
workspace, hoặc mở workspace không có quyền. Luôn có nút đăng xuất và ghi email đang dùng, vì một lý do hay gặp là
đăng nhập nhầm tài khoản; không có hai thứ này thì người dùng kẹt ở màn này. */

import type { ReactNode } from 'react'
import { useNavigate } from 'react-router'
import { useLogout, type User } from '@/api/auth'
import { StatusScreen } from '@/components/StatusScreen'
import { Button } from '@/components/ui/button'

type Props = {
  user: User
  title: string
  description: string
  action?: ReactNode
}

export function SignedInScreen({ user, title, description, action }: Props) {
  const logout = useLogout()
  const navigate = useNavigate()
  const handleLogout = () => logout.mutate(undefined, { onSuccess: () => navigate('/', { replace: true }) })

  return (
    <StatusScreen
      title={title}
      description={description}
      action={
        <div className="flex flex-wrap gap-2">
          {action}
          <Button variant="ghost" onClick={handleLogout} disabled={logout.isPending}>
            {logout.isPending ? 'Đang đăng xuất...' : 'Đăng xuất'}
          </Button>
        </div>
      }
      footer={
        <div className="flex flex-col gap-1 text-xs text-muted-foreground">
          <p>Đang đăng nhập bằng {user.email}</p>
          {logout.error && (
            <p role="alert" className="text-destructive">
              Đăng xuất không được: {logout.error.message}
            </p>
          )}
        </div>
      }
    />
  )
}
