// SPDX-License-Identifier: Apache-2.0
/* components/forms/RoleSelect.tsx - ô chọn vai trong workspace (Admin, Thành viên). Dùng thẻ select của trình duyệt:
trên điện thoại nó mở bảng chọn của hệ điều hành, dễ bấm hơn menu tự dựng. Chữ của vai lấy từ shell/labels.ts. */

import type { ComponentProps } from 'react'
import { cn } from '@/lib/utils'
import { roleLabel, workspaceRoles } from '@/components/shell/labels'

type Props = Omit<ComponentProps<'select'>, 'children'>

export function RoleSelect({ className, ...props }: Props) {
  return (
    <select
      className={cn(
        'h-9 rounded-lg border border-input bg-card px-2 text-sm outline-none',
        'focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:opacity-50',
        className,
      )}
      {...props}
    >
      {workspaceRoles.map((role) => (
        <option key={role} value={role}>
          {roleLabel(role)}
        </option>
      ))}
    </select>
  )
}
