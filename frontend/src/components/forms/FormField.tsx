// SPDX-License-Identifier: Apache-2.0
/* components/forms/FormField.tsx - một ô nhập có nhãn, dòng nhắc và câu lỗi bên dưới. Dùng cho mọi form của giao diện.
Nhãn nối với ô qua htmlFor; dòng nhắc và câu lỗi nối qua aria-describedby, ô có lỗi đặt aria-invalid: trình đọc màn
hình đọc cả nhãn, lỗi và nhắc khi vào ô. Câu lỗi có role alert để được đọc ngay khi hiện. */

import type { ComponentProps, ReactNode } from 'react'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

type Props = ComponentProps<typeof Input> & {
  id: string
  label: string
  hint?: ReactNode
  error?: string
}

export function FormField({ id, label, hint, error, ...inputProps }: Props) {
  const hintId = hint ? `${id}-hint` : undefined
  const errorId = error ? `${id}-error` : undefined
  const describedBy = [errorId, hintId].filter(Boolean).join(' ') || undefined
  return (
    <div className="flex flex-col gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      <Input id={id} aria-describedby={describedBy} aria-invalid={error ? true : undefined} className="h-9" {...inputProps} />
      {error && (
        <p id={errorId} role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      {hint && (
        <div id={hintId} className="text-xs text-muted-foreground">
          {hint}
        </div>
      )}
    </div>
  )
}
