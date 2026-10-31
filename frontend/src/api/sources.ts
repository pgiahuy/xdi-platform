// SPDX-License-Identifier: Apache-2.0
/* api/sources.ts - hook cho nguồn database của workspace (backend/src/api/sources.rs): liệt kê, thêm, thử kết nối, liệt
kê mọi bảng đọc được (kèm số dòng, dung lượng, khóa ngoại), chụp bảng vào dataset của database (#247), xóa. Chỉ admin gọi
được; member nhận 403. Mật khẩu chỉ đi một chiều lúc thêm nguồn, backend không bao giờ trả lại. */

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { catalogKey } from './catalog'
import { apiDelete, apiGet, apiPost } from './client'
import { invalidateTables } from './datasets'
import type { components } from './schema'

type Schemas = components['schemas']
export type Source = Schemas['SourceResponse']
export type SourceTable = Schemas['SourceTableInfo']
export type CreateSourceRequest = Schemas['CreateSourceRequest']
export type SnapshotRequest = Schemas['SnapshotRequest']
export type SnapshotResponse = Schemas['SnapshotResponse']
export type TestSourceResponse = Schemas['TestSourceResponse']

function sourcesPath(ws: string): string {
  return `/workspaces/${encodeURIComponent(ws)}/sources`
}

function sourcesKey(ws: string) {
  return ['workspaces', ws, 'sources']
}

/** Chỉ gọi khi người xem là admin (enabled): member gọi chỉ nhận 403. */
export function useSources(ws: string, enabled = true) {
  return useQuery({
    queryKey: sourcesKey(ws),
    queryFn: () => apiGet<Source[]>(sourcesPath(ws)),
    enabled,
  })
}

/** Backend thử kết nối trước khi lưu, nên lỗi mật khẩu hay máy chủ hiện ngay ở bước này. */
export function useCreateSource(ws: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (body: CreateSourceRequest) => apiPost<Source>(sourcesPath(ws), body),
    // Nguồn mới là một dataset chưa có bảng trong catalog (api/catalog.ts).
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: sourcesKey(ws) })
      void queryClient.invalidateQueries({ queryKey: catalogKey(ws) })
    },
  })
}

/** Thử kết nối lại nguồn đã lưu; trả số bảng và view tài khoản đọc được. */
export function useTestSource(ws: string) {
  return useMutation({
    mutationFn: (sourceId: string) =>
      apiPost<TestSourceResponse>(`${sourcesPath(ws)}/${encodeURIComponent(sourceId)}/test`),
  })
}

/** Bảng mỗi lần chọn nguồn đọc lại từ database ngoài; không giữ lâu trong bộ đệm vì bảng bên đó có thể đổi. */
export function useSourceTables(ws: string, sourceId: string | undefined) {
  return useQuery({
    queryKey: [...sourcesKey(ws), sourceId, 'tables'],
    queryFn: () => apiGet<SourceTable[]>(`${sourcesPath(ws)}/${encodeURIComponent(sourceId ?? '')}/tables`),
    enabled: sourceId !== undefined,
    staleTime: 0,
    retry: false,
  })
}

/** Chụp bảng của nguồn vào dataset của database: thân rỗng là mọi bảng đọc được, `tables` là các bảng được tick hay
 * một bảng chụp riêng. Xong thì tải lại danh sách dataset và danh sách bảng của nguồn (trạng thái từng bảng đổi). */
export function useSnapshot(ws: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ sourceId, ...body }: SnapshotRequest & { sourceId: string }) =>
      apiPost<SnapshotResponse>(`${sourcesPath(ws)}/${encodeURIComponent(sourceId)}/snapshot`, body),
    onSettled: () => {
      invalidateTables(queryClient, ws)
      void queryClient.invalidateQueries({ queryKey: sourcesKey(ws) })
    },
  })
}

export function useDeleteSource(ws: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (sourceId: string) => apiDelete<void>(`${sourcesPath(ws)}/${encodeURIComponent(sourceId)}`),
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: sourcesKey(ws) })
      void queryClient.invalidateQueries({ queryKey: catalogKey(ws) })
    },
  })
}
