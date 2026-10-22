// SPDX-License-Identifier: Apache-2.0
/* api/admin.ts - hook cho quản trị hệ thống: tài khoản và danh sách mọi workspace (backend/src/api/admin.rs).
Người không phải quản trị hệ thống gọi vào nhận 403 kèm lời từ backend. Kiểu dữ liệu lấy từ schema.ts, sinh từ
openapi.json; tên ở đây chỉ là bí danh để trang import gọn. */

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { apiGet, apiPost } from './client'
import { meKey, type User } from './auth'
import type { components } from './schema'

type Schemas = components['schemas']
export type UserSummary = Schemas['UserSummary']
export type CreateUserRequest = Schemas['CreateUserRequest']
export type WorkspaceOverview = Schemas['WorkspaceOverview']

const usersKey = ['admin', 'users']
const overviewKey = ['admin', 'workspaces']

/** enabled = false khi người mở trang không phải quản trị hệ thống: không gọi API chắc chắn trả 403. */
export function useUsers(enabled: boolean) {
  return useQuery({ queryKey: usersKey, queryFn: () => apiGet<UserSummary[]>('/admin/users'), enabled })
}

export function useCreateUser() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (request: CreateUserRequest) => apiPost<User>('/admin/users', request),
    // Mật khẩu ban đầu nằm trong variables của mutation; bỏ khỏi bộ đệm ngay khi trang không còn dùng.
    gcTime: 0,
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: usersKey }),
  })
}

/** Khóa (active = false) hoặc mở tài khoản. Khóa thì backend đăng xuất mọi phiên của người đó. */
export function useSetUserActive() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ userId, active }: { userId: string; active: boolean }) =>
      apiPost<void>(`/admin/users/${userId}/${active ? 'activate' : 'deactivate'}`),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: usersKey }),
  })
}

/** Sinh mật khẩu tạm và đăng xuất mọi phiên của người đó. Mật khẩu tạm chỉ có trong kết quả này, backend không lưu
 * dạng chữ; giao diện hiện một lần để quản trị chuyển cho người dùng. Trang hiện thẳng từ data và gọi reset() khi ẩn,
 * để mật khẩu được bỏ khỏi bộ đệm của TanStack Query. */
export function useResetPassword() {
  return useMutation({
    mutationFn: (userId: string) =>
      apiPost<Schemas['ResetPasswordResponse']>(`/admin/users/${userId}/reset-password`),
    // Mặc định TanStack giữ kết quả mutation 5 phút sau khi trang rời đi; 0 thì bỏ ngay.
    gcTime: 0,
  })
}

/** enabled như useUsers. */
export function useWorkspacesOverview(enabled: boolean) {
  return useQuery({ queryKey: overviewKey, queryFn: () => apiGet<WorkspaceOverview[]>('/admin/workspaces'), enabled })
}

export function useAssignWorkspaceAdmin() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ workspaceId, email }: { workspaceId: string; email: string }) =>
      apiPost<void>(`/admin/workspaces/${workspaceId}/admins`, { email } satisfies Schemas['AssignAdminRequest']),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: overviewKey })
      // Quản trị tự chỉ định mình thì workspace đó hiện ngay trong danh sách đổi workspace.
      void queryClient.invalidateQueries({ queryKey: meKey })
    },
  })
}
