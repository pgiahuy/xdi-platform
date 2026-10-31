// SPDX-License-Identifier: Apache-2.0
/* components/datasets/types.ts - kiểu dữ liệu các component của trang Dữ liệu nhận từ trang.
Trang dựng các kiểu này từ dữ liệu backend (src/api/datasets.ts), để component không phụ thuộc dạng JSON của API. */

/** Một bảng của dataset trên thẻ dataset (#257). */
export type TableRow = {
  id: string
  name: string
  /** Schema trong database nguồn; null với file. */
  schema: string | null
  /** Số dòng của phiên bản hiện hành; null khi chưa có dữ liệu. */
  rowCount: number | null
  /** Mô tả bảng ghi trong database nguồn (COMMENT ON TABLE); null khi không có. */
  description: string | null
}

/** Một quan hệ khóa ngoại giữa hai bảng của dataset: bảng from giữ khóa, trỏ tới bảng to. */
export type RelationRow = {
  key: string
  from: string
  fromColumns: string[]
  to: string
  toColumns: string[]
}

/** Một dataset trên trang Dữ liệu: một database đã kết nối hay một file. */
export type DatasetCardView = {
  id: string
  name: string
  kind: 'database' | 'file'
  /** máy:cổng/database của database; chỉ admin có (backend chỉ trả nguồn cho admin). */
  connection?: string
  /** Lần chụp hay nạp gần nhất; null khi chưa bảng nào có dữ liệu. */
  updatedAt: string | null
  tables: TableRow[]
  /** Bảng của database chưa chụp được dòng nào (quá trần dòng hay dung lượng); rỗng với file. */
  metadataOnly: MetadataTableRow[]
  relations: RelationRow[]
}

/** Một bảng nguồn trên màn kết nối (#247): admin tick để chụp. */
export type SourceTableRow = {
  /** schema.bảng, đúng chuỗi gửi khi chụp. */
  table: string
  name: string
  isView: boolean
  ticked: boolean
  /** Số dòng Postgres ước lượng; null khi chưa ANALYZE, hay với view. */
  estimatedRows: number | null
  sizeBytes: number | null
  columnCount: number
  comment: string | null
  /** Ước lượng đã quá SOURCE_MAX_ROWS: chụp thì chỉ lưu cấu trúc. */
  overRowLimit: boolean
  /** Nguồn đang giữ bảng: đã chụp, chỉ có cấu trúc (kèm lý do), hay chưa. */
  state: 'snapshotted' | 'metadata_only' | null
  reason: string | null
  /** Bỏ tick một bảng danh mục mà bảng đang tick trỏ tới: câu báo hệ quả; null khi không có gì để báo. */
  warning: string | null
}

export type SourceTableGroup = {
  schema: string
  rows: SourceTableRow[]
}

/** Một bảng chỉ có cấu trúc (cột, kiểu, khóa, mô tả), chưa có dòng nào, trên thẻ database. */
export type MetadataTableRow = {
  /** schema.bảng, đúng chuỗi gửi khi chụp riêng bảng này. */
  table: string
  name: string
  schema: string | null
  /** Vì sao chưa chụp, kèm biến cấu hình cần đổi (backend viết). */
  reason: string
  rowCount: number | null
  columnCount: number
  description: string | null
}

/** Kết quả chụp của một bảng. */
export type SnapshotTableResult = {
  datasetName: string
  /** False: nội dung trùng phiên bản đã có, không tạo gì mới. */
  created: boolean
  version: number
  rowCount: number | null
}

/** Kết quả một lần chụp: bảng chép được, quan hệ đã ghi, bảng chỉ lưu cấu trúc, bảng không còn đọc được. */
export type SnapshotSummary = {
  tables: SnapshotTableResult[]
  relationCount: number
  metadataOnly: { table: string; reason: string }[]
  unreadable: string[]
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
