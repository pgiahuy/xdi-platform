// SPDX-License-Identifier: Apache-2.0
/* api/dataRoles.ts - vai dữ liệu của workspace (#45, #167): vai, luật lọc dòng của từng vai, vai của từng thành viên.
Route ở backend/src/api/data_roles.rs; chỉ admin, trừ useMyDataRoles (ai cũng xem được vai của mình). Mọi lần ghi xong
thì tải lại danh sách vai, vì danh sách đó mang cả luật lẫn người mang vai. */

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { apiDelete, apiGet, apiPatch, apiPost, apiPut } from './client'
import type { components } from './schema'

type Schemas = components['schemas']
export type DataRole = Schemas['DataRoleResponse']
export type DataRoleRow = Schemas['DataRoleRow']
export type RolePolicy = Schemas['RolePolicy']
export type FilterColumn = Schemas['FilterColumnResponse']
export type SetPoliciesResponse = Schemas['SetPoliciesResponse']

function workspacePath(ws: string): string {
  return `/workspaces/${encodeURIComponent(ws)}`
}

function dataRolesKey(ws: string) {
  return ['workspaces', ws, 'data-roles'] as const
}

export function useDataRoles(ws: string, enabled: boolean) {
  return useQuery({
    queryKey: dataRolesKey(ws),
    queryFn: () => apiGet<DataRole[]>(`${workspacePath(ws)}/data-roles`),
    enabled,
  })
}

/** Vai của người đang đăng nhập, chỉ tên và mô tả. */
export function useMyDataRoles(ws: string) {
  return useQuery({
    queryKey: [...dataRolesKey(ws), 'mine'],
    queryFn: () => apiGet<DataRoleRow[]>(`${workspacePath(ws)}/data-roles/mine`),
  })
}

/** Cột chiều của một bảng kèm giá trị đã biết, cho ô chọn của luật. tableId rỗng thì chưa tải. */
export function useFilterColumns(ws: string, tableId: string) {
  return useQuery({
    queryKey: ['workspaces', ws, 'datasets', tableId, 'filter-columns'],
    queryFn: () =>
      apiGet<FilterColumn[]>(`${workspacePath(ws)}/datasets/${encodeURIComponent(tableId)}/filter-columns`),
    enabled: tableId !== '',
  })
}

function useRefresh(ws: string) {
  const queryClient = useQueryClient()
  return () => queryClient.invalidateQueries({ queryKey: dataRolesKey(ws) })
}

export function useCreateDataRole(ws: string) {
  const refresh = useRefresh(ws)
  return useMutation({
    mutationFn: (request: Schemas['DataRoleRequest']) =>
      apiPost<DataRoleRow>(`${workspacePath(ws)}/data-roles`, request),
    onSuccess: refresh,
  })
}

export function useUpdateDataRole(ws: string) {
  const refresh = useRefresh(ws)
  return useMutation({
    mutationFn: ({ id, ...request }: Schemas['DataRoleRequest'] & { id: string }) =>
      apiPatch<DataRoleRow>(`${workspacePath(ws)}/data-roles/${encodeURIComponent(id)}`, request),
    onSuccess: refresh,
  })
}

export function useDeleteDataRole(ws: string) {
  const refresh = useRefresh(ws)
  return useMutation({
    mutationFn: (id: string) => apiDelete<void>(`${workspacePath(ws)}/data-roles/${encodeURIComponent(id)}`),
    onSuccess: refresh,
  })
}

/** Thay cả bộ luật của một vai. Response có lời báo (ví dụ giá trị không có trong hồ sơ cột); luật vẫn được lưu. */
export function useSetDataRolePolicies(ws: string) {
  const refresh = useRefresh(ws)
  return useMutation({
    mutationFn: ({ id, policies }: { id: string; policies: RolePolicy[] }) =>
      apiPut<SetPoliciesResponse>(`${workspacePath(ws)}/data-roles/${encodeURIComponent(id)}/policies`, {
        policies,
      } satisfies Schemas['SetPoliciesRequest']),
    onSuccess: refresh,
  })
}

/** Thay cả bộ vai của một thành viên; danh sách rỗng là gỡ hết. */
export function useSetMemberDataRoles(ws: string) {
  const refresh = useRefresh(ws)
  return useMutation({
    mutationFn: ({ userId, roleIds }: { userId: string; roleIds: string[] }) =>
      apiPut<void>(`${workspacePath(ws)}/members/${encodeURIComponent(userId)}/data-roles`, {
        role_ids: roleIds,
      } satisfies Schemas['MemberDataRolesRequest']),
    onSuccess: refresh,
  })
}
