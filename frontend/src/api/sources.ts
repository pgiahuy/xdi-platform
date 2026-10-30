// SPDX-License-Identifier: Apache-2.0
/* api/sources.ts - hook cho nguồn database của workspace (backend/src/api/sources.rs): liệt kê, thêm, thử kết nối, liệt
kê bảng và quan hệ khóa ngoại quanh một bảng, chụp bảng (kèm bảng nối nhiều-một) thành dataset, xóa. Chỉ admin gọi được; member nhận 403. Mật khẩu chỉ đi một chiều lúc thêm nguồn,
backend không bao giờ trả lại. */

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { apiDelete, apiGet, apiPost } from './client'
import type { components } from './schema'

type Schemas = components['schemas']
export type Source = Schemas['SourceResponse']
export type SourceTable = Schemas['SourceTable']
export type CreateSourceRequest = Schemas['CreateSourceRequest']
export type SnapshotRequest = Schemas['SnapshotRequest']
export type UploadResult = Schemas['UploadResult']
export type TestSourceResponse = Schemas['TestSourceResponse']
export type RelationsResponse = Schemas['RelationsResponse']
export type SourceRelation = Schemas['SourceRelation']

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

/** Quan hệ khóa ngoại quanh bảng đang chọn, đọc lại từ database ngoài mỗi lần chọn bảng như danh sách bảng. Backend
 * nhận POST (thân là tên bảng) nhưng không đổi gì, nên dùng useQuery. */
export function useSourceRelations(ws: string, sourceId: string, table: string) {
  return useQuery({
    queryKey: [...sourcesKey(ws), sourceId, 'relations', table],
    queryFn: () =>
      apiPost<RelationsResponse>(`${sourcesPath(ws)}/${encodeURIComponent(sourceId)}/relations`, { table }),
    enabled: table !== '',
    staleTime: 0,
    retry: false,
  })
}

/** Chụp bảng thành dataset mới, hoặc chụp lại vào dataset đã có (dataset_id). Xong thì tải lại danh sách dataset. */
export function useSnapshot(ws: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ sourceId, ...body }: SnapshotRequest & { sourceId: string }) =>
      apiPost<UploadResult>(`${sourcesPath(ws)}/${encodeURIComponent(sourceId)}/snapshot`, body),
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
