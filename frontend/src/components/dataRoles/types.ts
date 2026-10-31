// SPDX-License-Identifier: Apache-2.0
/* components/dataRoles/types.ts - kiểu dữ liệu của màn Vai dữ liệu (#45) mà component nhận qua props;
pages/DataRolesPage.tsx dựng chúng từ api/dataRoles.ts, api/catalog.ts và api/workspaces.ts. */

/** Một dataset (database hay file) và các bảng của nó, để chọn bảng cho luật. */
export type DatasetChoice = {
  id: string
  name: string
  tables: { id: string; name: string }[]
}

/** Một cột dùng được cho luật: cột chiều, kèm giá trị đã biết (có thể rỗng). */
export type ColumnChoice = {
  name: string
  label: string | null
  values: string[]
}

/** Một luật: người mang vai chỉ thấy dòng của bảng tableId có cột column nằm trong values. */
export type RuleDraft = {
  tableId: string
  column: string
  values: string[]
}

export type RoleItem = {
  id: string
  name: string
  description: string | null
  rules: RuleDraft[]
  memberIds: string[]
}

export type MemberChoice = {
  userId: string
  name: string
  isAdmin: boolean
}

/** Tên dễ đọc của một bảng: "dataset / bảng"; bảng đã xóa thì nói vậy. */
export function tableLabel(datasets: DatasetChoice[], tableId: string): string {
  for (const dataset of datasets) {
    const table = dataset.tables.find((t) => t.id === tableId)
    if (table) return `${dataset.name} / ${table.name}`
  }
  return 'Bảng đã xóa'
}
