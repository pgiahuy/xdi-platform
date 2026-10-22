// SPDX-License-Identifier: Apache-2.0
/* components/admin/types.ts - kiểu dữ liệu các component quản trị hệ thống nhận từ trang. */

import type { MemberItem } from '@/components/members/types'

export type AccountItem = {
  id: string
  name: string
  email: string
  isSystemAdmin: boolean
  isActive: boolean
}

export type WorkspaceItem = {
  id: string
  name: string
  slug: string
  members: MemberItem[]
}

/** Kết quả việc vừa làm trên một dòng (một tài khoản, một workspace), để hiện lời ngay tại dòng đó. seq đổi mỗi lần,
 * để component hiện lời gắn lại và nhận focus lại khi cùng một dòng có kết quả mới. */
export type RowMessage = {
  id: string
  seq: number
  kind: 'error' | 'success'
  text: string
}
