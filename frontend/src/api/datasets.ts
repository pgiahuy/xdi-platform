// SPDX-License-Identifier: Apache-2.0
/* api/datasets.ts - hook cho dataset của một workspace: liệt kê, nạp file, liệt kê sheet của file Excel
(backend/src/api/datasets/). Kiểu dữ liệu lấy từ schema.ts, sinh từ openapi.json; tên ở đây chỉ là bí danh để trang
import gọn. */

import { useRef } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { apiDelete, apiGet, apiUpload, type UploadOptions } from './client'
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

// Lớp ngữ nghĩa chạy nền sau khi nạp (backend/src/service/semantic/mod.rs), mất bao lâu tùy mô hình và hàng đợi. Hỏi lại
// mỗi 10 giây để câu gợi ý hiện mà không phải tải lại trang; dừng sau 5 phút vì lớp đó lỗi thì phiên bản giữ structure
// tới lần khởi động sau, hỏi mãi chỉ tốn request.
const PROFILE_POLL_MS = 10_000
const PROFILE_POLL_LIMIT_MS = 5 * 60_000

/** Phiên bản hiện hành chưa xong hồ sơ: pending (chưa có lớp cấu trúc) hoặc structure (chưa có lớp ngữ nghĩa). Phiên
 * bản structure mà lớp ngữ nghĩa đã thử với mô hình đang chạy và không dùng được (semantic_unavailable) thì không còn
 * gì để chờ: tính là đang lập thì trang hỏi báo "đang lập" mãi và hỏi lại vô ích (#140). */
export function isProfiling(dataset: Dataset): boolean {
  if (dataset.current_profile_status === 'pending') return true
  return dataset.current_profile_status === 'structure' && !dataset.semantic_unavailable
}

type DatasetsOptions = {
  /** Tự tải lại danh sách khi còn dataset đang lập hồ sơ (trang hỏi cần câu gợi ý ngay khi có). */
  watchProfiles?: boolean
}

export function useDatasets(ws: string, { watchProfiles = false }: DatasetsOptions = {}) {
  // Mốc tính 5 phút gắn với tập phiên bản đang lập hồ sơ: nạp thêm một phiên bản thì tập đổi, đếm lại từ đầu; nếu
  // chỉ đếm từ lúc mở trang thì dataset nạp sau 5 phút không bao giờ được hỏi lại.
  const watch = useRef({ versions: '', since: 0 })
  return useQuery({
    queryKey: datasetsKey(ws),
    queryFn: () => apiGet<Dataset[]>(datasetsPath(ws)),
    // TanStack Query gọi hàm này sau mỗi lần tải, nên khoảng hỏi lại tự tắt khi không còn gì đang lập hồ sơ.
    refetchInterval: (query) => {
      if (!watchProfiles) return false
      const versions = (query.state.data ?? [])
        .filter(isProfiling)
        .map((d) => d.current_version_id)
        .join(',')
      if (versions === '') return false
      if (versions !== watch.current.versions) watch.current = { versions, since: Date.now() }
      return Date.now() - watch.current.since < PROFILE_POLL_LIMIT_MS ? PROFILE_POLL_MS : false
    },
  })
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

/** Xóa hẳn một dataset cùng mọi phiên bản, file và lịch sử câu hỏi của nó (chỉ admin); không khôi phục được. */
export function useDeleteDataset(ws: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (datasetId: string) =>
      apiDelete<void>(`${datasetsPath(ws)}/${encodeURIComponent(datasetId)}`),
    onSettled: () => queryClient.invalidateQueries({ queryKey: datasetsKey(ws) }),
  })
}
