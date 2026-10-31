// SPDX-License-Identifier: Apache-2.0
/* components/datasets/ConnectForm.tsx - form thêm nguồn Postgres: máy chủ, cổng, database, tài khoản, mật khẩu, sslmode,
và chứng chỉ CA (chỉ hiện với verify-full, cho máy chủ dùng CA riêng của tổ chức).
Trang giữ giá trị và gọi backend (pages/DatabaseSources.tsx); backend thử kết nối trước khi lưu, lỗi hiện ở error.
Mật khẩu chỉ gửi đi một lần, backend lưu dạng mã hóa và không trả lại. */

import { useId } from 'react'
import { FormField } from '@/components/forms/FormField'
import { FormMessage } from '@/components/forms/FormMessage'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'

export type ConnectValues = {
  host: string
  port: string
  database: string
  user: string
  password: string
  sslmode: string
  /** Chứng chỉ CA dạng PEM; chỉ gửi khi sslmode là verify-full. */
  sslRootCert: string
}

type Props = {
  values: ConnectValues
  onChange: (values: ConnectValues) => void
  /** Giá trị và lời giải thích ngắn của từng sslmode. */
  sslModes: { value: string; label: string }[]
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
          className="h-9 w-48 rounded-md border bg-background px-2 text-sm"
          value={values.sslmode}
          onChange={set('sslmode')}
        >
          {sslModes.map((mode) => (
            <option key={mode.value} value={mode.value}>
              {mode.value}
            </option>
          ))}
        </select>
        <p className="text-xs text-muted-foreground">
          {sslModes.find((mode) => mode.value === values.sslmode)?.label}
        </p>
      </div>
      {values.sslmode === 'verify-full' && (
        <div className="flex flex-col gap-1.5">
          <Label htmlFor={`${id}-ca`}>Chứng chỉ CA (không bắt buộc)</Label>
          <Textarea
            id={`${id}-ca`}
            aria-describedby={`${id}-ca-hint`}
            className="font-mono text-xs"
            rows={4}
            placeholder="-----BEGIN CERTIFICATE-----"
            value={values.sslRootCert}
            onChange={set('sslRootCert')}
          />
          <p id={`${id}-ca-hint`} className="text-xs text-muted-foreground">
            Để trống thì tin các CA của hệ thống. Máy chủ dùng CA riêng của tổ chức thì dán nội dung file PEM của CA.
          </p>
        </div>
      )}
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
