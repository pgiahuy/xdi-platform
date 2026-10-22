// SPDX-License-Identifier: Apache-2.0
/* components/admin/CreateAccountForm.tsx - quản trị hệ thống tạo tài khoản mới: email, tên, mật khẩu ban đầu.
Mật khẩu hiện rõ chữ, vì quản trị phải chuyển nó cho người dùng. Độ dài mật khẩu kiểm trước khi gửi, cùng quy tắc và
cùng câu với backend; lỗi của một ô hiện ngay dưới ô đó và đưa focus về ô (quy ước ở forms/formError.ts). Tạo xong thì
trang cha gắn lại form (đổi key) để xóa các ô. Tài khoản mới chưa thuộc workspace nào cho tới khi admin thêm họ. */

import { useState, type FormEvent } from 'react'
import { FormField } from '@/components/forms/FormField'
import { countChars, type FormError, useFocusOnError } from '@/components/forms/formError'
import { FormMessage } from '@/components/forms/FormMessage'
import { Button } from '@/components/ui/button'

export type CreateAccountValues = {
  email: string
  displayName: string
  password: string
}

export type CreateAccountField = 'email' | 'password'

const fieldIds: Record<CreateAccountField, string> = { email: 'account-email', password: 'account-password' }

type Props = {
  onSubmit: (values: CreateAccountValues) => void
  pending: boolean
  error?: FormError<CreateAccountField>
  success?: string
  passwordMin: number
  passwordMax: number
}

export function CreateAccountForm({ onSubmit, pending, error, success, passwordMin, passwordMax }: Props) {
  const [email, setEmail] = useState('')
  const [displayName, setDisplayName] = useState('')
  const [password, setPassword] = useState('')
  const [localError, setLocalError] = useState<FormError<CreateAccountField>>()
  const shown = localError ?? error
  useFocusOnError(shown, fieldIds)
  const fieldError = (field: CreateAccountField) => (shown?.field === field ? shown.message : undefined)

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (pending) return
    const length = countChars(password)
    const problem: FormError<CreateAccountField> | undefined =
      length < passwordMin
        ? { field: 'password', message: `Mật khẩu cần ít nhất ${passwordMin} ký tự` }
        : length > passwordMax
          ? { field: 'password', message: `Mật khẩu dài tối đa ${passwordMax} ký tự` }
          : undefined
    setLocalError(problem)
    if (problem) return
    onSubmit({ email: email.trim(), displayName: displayName.trim(), password })
  }

  return (
    <form onSubmit={handleSubmit} className="flex max-w-sm flex-col gap-4">
      <FormField
        id={fieldIds.email}
        label="Email"
        type="email"
        autoComplete="off"
        required
        error={fieldError('email')}
        value={email}
        onChange={(e) => setEmail(e.target.value)}
      />
      <FormField
        id="account-name"
        label="Tên hiển thị"
        autoComplete="off"
        value={displayName}
        onChange={(e) => setDisplayName(e.target.value)}
      />
      <FormField
        id={fieldIds.password}
        label="Mật khẩu ban đầu"
        autoComplete="off"
        required
        hint={
          `Từ ${passwordMin} đến ${passwordMax} ký tự. ` +
          'Chuyển mật khẩu này cho người dùng; họ đổi lại sau khi đăng nhập.'
        }
        error={fieldError('password')}
        value={password}
        onChange={(e) => setPassword(e.target.value)}
      />
      {shown && !shown.field && <FormMessage kind="error">{shown.message}</FormMessage>}
      {success && !shown && <FormMessage kind="success">{success}</FormMessage>}
      <div>
        <Button type="submit" aria-disabled={pending}>
          {pending ? 'Đang tạo...' : 'Tạo tài khoản'}
        </Button>
      </div>
    </form>
  )
}
