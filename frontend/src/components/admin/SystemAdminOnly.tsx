// SPDX-License-Identifier: Apache-2.0
/* components/admin/SystemAdminOnly.tsx - câu báo khi người không phải quản trị hệ thống mở thẳng đường dẫn của trang
quản trị. Trang hiện câu này thay vì gọi API rồi hiện lỗi 403 kèm nút thử lại không bao giờ thành công. */

import { EmptyState } from '@/components/EmptyState'

export function SystemAdminOnly() {
  return (
    <EmptyState
      title="Trang này chỉ dành cho quản trị hệ thống"
      description="Tài khoản của bạn không có quyền quản trị hệ thống."
    />
  )
}
