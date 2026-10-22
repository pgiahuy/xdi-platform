// SPDX-License-Identifier: Apache-2.0
/* pages/LoginPage.tsx - màn đăng nhập ở /login. Bản cài chưa có tài khoản thì chuyển sang màn thiết lập; đã đăng nhập
thì về trang trong tham số next. Đăng nhập xong cũng về next, để người dùng làm tiếp đúng chỗ đang dở. */

import { useMemo } from 'react'
import { Navigate, useNavigate, useSearchParams } from 'react-router'
import { useLogin, useMe, useSetupStatus } from '@/api/auth'
import { isUnauthorized } from '@/api/queryClient'
import { useVersion } from '@/api/system'
import { AuthScreen } from '@/components/auth/AuthScreen'
import { type LoginField, LoginForm } from '@/components/auth/LoginForm'
import type { FormError } from '@/components/forms/formError'
import { ServerInfo } from '@/components/shell/ServerInfo'
import { StatusScreen } from '@/components/StatusScreen'
import { safeNext, setupRoute } from './authNav'

export function LoginPage() {
  const [params] = useSearchParams()
  const navigate = useNavigate()
  const setup = useSetupStatus()
  const me = useMe()
  const version = useVersion()
  const login = useLogin()
  const next = safeNext(params.get('next'))
  // 401 là sai email hoặc mật khẩu: báo ở ô mật khẩu và đưa focus về đó, giữ email đã gõ. Lỗi khác (mạng, máy chủ)
  // báo chung cuối form.
  const error = useMemo<FormError<LoginField> | undefined>(
    () =>
      login.error
        ? { message: login.error.message, field: isUnauthorized(login.error) ? 'password' : undefined }
        : undefined,
    [login.error],
  )

  if (setup.data?.needed) return <Navigate replace to={setupRoute} />
  // Đã đăng nhập từ trước thì đi tiếp. Trong lúc và ngay sau một lần đăng nhập ở form này thì không: người dùng mới vừa
  // vào bộ đệm, và chuyển trang do handleSubmit quyết định (có thể không theo next, xem dưới).
  const signingIn = login.isPending || login.isSuccess
  if (me.data && !isUnauthorized(me.error) && !signingIn) return <Navigate replace to={next} />
  // Chỉ lần kiểm phiên đầu tiên mới hiện đang tải, để người đã đăng nhập không thấy form nháy lên rồi biến mất. Các lần
  // kiểm lại sau (quay lại tab) giữ nguyên form: gỡ form ra là mất các ô đã gõ.
  if ((me.isPending && !me.isFetched) || setup.isPending) return <StatusScreen busy title="Đang tải..." />

  // Người đăng nhập khác người còn trong bộ đệm (phiên trước hết hạn) thì next là trang của người trước; về trang gốc.
  const previousUserId = me.data?.user.id
  const handleSubmit = (values: { email: string; password: string }) =>
    login.mutate(values, {
      onSuccess: (signedIn) => {
        const sameUser = previousUserId === undefined || previousUserId === signedIn.user.id
        navigate(sameUser ? next : '/', { replace: true })
      },
    })

  return (
    <AuthScreen
      title="Đăng nhập"
      description="Tài khoản do quản trị hệ thống của bản cài này tạo."
      footer={version.data && <ServerInfo info={version.data} />}
    >
      <LoginForm pending={login.isPending} error={error} onSubmit={handleSubmit} />
      {/* Quên mật khẩu không tự đặt lại được: chưa có gửi thư. Nói rõ cần nhờ ai. */}
      <p className="mt-4 text-xs text-muted-foreground">Quên mật khẩu thì nhờ quản trị hệ thống đặt lại.</p>
    </AuthScreen>
  )
}
