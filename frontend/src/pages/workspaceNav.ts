// SPDX-License-Identifier: Apache-2.0
/* pages/workspaceNav.ts - các mục điều hướng trong một workspace, theo thứ tự hiện trên thanh bên.
path khớp với route con của /w/:ws trong router.tsx; '' là trang đầu của workspace. Thêm trang là thêm route ở
router.tsx và một dòng ở đây. */

export type WorkspaceNavEntry = {
  path: string
  label: string
}

export const workspaceNav: WorkspaceNavEntry[] = [
  { path: '', label: 'Hỏi dữ liệu' },
  { path: 'datasets', label: 'Dữ liệu' },
]
