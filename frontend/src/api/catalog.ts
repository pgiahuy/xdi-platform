// SPDX-License-Identifier: Apache-2.0
/* api/catalog.ts - dataset của workspace (#257): một database đã kết nối hay một file, bảng nằm bên trong. Route ở
backend/src/api/catalog.rs, hợp đồng ở backend/src/service/catalog/contract.rs. Các route của bảng (tổng quan, chỉ số,
truy dòng nguồn, mở một cuộc trò chuyện) vẫn nhận id bảng (DatasetTable.id), nên trang lấy id bảng từ đây.
Hỏi trên dataset ở api/ask.ts, cuộc trò chuyện của dataset ở api/conversations.ts. */

import { useRef } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { apiGet, apiPost } from './client'
import type { components } from './schema'

type Schemas = components['schemas']
export type CatalogDataset = Schemas['DatasetSummary']
export type CatalogTable = Schemas['DatasetTable']
export type CatalogRelation = Schemas['DatasetRelation']
export type CatalogKind = Schemas['DatasetKind']
export type CatalogSnapshotResponse = Schemas['SnapshotResponse']

export function catalogPath(ws: string, id?: string): string {
  const base = `/workspaces/${encodeURIComponent(ws)}/catalog`
  return id === undefined ? base : `${base}/${encodeURIComponent(id)}`
}

export function catalogKey(ws: string) {
  return ['workspaces', ws, 'catalog'] as const
}

// Cùng quy tắc với useDatasets (api/datasets.ts): lớp ngữ nghĩa chạy nền sau khi nạp hay chụp, hỏi lại mỗi 10 giây để
// câu gợi ý hiện mà không phải tải lại trang, dừng sau 5 phút.
const PROFILE_POLL_MS = 10_000
const PROFILE_POLL_LIMIT_MS = 5 * 60_000

/** Bảng chưa xong hồ sơ, như isProfiling của api/datasets.ts: pending, hay structure khi lớp ngữ nghĩa còn chờ. */
export function isTableProfiling(table: CatalogTable): boolean {
  if (table.current_profile_status === 'pending') return true
  return table.current_profile_status === 'structure' && !table.semantic_unavailable
}

type CatalogOptions = {
  /** Tự tải lại khi còn bảng đang lập hồ sơ (trang hỏi cần câu gợi ý ngay khi có). */
  watchProfiles?: boolean
}

/** Mọi dataset của workspace, cũ trước, mỗi dataset kèm bảng và quan hệ. Member cũng gọi được. */
export function useCatalog(ws: string, { watchProfiles = false }: CatalogOptions = {}) {
  // Mốc 5 phút gắn với tập phiên bản đang lập hồ sơ, như useDatasets: chụp thêm thì đếm lại từ đầu.
  const watch = useRef({ versions: '', since: 0 })
  return useQuery({
    queryKey: catalogKey(ws),
    queryFn: () => apiGet<CatalogDataset[]>(catalogPath(ws)),
    refetchInterval: (query) => {
      if (!watchProfiles) return false
      const versions = (query.state.data ?? [])
        .flatMap((d) => d.tables)
        .filter(isTableProfiling)
        .map((t) => t.current_version_id)
        .join(',')
      if (versions === '') return false
      if (versions !== watch.current.versions) watch.current = { versions, since: Date.now() }
      return Date.now() - watch.current.since < PROFILE_POLL_LIMIT_MS ? PROFILE_POLL_MS : false
    },
  })
}

/** Chụp lại mọi bảng của một dataset là database trong một giao dịch (chỉ admin). Xong thì tải lại cả danh sách
 * dataset lẫn danh sách bảng cũ (api/datasets.ts), vì các màn còn theo bảng đọc danh sách đó. */
export function useSnapshotCatalog(ws: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (id: string) => apiPost<CatalogSnapshotResponse>(`${catalogPath(ws, id)}/snapshot`),
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: catalogKey(ws) })
      void queryClient.invalidateQueries({ queryKey: ['workspaces', ws, 'datasets'] })
    },
  })
}
