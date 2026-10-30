// SPDX-License-Identifier: Apache-2.0
/* components/datasets/types.ts - kiểu dữ liệu các component của trang Dữ liệu nhận từ trang.
Trang dựng các kiểu này từ dữ liệu backend (src/api/datasets.ts), để component không phụ thuộc dạng JSON của API. */

export type DatasetRow = {
  id: string
  name: string
  /** Tên bảng dùng trong SQL. */
  tableName: string
  createdAt: string
  /** Dataset chụp từ bảng database: bảng nguồn và lần chụp gần nhất. Không có với dataset nạp từ file. */
  snapshot?: { table: string; at: string | null }
  /** Dataset khác nối với dataset này theo khóa ngoại của database nguồn, mỗi dataset một lần (#165). */
  related: { id: string; name: string }[]
}

/** Một bảng nối với một bảng của nhóm theo một khóa ngoại, chụp kèm được thành dataset riêng. */
export type RelationOption = {
  /** Khóa React: một bảng có thể nối theo hai khóa ngoại. */
  key: string
  /** schema.bảng, đúng chuỗi gửi khi chụp. */
  table: string
  /** Tên hiện ra: bỏ schema khi cùng schema với bảng bắt đầu. */
  label: string
  /** Cột khóa ngoại, ở bảng giữ khóa. */
  columns: string[]
  addable: boolean
  reason: string | null
  /** Số cột đọc được của bảng. */
  columnCount: number
}

/** Bảng liên quan quanh một bảng, tách theo chiều của khóa ngoại. */
export type RelationNode = {
  table: string
  label: string
  /** Bảng này trỏ tới (khóa ngoại của nó, nhiều-một). */
  parents: RelationOption[]
  /** Trỏ tới bảng này (khóa ngoại của bảng kia, một-nhiều). */
  children: RelationOption[]
}

/** Đọc bảng liên quan của một bảng: đang đọc, lỗi, hoặc xong. */
export type RelationNodeState =
  | { kind: 'loading' }
  | { kind: 'error'; message: string }
  | { kind: 'ready'; node: RelationNode }

/** Kết quả chụp của một bảng. */
export type SnapshotTableResult = {
  datasetName: string
  /** False: nội dung trùng phiên bản đã có, không tạo gì mới. */
  created: boolean
  version: number
  rowCount: number | null
}

export type SheetOption = {
  name: string
  hidden: boolean
  /** Rỗng: sheet không có dữ liệu, không nạp được. */
  columns: string[]
  /** Số dòng ước lượng; 0 là file không ghi. */
  approxRows: number
}

export type ColumnInfo = {
  name: string
  dataType: string
}

/** Trạng thái một lần gửi lên backend. Một lần chọn file Excel nhiều nhóm sheet thành nhiều lần gửi. */
export type UploadStatus =
  | { kind: 'queued' }
  | { kind: 'sending'; progress: number }
  | { kind: 'processing' }
  | { kind: 'created'; datasetName: string; version: number; rowCount: number | null; columns: ColumnInfo[] }
  | { kind: 'duplicate'; datasetName: string; version: number }
  | { kind: 'error'; message: string }

export type UploadEntry = {
  id: number
  /** Gửi gì: tên file, hoặc tên file kèm các sheet. */
  label: string
  status: UploadStatus
}
