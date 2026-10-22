// SPDX-License-Identifier: Apache-2.0
/* api/datasets.ts - hook cho dataset của một workspace: liệt kê, nạp file, liệt kê sheet của file Excel
(backend/src/api/datasets.rs). Kiểu dữ liệu lấy từ schema.ts, sinh từ openapi.json; tên ở đây chỉ là bí danh để trang
import gọn. */

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { apiGet, apiUpload, type UploadOptions } from './client'
import type { components } from './schema'

type Schemas = components['schemas']
export type Dataset = Schemas['Dataset']
export type DatasetVersion = Schemas['DatasetVersion']
export type Column = Schemas['Column']
/** Kết quả nạp; created là false khi bộ file trùng một phiên bản đã có, backend không tạo gì mới. */
export type UploadResult = Schemas['UploadResult']
/** Một sheet của file Excel, để người dùng chọn trước khi nạp. */
export type SheetSummary = Schemas['SheetSummary']

export type UploadRequest = {
  files: File[]
  /** Tên dataset mới; bỏ trống thì backend lấy tên file đầu. */
  name?: string
  /** Có thì thêm phiên bản cho dataset này thay vì tạo dataset mới. */
  datasetId?: string
  /** Sheet cần nạp của file Excel. */
  sheets?: string[]
} & UploadOptions

function datasetsPath(ws: string): string {
  return `/workspaces/${encodeURIComponent(ws)}/datasets`
}

function datasetsKey(ws: string) {
  return ['workspaces', ws, 'datasets']
}

export function useDatasets(ws: string) {
  return useQuery({ queryKey: datasetsKey(ws), queryFn: () => apiGet<Dataset[]>(datasetsPath(ws)) })
}

/** Nạp file thành một dataset mới hoặc một phiên bản mới. Backend trả 201 khi tạo, 200 với created false khi trùng. */
export function useUploadDataset(ws: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ files, name, datasetId, sheets = [], ...options }: UploadRequest) => {
      const form = new FormData()
      if (name) form.append('name', name)
      if (datasetId) form.append('dataset_id', datasetId)
      for (const sheet of sheets) form.append('sheet', sheet)
      for (const file of files) form.append('file', file)
      return apiUpload<UploadResult>(datasetsPath(ws), form, options)
    },
    // Tải lại danh sách cả khi lỗi: một lần nạp lỗi giữa chừng mạng vẫn có thể đã tạo dataset ở backend.
    onSettled: () => queryClient.invalidateQueries({ queryKey: datasetsKey(ws) }),
  })
}

/** Gửi một file Excel để lấy danh sách sheet; backend không lưu file. */
export function useExcelSheets(ws: string) {
  return useMutation({
    mutationFn: ({ file, ...options }: { file: File } & UploadOptions) => {
      const form = new FormData()
      form.append('file', file)
      return apiUpload<SheetSummary[]>(`${datasetsPath(ws)}/sheets`, form, options)
    },
  })
}
