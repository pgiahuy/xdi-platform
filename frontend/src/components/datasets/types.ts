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
}

/** Một bảng nối với bảng đang chọn theo một khóa ngoại, chụp kèm được thành dataset riêng. */
export type RelationOption = {
  /** schema.bảng sẽ chụp kèm. */
  table: string
  /** Bảng giữ khóa ngoại và cột khóa. */
  fromTable: string
  fromColumns: string[]
  addable: boolean
  reason: string | null
  /** Số cột đọc được của bảng. */
  columnCount: number
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
