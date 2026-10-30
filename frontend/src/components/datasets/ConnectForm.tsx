// SPDX-License-Identifier: Apache-2.0
/* components/datasets/ConnectForm.tsx - form thêm nguồn Postgres: máy chủ, cổng, database, tài khoản, mật khẩu, sslmode.
Trang giữ giá trị và gọi backend (pages/DatabaseSources.tsx); backend thử kết nối trước khi lưu, lỗi hiện ở error.
Mật khẩu chỉ gửi đi một lần, backend lưu dạng mã hóa và không trả lại. */

import { useId } from 'react'
import { FormField } from '@/components/forms/FormField'
import { FormMessage } from '@/components/forms/FormMessage'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'

export type ConnectValues = {
  host: string
  port: string
  database: string
  user: string
  password: string
  sslmode: string
}

type Props = {
  values: ConnectValues
  onChange: (values: ConnectValues) => void
  sslModes: string[]
  busy: boolean
  error?: string
  onSubmit: () => void
  onCancel: () => void
}

export function ConnectForm({ values, onChange, sslModes, busy, error, onSubmit, onCancel }: Props) {
  const id = useId()
  const set = (key: keyof ConnectValues) => (event: { target: { value: string } }) =>
    onChange({ ...values, [key]: event.target.value })
  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(event) => {
        event.preventDefault()
        onSubmit()
      }}
    >
      <div className="grid gap-3 sm:grid-cols-[1fr_7rem]">
        <FormField id={`${id}-host`} label="Máy chủ" value={values.host} onChange={set('host')} required />
        <FormField
          id={`${id}-port`}
          label="Cổng"
          inputMode="numeric"
          value={values.port}
          onChange={set('port')}
          required
        />
      </div>
      <FormField id={`${id}-db`} label="Database" value={values.database} onChange={set('database')} required />
      <div className="grid gap-3 sm:grid-cols-2">
        <FormField
          id={`${id}-user`}
          label="Tài khoản"
          autoComplete="off"
          value={values.user}
          onChange={set('user')}
          required
          hint="Nên dùng tài khoản chỉ có quyền SELECT."
        />
        <FormField
          id={`${id}-password`}
          label="Mật khẩu"
          type="password"
          autoComplete="new-password"
          value={values.password}
          onChange={set('password')}
          hint="Lưu dạng mã hóa, không hiện lại."
        />
      </div>
      <div className="flex flex-col gap-1.5">
        <Label htmlFor={`${id}-ssl`}>sslmode</Label>
        <select
          id={`${id}-ssl`}
          className="h-9 w-40 rounded-md border bg-background px-2 text-sm"
          value={values.sslmode}
          onChange={set('sslmode')}
        >
          {sslModes.map((mode) => (
            <option key={mode} value={mode}>
              {mode}
            </option>
          ))}
        </select>
      </div>
      {error && <FormMessage kind="error">{error}</FormMessage>}
      <div className="flex flex-wrap gap-2">
        <Button type="submit" disabled={busy}>
          {busy ? 'Đang thử kết nối...' : 'Kết nối'}
        </Button>
        <Button type="button" variant="outline" onClick={onCancel} disabled={busy}>
          Hủy
        </Button>
      </div>
    </form>
  )
}
