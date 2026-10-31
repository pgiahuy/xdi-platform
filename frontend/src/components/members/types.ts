// SPDX-License-Identifier: Apache-2.0
/* components/members/types.ts - kiểu dữ liệu các component thành viên nhận từ trang. Trang dựng từ dữ liệu backend;
component không biết API (AGENTS.md mục 3). */

export type MemberItem = {
  userId: string
  name: string
  email: string
  role: string
  /** Tên các vai dữ liệu người này mang; rỗng khi không có hay khi người xem không được biết. */
  dataRoles?: string[]
}
