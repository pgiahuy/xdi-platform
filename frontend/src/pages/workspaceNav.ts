// SPDX-License-Identifier: Apache-2.0
/* pages/workspaceNav.ts - đường dẫn của trang trong một workspace và các mục điều hướng, theo thứ tự trên thanh bên.
path khớp với route con của workspaceRoute trong router.tsx; '' là trang đầu của workspace. Thêm trang là thêm route ở
router.tsx và một dòng ở workspaceNav. Trang quản trị hệ thống không thuộc workspace nào, đường dẫn ở globalNav.ts.
Trang hỏi ('' là trang đầu) không có mục riêng: vào workspace là mở nó, và nút "Lịch sử trò chuyện" đứng đầu thanh bên mở
một cuộc cũ hay cuộc mới trên trang đó. */

import type { LucideIcon } from 'lucide-react'
import { BadgeCheck, BookOpen, ChartColumn, Database, FileText, Lightbulb, Users } from 'lucide-react'

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

/** Trang Khái niệm ngành: gói ngành của workspace, admin sửa khái niệm. */
export const domainsPath = 'domains'

/** Trang Tổng quan dữ liệu; dataset và bảng đang xem nằm trong đường dẫn (?dataset=...&table=...). */
export const overviewPath = 'overview'

/** Màn Câu mẫu (#212): admin duyệt câu trả lời được đánh giá Tốt. */
export const examplesPath = 'examples'

/** Trang Báo cáo: báo cáo của người dùng, tạo báo cáo trống hay tự động; một báo cáo ở reports/<id>. */
export const reportsPath = 'reports'

/** Màn Bài học (#228): admin xem nhật ký lỗi, viết và duyệt bài học. */
export const lessonsPath = 'lessons'

export const workspaceNav: WorkspaceNavEntry[] = [
  { path: overviewPath, label: 'Tổng quan dữ liệu', icon: ChartColumn },
  { path: reportsPath, label: 'Báo cáo', icon: FileText },
  { path: 'datasets', label: 'Dữ liệu', icon: Database },
  { path: domainsPath, label: 'Khái niệm ngành', icon: BookOpen },
  { path: membersPath, label: 'Thành viên', icon: Users },
  { path: examplesPath, label: 'Câu mẫu', icon: BadgeCheck, adminOnly: true },
  { path: lessonsPath, label: 'Bài học', icon: Lightbulb, adminOnly: true },
].map((entry) => ({ ...entry, section: 'Workspace' }))

/** Đường dẫn tới một trang của workspace; path rỗng là trang đầu. Mọi chỗ dựng link tới workspace đi qua hàm này. */
export function workspaceHref(slug: string, path: string): string {
  const base = workspaceRoute.replace(':ws', encodeURIComponent(slug))
  return path ? `${base}/${path}` : base
}

/** Trang Tổng quan dữ liệu mở sẵn một dataset, và một bảng của nó khi có tableId (#257). */
export function overviewHref(slug: string, datasetId: string, tableId?: string): string {
  const params = new URLSearchParams({ dataset: datasetId })
  if (tableId) params.set('table', tableId)
  return `${workspaceHref(slug, overviewPath)}?${params.toString()}`
}

/** Một báo cáo của workspace. */
export function reportHref(slug: string, reportId: string): string {
  return `${workspaceHref(slug, reportsPath)}/${encodeURIComponent(reportId)}`
}
