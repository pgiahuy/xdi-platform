// SPDX-License-Identifier: Apache-2.0
/* pages/workspaceNav.ts - đường dẫn của trang trong một workspace và các mục điều hướng, theo thứ tự trên thanh bên.
path khớp với route con của workspaceRoute trong router.tsx; '' là trang đầu của workspace. Thêm trang là thêm route ở
router.tsx và một dòng ở workspaceNav. Trang quản trị hệ thống không thuộc workspace nào, đường dẫn ở globalNav.ts. */

import type { LucideIcon } from 'lucide-react'
import { Database, MessageSquareText, Users } from 'lucide-react'

export type WorkspaceNavEntry = {
  path: string
  label: string
  icon: LucideIcon
}

export const workspaceRoute = '/w/:ws'

/** Trang tài khoản trong workspace; mở từ menu tài khoản, không nằm trên thanh bên. */
export const accountPath = 'account'

export const membersPath = 'members'

export const workspaceNav: WorkspaceNavEntry[] = [
  { path: '', label: 'Hỏi dữ liệu', icon: MessageSquareText },
  { path: 'datasets', label: 'Dữ liệu', icon: Database },
  { path: membersPath, label: 'Thành viên', icon: Users },
]

/** Đường dẫn tới một trang của workspace; path rỗng là trang đầu. Mọi chỗ dựng link tới workspace đi qua hàm này. */
export function workspaceHref(slug: string, path: string): string {
  const base = workspaceRoute.replace(':ws', encodeURIComponent(slug))
  return path ? `${base}/${path}` : base
}
