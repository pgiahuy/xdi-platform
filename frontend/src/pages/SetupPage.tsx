// SPDX-License-Identifier: Apache-2.0
/* pages/SetupPage.tsx - thiết lập lần đầu ở /setup: tạo tài khoản quản trị kèm workspace đầu tiên, rồi vào workspace
đó. Bản cài đã có tài khoản thì chuyển sang màn đăng nhập. */

import { useMemo } from 'react'
import { Navigate, useNavigate } from 'react-router'
import { passwordLength, useSetup, useSetupStatus } from '@/api/auth'
import { ApiError } from '@/api/client'
import { useVersion } from '@/api/system'
import { AuthScreen } from '@/components/auth/AuthScreen'
import { type SetupField, SetupForm } from '@/components/auth/SetupForm'
import type { FormError } from '@/components/forms/formError'
import { ServerInfo } from '@/components/shell/ServerInfo'
import { StatusScreen } from '@/components/StatusScreen'
import { Button } from '@/components/ui/button'
import { loginRoute } from './authNav'
import { workspaceHref } from './workspaceNav'

// Cùng lệnh với docs/INSTALL.md mục chạy bằng Docker.
const setupCodeCommand = 'docker compose logs backend | grep setup_code'

export function SetupPage() {
  const navigate = useNavigate()
  const status = useSetupStatus()
  const version = useVersion()
  const setup = useSetup()
  // Gắn lỗi của backend vào ô gây ra nó. 403 là mã thiết lập sai (bản cài đã thiết lập thì trang tự chuyển đi). 400 ở
  // đây chỉ còn là email không hợp lệ: độ dài mật khẩu đã kiểm ở form trước khi gửi.
  const error = useMemo<FormError<SetupField> | undefined>(() => {
    if (!setup.error) return undefined
    const status = setup.error instanceof ApiError ? setup.error.status : 0
    const field = status === 403 ? 'setupCode' : status === 400 ? 'email' : undefined
    return { message: setup.error.message, field }
  }, [setup.error])

  if (status.isPending) return <StatusScreen busy title="Đang tải..." />
  if (status.error) {
    return (
      <StatusScreen
        title="Không kiểm được trạng thái thiết lập"
        description={status.error.message}
        action={
          <Button variant="outline" onClick={() => status.refetch()}>
            Thử lại
          </Button>
        }
      />
    )
  }
  // Đang gửi hoặc vừa xong thì không chuyển đi: needed vừa đổi thành false do chính lần thiết lập này.
  if (!status.data.needed && !setup.isPending && !setup.isSuccess) return <Navigate replace to={loginRoute} />

  return (
    <AuthScreen
      title="Thiết lập lần đầu"
      description="Bản cài chưa có tài khoản nào. Tạo tài khoản quản trị hệ thống và workspace đầu tiên."
      footer={version.data && <ServerInfo info={version.data} />}
    >
      <SetupForm
        pending={setup.isPending}
        error={error}
        passwordMin={passwordLength.min}
        passwordMax={passwordLength.max}
        setupCodeCommand={setupCodeCommand}
        onSubmit={(values) =>
          setup.mutate(
            {
              setup_code: values.setupCode,
              email: values.email,
              display_name: values.displayName,
              password: values.password,
              workspace_name: values.workspaceName,
            },
            {
              onSuccess: (me) => {
                const first = me.workspaces[0]
                navigate(first ? workspaceHref(first.slug, '') : '/', { replace: true })
              },
            },
          )
        }
      />
    </AuthScreen>
  )
}
