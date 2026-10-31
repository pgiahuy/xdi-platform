// SPDX-License-Identifier: Apache-2.0
/* api/sources.ts - hook cho nguồn database của workspace (backend/src/api/sources.rs): liệt kê, thêm, thử kết nối, liệt
kê bảng và bảng liên quan theo khóa ngoại, chụp một nhóm bảng thành dataset (mỗi bảng một dataset), xóa. Chỉ admin gọi
được; member nhận 403. Mật khẩu chỉ đi một chiều lúc thêm nguồn, backend không bao giờ trả lại. */

import { useMutation, useQueries, useQuery, useQueryClient } from '@tanstack/react-query'
import { apiDelete, apiGet, apiPost } from './client'
import type { components } from './schema'

type Schemas = components['schemas']
export type Source = Schemas['SourceResponse']
export type SourceTable = Schemas['SourceTableInfo']
export type CreateSourceRequest = Schemas['CreateSourceRequest']
export type SnapshotRequest = Schemas['SnapshotRequest']
export type SnapshotResponse = Schemas['SnapshotResponse']
export type TestSourceResponse = Schemas['TestSourceResponse']
export type RelationsResponse = Schemas['RelationsResponse']
export type RelatedTable = Schemas['RelatedTable']

function sourcesPath(ws: string): string {
  return `/workspaces/${encodeURIComponent(ws)}/sources`
}

function sourcesKey(ws: string) {
  return ['workspaces', ws, 'sources']
}

/** Chỉ gọi khi người xem là admin (trang chỉ dựng khung nguồn cho admin): member gọi chỉ nhận 403. */
export function useSources(ws: string) {
  return useQuery({
    queryKey: sourcesKey(ws),
    queryFn: () => apiGet<Source[]>(sourcesPath(ws)),
  })
}

/** Backend thử kết nối trước khi lưu, nên lỗi mật khẩu hay máy chủ hiện ngay ở bước này. */
export function useCreateSource(ws: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (body: CreateSourceRequest) => apiPost<Source>(sourcesPath(ws), body),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: sourcesKey(ws) }),
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

/** Bảng liên quan quanh từng bảng trong `tables` (bảng bắt đầu và các bảng người dùng mở thêm một bước), mỗi bảng một
 * lần gọi, đọc lại từ database ngoài như danh sách bảng. Backend nhận POST (thân là tên bảng) nhưng không đổi gì, nên
 * dùng query. Kết quả theo đúng thứ tự của `tables`. */
export function useSourceRelations(ws: string, sourceId: string, tables: string[]) {
  return useQueries({
    queries: tables.map((table) => ({
      queryKey: [...sourcesKey(ws), sourceId, 'relations', table],
      queryFn: () =>
        apiPost<RelationsResponse>(`${sourcesPath(ws)}/${encodeURIComponent(sourceId)}/relations`, { table }),
      staleTime: 0,
      retry: false,
    })),
  })
}

/** Chụp bảng (kèm các bảng chọn thêm) thành dataset, hoặc chụp lại cả nhóm của một dataset (dataset_id). Xong thì tải
 * lại danh sách dataset. */
export function useSnapshot(ws: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ sourceId, ...body }: SnapshotRequest & { sourceId: string }) =>
      apiPost<SnapshotResponse>(`${sourcesPath(ws)}/${encodeURIComponent(sourceId)}/snapshot`, body),
    onSettled: () => queryClient.invalidateQueries({ queryKey: ['workspaces', ws, 'datasets'] }),
  })
}

export function useDeleteSource(ws: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (sourceId: string) => apiDelete<void>(`${sourcesPath(ws)}/${encodeURIComponent(sourceId)}`),
    onSettled: () => queryClient.invalidateQueries({ queryKey: sourcesKey(ws) }),
  })
}
