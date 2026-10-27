// SPDX-License-Identifier: Apache-2.0
/* pages/AccountPage.tsx - trang tài khoản: thông tin đăng nhập và đổi mật khẩu. Tài khoản không thuộc workspace nào,
nhưng trong workspace trang mở ở /w/<workspace>/account, trong khung workspace, để không mất thanh bên khi đổi mật khẩu.
Ở ngoài workspace (trang quản trị, hoặc người chưa thuộc workspace nào) cùng trang mở ở /account. */

import { useMemo, useState } from 'react'
import { passwordLength, useChangePassword, useMe } from '@/api/auth'
import { ApiError } from '@/api/client'
import { type ChangePasswordField, ChangePasswordForm } from '@/components/account/ChangePasswordForm'
import type { FormError } from '@/components/forms/formError'

export function AccountPage() {
  const me = useMe()
  const change = useChangePassword()
  // Đổi key để gắn lại form, xóa các ô mật khẩu sau khi đổi xong.
  const [formKey, setFormKey] = useState(0)
  const [success, setSuccess] = useState<string>()
  // 400 là mật khẩu hiện tại sai (độ dài mật khẩu mới đã kiểm ở form); 401 thì queryClient đưa về màn đăng nhập.
  const error = useMemo<FormError<ChangePasswordField> | undefined>(() => {
    if (!change.error) return undefined
    const wrongCurrent = change.error instanceof ApiError && change.error.status === 400
    return { message: change.error.message, field: wrongCurrent ? 'currentPassword' : undefined }
  }, [change.error])

  // AppLayout chỉ hiện trang con khi đã có người dùng, nên ở đây me.data luôn có.
  const user = me.data?.user

  return (
    <div className="flex flex-col gap-8">
      {user && (
        <section aria-labelledby="account-info" className="flex flex-col gap-3 rounded-xl border bg-card p-4 shadow-xs sm:px-6 sm:py-5">
          <h2 id="account-info" className="text-[15px] font-semibold tracking-tight">
            Thông tin đăng nhập
          </h2>
          <dl className="grid max-w-sm grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1 text-sm">
            <dt className="text-muted-foreground">Email</dt>
            <dd className="truncate">{user.email}</dd>
            <dt className="text-muted-foreground">Tên hiển thị</dt>
            <dd className="truncate">{user.display_name || 'Chưa đặt'}</dd>
          </dl>
        </section>
      )}
      <section aria-labelledby="account-password" className="flex flex-col gap-3 rounded-xl border bg-card p-4 shadow-xs sm:px-6 sm:py-5">
        <div>
          <h2 id="account-password" className="text-[15px] font-semibold tracking-tight">
            Đổi mật khẩu
          </h2>
          <p className="mt-1 text-sm text-muted-foreground">
            Đổi xong, các phiên đăng nhập khác của tài khoản này bị đăng xuất; phiên đang dùng giữ nguyên.
          </p>
        </div>
        <ChangePasswordForm
          key={formKey}
          username={user?.email ?? ''}
          pending={change.isPending}
          error={error}
          success={success}
          passwordMin={passwordLength.min}
          passwordMax={passwordLength.max}
          onSubmit={(values) => {
            setSuccess(undefined)
            change.mutate(
              { current_password: values.currentPassword, new_password: values.newPassword },
              {
                onSuccess: () => {
                  setFormKey((k) => k + 1)
                  setSuccess('Đã đổi mật khẩu. Các phiên đăng nhập khác của tài khoản này đã bị đăng xuất.')
                },
              },
            )
          }}
        />
      </section>
    </div>
  )
}
