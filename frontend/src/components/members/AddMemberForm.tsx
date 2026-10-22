// SPDX-License-Identifier: Apache-2.0
/* components/members/AddMemberForm.tsx - admin thêm thành viên theo email, chọn vai. Tài khoản phải có sẵn (quản trị
hệ thống tạo); email chưa có tài khoản hay đã là thành viên thì backend báo lời, form hiện ngay dưới ô email và đưa
focus về ô đó (quy ước ở forms/formError.ts). Thêm xong thì trang cha gắn lại form (đổi key) để xóa ô email. */

import { useState, type FormEvent } from 'react'
import { FormField } from '@/components/forms/FormField'
import { type FormError, useFocusOnError } from '@/components/forms/formError'
import { FormMessage } from '@/components/forms/FormMessage'
import { RoleSelect } from '@/components/forms/RoleSelect'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'

export type AddMemberField = 'email'

const fieldIds: Record<AddMemberField, string> = { email: 'member-email' }

type Props = {
  onSubmit: (email: string, role: string) => void
  pending: boolean
  error?: FormError<AddMemberField>
  success?: string
}

export function AddMemberForm({ onSubmit, pending, error, success }: Props) {
  const [email, setEmail] = useState('')
  const [role, setRole] = useState('member')
  useFocusOnError(error, fieldIds)

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (pending) return
    onSubmit(email.trim(), role)
  }

  return (
    <form onSubmit={handleSubmit} className="flex max-w-xl flex-col gap-3">
      {/* items-start, không items-end: câu lỗi dưới ô email làm cột đó cao hơn, nút vẫn phải ngang hàng với ô. */}
      <div className="flex flex-wrap items-start gap-3">
        <div className="min-w-0 flex-1 basis-56">
          <FormField
            id={fieldIds.email}
            label="Email"
            type="email"
            autoComplete="off"
            required
            error={error?.field === 'email' ? error.message : undefined}
            value={email}
            onChange={(e) => setEmail(e.target.value)}
          />
        </div>
        <div className="flex flex-col gap-1.5">
          <Label htmlFor="member-role">Vai</Label>
          <RoleSelect id="member-role" value={role} onChange={(e) => setRole(e.target.value)} />
        </div>
        <div className="flex flex-col gap-1.5">
          {/* Chỗ trống cao bằng nhãn, để nút ngang hàng với ô nhập. */}
          <span aria-hidden="true" className="text-sm leading-none">
            &nbsp;
          </span>
          <Button type="submit" aria-disabled={pending} className="h-9">
            {pending ? 'Đang thêm...' : 'Thêm'}
          </Button>
        </div>
      </div>
      {error && !error.field && <FormMessage kind="error">{error.message}</FormMessage>}
      {success && !error && <FormMessage kind="success">{success}</FormMessage>}
    </form>
  )
}
