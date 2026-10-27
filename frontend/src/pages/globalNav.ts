// SPDX-License-Identifier: Apache-2.0
/* pages/globalNav.ts - đường dẫn của các trang không thuộc workspace nào: trang quản trị hệ thống và trang tài khoản
ngoài workspace. Các trang này hiện trong khung AppLayout mà không cần workspace đang mở, nên quản trị hệ thống chưa
thuộc workspace nào vẫn vào được để tự chỉ định mình làm admin một workspace. Thêm trang là thêm route con của
AppLayout ở router.tsx và, nếu cần hiện trên thanh bên, một dòng ở adminNav. */

import type { LucideIcon } from 'lucide-react'
import { LayoutGrid, UserCog } from 'lucide-react'

export type GlobalNavEntry = {
  href: string
  label: string
  icon: LucideIcon
}

/** Trang tài khoản cho người đang ở ngoài workspace. Trong workspace, menu tài khoản trỏ tới trang tài khoản của
 * workspace đó (workspaceNav.ts) để không mất thanh bên của workspace. */
export const accountRoute = '/account'

export const adminAccountsRoute = '/admin/accounts'
export const adminWorkspacesRoute = '/admin/workspaces'

/** Mục quản trị hệ thống, hiện ở cuối thanh bên cả trong và ngoài workspace, chỉ với quản trị hệ thống. Ẩn mục chỉ để
 * gọn; quyền vẫn do backend kiểm. Tên mục khác "Tài khoản và mật khẩu" trong menu tài khoản (trang của chính mình), để
 * không nhầm hai trang. */
export const adminNav: GlobalNavEntry[] = [
  { href: adminAccountsRoute, label: 'Quản lý tài khoản', icon: UserCog },
  { href: adminWorkspacesRoute, label: 'Mọi workspace', icon: LayoutGrid },
]
