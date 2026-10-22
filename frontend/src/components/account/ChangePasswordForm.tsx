// SPDX-License-Identifier: Apache-2.0
/* components/account/ChangePasswordForm.tsx - form đổi mật khẩu của chính mình: mật khẩu hiện tại, mật khẩu mới.
Đổi xong thì trang cha gắn lại form (đổi key) để xóa các ô, và báo kết quả qua success.
Có một ô email ẩn (autocomplete username): trình quản lý mật khẩu dựa vào nó để biết cập nhật mục của tài khoản nào. */

import { useState, type FormEvent } from 'react'
import { Button } from '@/components/ui/button'
import { FormField } from '@/components/forms/FormField'
import { checkNewPassword, type FormError, useFocusOnError } from '@/components/forms/formError'
import { FormMessage } from '@/components/forms/FormMessage'
import { NewPasswordFields } from '@/components/forms/NewPasswordFields'

export type ChangePasswordValues = {
  currentPassword: string
  newPassword: string
}

export type ChangePasswordField = 'currentPassword' | 'password' | 'confirm'

const fieldIds: Record<ChangePasswordField, string> = {
  currentPassword: 'account-current',
  password: 'account-new-password',
  confirm: 'account-new-confirm',
}

type Props = {
  username: string
  onSubmit: (values: ChangePasswordValues) => void
  pending: boolean
  error?: FormError<ChangePasswordField>
  success?: string
  passwordMin: number
  passwordMax: number
}

export function ChangePasswordForm(props: Props) {
  const { username, onSubmit, pending, error, success, passwordMin, passwordMax } = props
  const [currentPassword, setCurrentPassword] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [confirm, setConfirm] = useState('')
  const [localError, setLocalError] = useState<FormError<ChangePasswordField>>()
  const shown = localError ?? error
  useFocusOnError(shown, fieldIds)
  const fieldError = (field: ChangePasswordField) => (shown?.field === field ? shown.message : undefined)

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (pending) return
    const problem = checkNewPassword(newPassword, confirm, passwordMin, passwordMax)
    setLocalError(problem)
    if (problem) return
    onSubmit({ currentPassword, newPassword })
  }

  return (
    <form onSubmit={handleSubmit} className="flex max-w-sm flex-col gap-4">
      <input
        type="text"
        name="username"
        autoComplete="username"
        value={username}
        readOnly
        tabIndex={-1}
        aria-hidden="true"
        className="sr-only"
      />
      <FormField
        id={fieldIds.currentPassword}
        label="Mật khẩu hiện tại"
        type="password"
        autoComplete="current-password"
        required
        error={fieldError('currentPassword')}
        value={currentPassword}
        onChange={(e) => setCurrentPassword(e.target.value)}
      />
      <NewPasswordFields
        passwordId={fieldIds.password}
        confirmId={fieldIds.confirm}
        label="Mật khẩu mới"
        password={newPassword}
        confirm={confirm}
        onPasswordChange={setNewPassword}
        onConfirmChange={setConfirm}
        minLength={passwordMin}
        maxLength={passwordMax}
        passwordError={fieldError('password')}
        confirmError={fieldError('confirm')}
      />
      {shown && !shown.field && <FormMessage kind="error">{shown.message}</FormMessage>}
      {success && !shown && <FormMessage kind="success">{success}</FormMessage>}
      <div>
        <Button type="submit" aria-disabled={pending}>
          {pending ? 'Đang đổi...' : 'Đổi mật khẩu'}
        </Button>
      </div>
    </form>
  )
}
