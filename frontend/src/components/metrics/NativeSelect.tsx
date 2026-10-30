// SPDX-License-Identifier: Apache-2.0
/* components/metrics/NativeSelect.tsx - ô chọn của màn Chỉ số (phép tính, cột, cột thời gian, cách gộp). Dùng thẻ select
của trình duyệt như forms/RoleSelect.tsx: trên điện thoại nó mở bảng chọn của hệ điều hành. */

import type { ComponentProps } from 'react'
import { cn } from '@/lib/utils'

type Props = ComponentProps<'select'> & {
  options: { value: string; label: string }[]
}

export function NativeSelect({ options, className, ...props }: Props) {
  return (
    <select
      className={cn(
        'h-9 min-w-0 rounded-lg border border-input bg-card px-2 text-sm outline-none',
        'focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:opacity-50',
        'aria-invalid:border-destructive',
        className,
      )}
      {...props}
    >
      {options.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  )
}
