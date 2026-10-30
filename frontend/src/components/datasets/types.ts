// SPDX-License-Identifier: Apache-2.0
/* components/datasets/types.ts - kiểu dữ liệu các component của trang Dữ liệu nhận từ trang.
Trang dựng các kiểu này từ dữ liệu backend (src/api/datasets.ts), để component không phụ thuộc dạng JSON của API. */

export type DatasetRow = {
  id: string
  name: string
  /** Tên bảng dùng trong SQL. */
  tableName: string
  createdAt: string
  /** Dataset chụp từ bảng database: bảng nguồn, lần chụp gần nhất và các bảng đã nối. Không có với dataset nạp từ file. */
  snapshot?: { table: string; at: string | null; joins: ModelJoinRow[] }
}

/** Một bảng nối nhiều-một của phiên bản hiện hành. */
export type ModelJoinRow = {
  fromTable: string
  fromColumns: string[]
  toTable: string
}

/** Một khóa ngoại quanh bảng đang chọn để chụp. */
export type RelationOption = {
  id: string
  depth: number
  /** Id quan hệ bước 1 mà quan hệ này đi tiếp; null ở bước 1. */
  parent: string | null
  fromTable: string
  fromColumns: string[]
  toTable: string
  joinable: boolean
  reason: string | null
  /** Số cột lần nối thêm vào dataset. */
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
