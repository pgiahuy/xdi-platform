// SPDX-License-Identifier: Apache-2.0
/* pages/workspaceNav.ts - đường dẫn của trang trong một workspace và các mục điều hướng, theo thứ tự trên thanh bên.
path khớp với route con của workspaceRoute trong router.tsx; '' là trang đầu của workspace. Thêm trang là thêm route ở
router.tsx và một dòng ở workspaceNav. Trang quản trị hệ thống không thuộc workspace nào, đường dẫn ở globalNav.ts.
Trang hỏi ('' là trang đầu) không có mục riêng: vào workspace là mở nó, và nút "Lịch sử trò chuyện" đứng đầu thanh bên mở
một cuộc cũ hay cuộc mới trên trang đó. */

import type { LucideIcon } from 'lucide-react'
import { BadgeCheck, ChartColumn, Database, Users } from 'lucide-react'

export type WorkspaceNavEntry = {
  path: string
  label: string
  icon: LucideIcon
  /** Nhãn nhóm trên thanh bên (mục quản trị là nhóm riêng). */
  section?: string
  /** Chỉ hiện với admin của workspace. */
  adminOnly?: boolean
}

export const workspaceRoute = '/w/:ws'

/** Trang tài khoản trong workspace; mở từ menu tài khoản, không nằm trên thanh bên. */
export const accountPath = 'account'

export const membersPath = 'members'

/** Trang Tổng quan dữ liệu; dataset đang xem nằm trong đường dẫn (?dataset=...). */
export const overviewPath = 'overview'

/** Màn Câu mẫu (#212): admin duyệt câu trả lời được đánh giá Tốt. */
export const examplesPath = 'examples'

export const workspaceNav: WorkspaceNavEntry[] = [
  { path: overviewPath, label: 'Tổng quan dữ liệu', icon: ChartColumn },
  { path: 'datasets', label: 'Dữ liệu', icon: Database },
  { path: membersPath, label: 'Thành viên', icon: Users },
  { path: examplesPath, label: 'Câu mẫu', icon: BadgeCheck, adminOnly: true },
].map((entry) => ({ ...entry, section: 'Workspace' }))

/** Đường dẫn tới một trang của workspace; path rỗng là trang đầu. Mọi chỗ dựng link tới workspace đi qua hàm này. */
export function workspaceHref(slug: string, path: string): string {
  const base = workspaceRoute.replace(':ws', encodeURIComponent(slug))
  return path ? `${base}/${path}` : base
}

/** Trang Tổng quan dữ liệu mở sẵn một dataset. */
export function overviewHref(slug: string, datasetId: string): string {
  return `${workspaceHref(slug, overviewPath)}?${new URLSearchParams({ dataset: datasetId }).toString()}`
}
