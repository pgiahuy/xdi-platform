// SPDX-License-Identifier: Apache-2.0
/* pages/datasetUploadPlan.ts - các hàm thuần của luồng nạp file (DatasetUpload.tsx): từ lựa chọn của người dùng ra
danh sách lần gửi, lý do chưa nạp được, tên dataset cho từng nhóm sheet, và trạng thái hiện ra từ kết quả backend. */

import type { Dataset, UploadRequest, UploadResult } from '@/api/datasets'
import type { SheetGroup } from '@/components/datasets/sheetGroups'
import type { SheetOption, UploadStatus } from '@/components/datasets/types'

/** Đuôi file backend nhận (FileFormat trong backend/src/engine/analytics.rs). Thêm định dạng thì sửa cả hai nơi. */
export const ACCEPT = '.csv,.parquet,.xlsx'
const EXCEL = /\.xlsx$/i

/** Độ dài tối đa của tên dataset, đếm theo ký tự (dataset_name trong backend/src/service/ingest.rs). Tên gửi lên dài
hơn thì backend trả 400; đổi ở backend thì đổi cả ở đây. */
export const MAX_DATASET_NAME = 100

export type SheetsState =
  | { status: 'idle' }
  | { status: 'loading'; progress: number }
  | { status: 'ready'; sheets: SheetOption[] }
  | { status: 'error'; message: string }

export type PlannedRequest = {
  /** Gửi gì, hiện cho người dùng; cũng là khóa để biết lần gửi nào đã xong khi nạp lại sau lỗi. */
  label: string
  request: Omit<UploadRequest, 'onProgress' | 'signal'>
}

/** File Excel duy nhất được chọn, hoặc undefined. Backend chỉ nhận một file Excel mỗi lần nạp. */
export function singleExcel(files: File[]): File | undefined {
  return files.length === 1 && EXCEL.test(files[0].name) ? files[0] : undefined
}

/** Mỗi phần tử là một lần gửi. CSV, Parquet: một lần cho mọi file. Excel: một lần cho mỗi nhóm sheet cùng cột. */
export function buildRequests(
  files: File[],
  groups: SheetGroup[],
  name: string,
  target: Dataset | undefined,
): PlannedRequest[] {
  const datasetId = target?.id
  // Thêm phiên bản thì backend giữ tên dataset có sẵn, không gửi tên.
  const ownName = target ? '' : name
  const excelFile = singleExcel(files)
  if (!excelFile) {
    return [{ label: files.map((f) => f.name).join(', '), request: { files, name: ownName || undefined, datasetId } }]
  }
  return groups.map((group) => ({
    label: `${excelFile.name}: ${group.sheets.join(', ')}`,
    request: {
      files: [excelFile],
      sheets: group.sheets,
      datasetId,
      // Nhiều nhóm thì nhiều dataset; cùng một tên thì danh sách có các dòng trùng tên, không biết dòng nào là sheet
      // nào. Thêm tên sheet đầu của nhóm để phân biệt.
      name: target || groups.length === 1 ? ownName || undefined : groupName(ownName || stem(excelFile.name), group),
    },
  }))
}

/** "<gốc> (<sheet đầu>)", cắt cho vừa MAX_DATASET_NAME. Đếm theo code point (Array.from), giống chars().count() của
backend; đếm theo length của chuỗi JavaScript thì chữ ngoài BMP tính thành hai. Phần nào dài thì cắt phần đó, cả hai
dài thì mỗi phần giữ khoảng một nửa. Gốc rỗng (file chỉ có đuôi) thì chỉ dùng tên sheet. */
export function groupName(base: string, group: SheetGroup): string {
  const sheet = Array.from(group.sheets[0])
  if (!base) return sheet.slice(0, MAX_DATASET_NAME).join('')
  const baseChars = Array.from(base)
  const room = MAX_DATASET_NAME - 3 // " (" và ")"
  const sheetLength = Math.min(sheet.length, Math.max(Math.floor(room / 2), room - baseChars.length))
  const baseLength = Math.min(baseChars.length, room - sheetLength)
  return `${baseChars.slice(0, baseLength).join('').trimEnd()} (${sheet.slice(0, sheetLength).join('')})`
}

/** Lý do chưa bấm Nạp được, hoặc undefined khi đã đủ. */
export function blocker(
  files: File[],
  sheets: SheetsState,
  groups: SheetGroup[],
  target: Dataset | undefined,
): string | undefined {
  if (files.length === 0) return 'Chọn file để nạp.'
  // Gửi đi backend cũng từ chối; chặn trước để khỏi phải gửi cả file mới biết.
  if (files.length > 1 && files.some((f) => EXCEL.test(f.name))) {
    return 'File Excel nạp riêng, mỗi lần một file.'
  }
  if (!singleExcel(files)) return undefined
  if (sheets.status === 'error') return 'Chọn lại file để đọc lại danh sách sheet.'
  if (sheets.status !== 'ready') return 'Chờ đọc xong danh sách sheet.'
  if (groups.length === 0) return 'Chọn ít nhất một sheet có dữ liệu.'
  if (target && groups.length > 1) {
    return 'Một phiên bản chỉ gộp được các sheet cùng cột. Bỏ chọn sheet khác cột, hoặc nạp chúng thành dataset mới.'
  }
  return undefined
}

export function resultStatus(result: UploadResult): UploadStatus {
  if (!result.created) {
    return { kind: 'duplicate', datasetName: result.dataset.name, version: result.version.version }
  }
  return {
    kind: 'created',
    datasetName: result.dataset.name,
    version: result.version.version,
    rowCount: result.version.row_count,
    columns: result.columns.map((c) => ({ name: c.name, dataType: c.data_type })),
  }
}

function stem(fileName: string): string {
  return fileName.replace(/\.[^.]*$/, '')
}
