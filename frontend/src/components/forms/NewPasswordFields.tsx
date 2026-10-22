// SPDX-License-Identifier: Apache-2.0
/* components/forms/NewPasswordFields.tsx - hai ô mật khẩu mới (nhập và nhập lại), dùng ở thiết lập lần đầu và đổi
mật khẩu. Nhập lại để bắt lỗi gõ nhầm trước khi gửi: mật khẩu quản trị gõ nhầm thì phải chạy lệnh trên máy chủ mới
đặt lại được. Form cha giữ giá trị và gọi checkNewPassword (formError.ts) khi gửi.
Không dùng minLength, maxLength của trình duyệt: trình duyệt đếm theo UTF-16 (lệch backend với emoji), và maxLength cắt
ngầm mật khẩu dán vào, nên mật khẩu được lưu khác mật khẩu người dùng nghĩ mình đặt. */

import { FormField } from './FormField'

type Props = {
  passwordId: string
  confirmId: string
  label: string
  password: string
  confirm: string
  onPasswordChange: (value: string) => void
  onConfirmChange: (value: string) => void
  minLength: number
  maxLength: number
  passwordError?: string
  confirmError?: string
}

export function NewPasswordFields(props: Props) {
  const { passwordId, confirmId, label, password, confirm, minLength, maxLength } = props
  return (
    <>
      <FormField
        id={passwordId}
        label={label}
        type="password"
        autoComplete="new-password"
        required
        hint={`Từ ${minLength} đến ${maxLength} ký tự.`}
        error={props.passwordError}
        value={password}
        onChange={(e) => props.onPasswordChange(e.target.value)}
      />
      <FormField
        id={confirmId}
        label="Nhập lại mật khẩu"
        type="password"
        autoComplete="new-password"
        required
        error={props.confirmError}
        value={confirm}
        onChange={(e) => props.onConfirmChange(e.target.value)}
      />
    </>
  )
}
