// SPDX-License-Identifier: Apache-2.0
/* components/forms/formError.ts - lỗi của một form (câu báo, và ô gây lỗi nếu biết), cách đưa focus về ô lỗi, và kiểm
mật khẩu mới trước khi gửi. Có field thì form hiện câu ngay dưới ô đó và đưa focus về ô; không có thì hiện một dòng
chung cuối form. Trang quyết định field từ mã lỗi HTTP, vì chỉ trang biết endpoint nào trả lỗi gì. */

import { useEffect } from 'react'

export type FormError<Field extends string> = {
  message: string
  field?: Field
}

/** Đếm ký tự như backend (chars().count() trong Rust): mỗi ký tự Unicode là một, kể cả emoji. String.length của
 * JavaScript đếm theo đơn vị UTF-16, emoji tính là hai. */
export function countChars(value: string): number {
  return [...value].length
}

/** Đưa focus về ô gây lỗi mỗi khi có lỗi mới. fieldIds là id của các ô, khai báo một lần ở cấp module của form để
 * không đổi giữa các lần vẽ. Trang phải giữ nguyên đối tượng lỗi giữa các lần vẽ (useMemo theo lỗi của mutation): đối
 * tượng mới mỗi lần vẽ thì focus bị kéo về ô lỗi liên tục. */
export function useFocusOnError<Field extends string>(
  error: FormError<Field> | undefined,
  fieldIds: Partial<Record<Field, string>>,
) {
  useEffect(() => {
    const id = error?.field && fieldIds[error.field]
    if (id) document.getElementById(id)?.focus()
  }, [error, fieldIds])
}

/** Kiểm mật khẩu mới trước khi gửi, cùng quy tắc và cùng câu với backend (backend/src/auth/password.rs). */
export function checkNewPassword(
  password: string,
  confirm: string,
  min: number,
  max: number,
): { field: 'password' | 'confirm'; message: string } | undefined {
  const length = countChars(password)
  if (length < min) return { field: 'password', message: `Mật khẩu cần ít nhất ${min} ký tự` }
  if (length > max) return { field: 'password', message: `Mật khẩu dài tối đa ${max} ký tự` }
  if (password !== confirm) return { field: 'confirm', message: 'Hai ô mật khẩu không giống nhau' }
  return undefined
}
