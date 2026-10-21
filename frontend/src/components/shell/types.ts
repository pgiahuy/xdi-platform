// SPDX-License-Identifier: Apache-2.0
/* components/shell/types.ts - kiểu dữ liệu khung giao diện nhận từ trang.
Component không dùng router (AGENTS.md mục 3), nên trang truyền vào sẵn đường dẫn đã tính và một component Link
bọc router. Đổi router sau này chỉ cần đổi component Link đó, không sửa khung. */

import type { ComponentProps, ComponentType } from 'react'

/** Link do trang cung cấp: nhận mọi thuộc tính của thẻ a, đường dẫn đặt ở href. */
export type LinkComponent = ComponentType<ComponentProps<'a'> & { href: string }>

export type NavItem = {
  href: string
  label: string
  active: boolean
}

/** Một workspace trong danh sách đổi workspace. href đã trỏ sẵn tới đúng mục đang xem của workspace đó. */
export type WorkspaceOption = {
  slug: string
  name: string
  role: string
  href: string
}

export type ServerInfo = {
  name: string
  version: string
  llm: string
}
