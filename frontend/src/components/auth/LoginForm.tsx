// SPDX-License-Identifier: Apache-2.0
/* components/auth/LoginForm.tsx - form đăng nhập bằng email và mật khẩu.
Lỗi đăng nhập sai là một câu chung của backend, không nói email hay mật khẩu sai, để không dò được ai có tài khoản.
Nút gửi không bị disabled lúc chờ (chặn gửi trùng trong handler): disabled làm focus rơi khỏi nút, người dùng bàn phím
mất chỗ đang đứng. */

import { useState, type FormEvent } from 'react'
import { Button } from '@/components/ui/button'
import { FormField } from '@/components/forms/FormField'
import { type FormError, useFocusOnError } from '@/components/forms/formError'
import { FormMessage } from '@/components/forms/FormMessage'

export type LoginValues = {
  email: string
  password: string
}

export type LoginField = 'email' | 'password'

const fieldIds: Record<LoginField, string> = { email: 'login-email', password: 'login-password' }

type Props = {
  onSubmit: (values: LoginValues) => void
  pending: boolean
  error?: FormError<LoginField>
}

export function LoginForm({ onSubmit, pending, error }: Props) {
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  useFocusOnError(error, fieldIds)

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (pending) return
    onSubmit({ email, password })
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-4">
      <FormField
        id={fieldIds.email}
        label="Email"
        type="email"
        autoComplete="username"
        required
        autoFocus
        error={error?.field === 'email' ? error.message : undefined}
        value={email}
        onChange={(e) => setEmail(e.target.value)}
      />
      <FormField
        id={fieldIds.password}
        label="Mật khẩu"
        type="password"
        autoComplete="current-password"
        required
        error={error?.field === 'password' ? error.message : undefined}
        value={password}
        onChange={(e) => setPassword(e.target.value)}
      />
      {error && !error.field && <FormMessage kind="error">{error.message}</FormMessage>}
      <Button type="submit" size="lg" aria-disabled={pending}>
        {pending ? 'Đang đăng nhập...' : 'Đăng nhập'}
      </Button>
    </form>
  )
}
