// SPDX-License-Identifier: Apache-2.0
/* components/auth/SetupForm.tsx - form tạo tài khoản quản trị lần đầu: mã thiết lập, email, tên, mật khẩu, tên
workspace đầu tiên. Mã thiết lập chứng minh người điền là người chạy máy chủ (mã chỉ in trong log của backend), nên
màn phải nói rõ lấy mã ở đâu; không có dòng đó người cài không biết làm tiếp. */

import { useState, type FormEvent } from 'react'
import { Button } from '@/components/ui/button'
import { FormField } from '@/components/forms/FormField'
import { checkNewPassword, type FormError, useFocusOnError } from '@/components/forms/formError'
import { FormMessage } from '@/components/forms/FormMessage'
import { NewPasswordFields } from '@/components/forms/NewPasswordFields'

export type SetupValues = {
  setupCode: string
  email: string
  displayName: string
  password: string
  workspaceName: string
}

export type SetupField = 'setupCode' | 'email' | 'password' | 'confirm'

const fieldIds: Record<SetupField, string> = {
  setupCode: 'setup-code',
  email: 'setup-email',
  password: 'setup-password',
  confirm: 'setup-confirm',
}

type Props = {
  onSubmit: (values: SetupValues) => void
  pending: boolean
  error?: FormError<SetupField>
  passwordMin: number
  passwordMax: number
  /** Lệnh xem mã thiết lập khi chạy bằng Docker, hiện ngay dưới ô nhập mã. */
  setupCodeCommand: string
}

export function SetupForm({ onSubmit, pending, error, passwordMin, passwordMax, setupCodeCommand }: Props) {
  const [setupCode, setSetupCode] = useState('')
  const [email, setEmail] = useState('')
  const [displayName, setDisplayName] = useState('')
  const [password, setPassword] = useState('')
  const [confirm, setConfirm] = useState('')
  const [workspaceName, setWorkspaceName] = useState('')
  const [localError, setLocalError] = useState<FormError<SetupField>>()
  // Lỗi kiểm ở form đứng trước lỗi của lần gửi trước.
  const shown = localError ?? error
  useFocusOnError(shown, fieldIds)
  const fieldError = (field: SetupField) => (shown?.field === field ? shown.message : undefined)

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (pending) return
    const problem = checkNewPassword(password, confirm, passwordMin, passwordMax)
    setLocalError(problem)
    if (problem) return
    onSubmit({ setupCode, email, displayName, password, workspaceName })
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-4">
      <FormField
        id={fieldIds.setupCode}
        label="Mã thiết lập"
        required
        autoFocus
        autoComplete="off"
        autoCapitalize="characters"
        spellCheck={false}
        placeholder="ABCD-EFGH-JKMN"
        error={fieldError('setupCode')}
        value={setupCode}
        onChange={(e) => setSetupCode(e.target.value)}
        hint={
          <>
            Mã in trong log của máy chủ lúc khởi động. Chạy bằng Docker thì xem bằng lệnh:
            <code className="mt-1 block rounded bg-muted px-2 py-1 font-mono break-words text-foreground">
              {setupCodeCommand}
            </code>
          </>
        }
      />
      <FormField
        id={fieldIds.email}
        label="Email quản trị"
        type="email"
        autoComplete="username"
        required
        error={fieldError('email')}
        value={email}
        onChange={(e) => setEmail(e.target.value)}
      />
      <FormField
        id="setup-name"
        label="Tên hiển thị"
        autoComplete="name"
        hint="Không bắt buộc."
        value={displayName}
        onChange={(e) => setDisplayName(e.target.value)}
      />
      <NewPasswordFields
        passwordId={fieldIds.password}
        confirmId={fieldIds.confirm}
        label="Mật khẩu"
        password={password}
        confirm={confirm}
        onPasswordChange={setPassword}
        onConfirmChange={setConfirm}
        minLength={passwordMin}
        maxLength={passwordMax}
        passwordError={fieldError('password')}
        confirmError={fieldError('confirm')}
      />
      <FormField
        id="setup-workspace"
        label="Tên workspace đầu tiên"
        hint="Ví dụ tên phòng ban. Để trống thì đặt là Workspace đầu tiên; đổi tên được sau."
        value={workspaceName}
        onChange={(e) => setWorkspaceName(e.target.value)}
      />
      {shown && !shown.field && <FormMessage kind="error">{shown.message}</FormMessage>}
      <Button type="submit" size="lg" aria-disabled={pending}>
        {pending ? 'Đang tạo...' : 'Tạo tài khoản quản trị'}
      </Button>
    </form>
  )
}
