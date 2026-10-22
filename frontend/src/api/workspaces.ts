// SPDX-License-Identifier: Apache-2.0
/* api/workspaces.ts - hook cho thành viên của một workspace và đổi tên, xóa workspace (backend/src/api/workspaces.rs).
Quyền do backend kiểm (docs/ARCHITECTURE.md mục 4b); giao diện chỉ ẩn nút người dùng không bấm được, lỗi từ backend
vẫn hiện nguyên lời. Kiểu dữ liệu lấy từ schema.ts, sinh từ openapi.json; tên ở đây chỉ là bí danh để trang import
gọn. */

import { useMutation, useQuery, useQueryClient, type QueryClient } from '@tanstack/react-query'
import { apiDelete, apiGet, apiPatch, apiPost } from './client'
import { meKey, type Me } from './auth'
import type { components } from './schema'

type Schemas = components['schemas']
export type MemberRow = Schemas['MemberRow']
export type AddMemberRequest = Schemas['AddMemberRequest']

function membersKey(slug: string) {
  return ['workspaces', slug, 'members']
}

function workspacePath(slug: string): string {
  return `/workspaces/${encodeURIComponent(slug)}`
}

/** Mọi thành viên của workspace; ai trong workspace cũng xem được. */
export function useMembers(slug: string) {
  return useQuery({
    queryKey: membersKey(slug),
    queryFn: () => apiGet<MemberRow[]>(`${workspacePath(slug)}/members`),
  })
}

// Vai và tên workspace hiện ở thanh bên lấy từ /auth/me; đổi vai, đổi tên thì tải lại cả nó để thanh bên đổi theo,
// ví dụ admin tự hạ vai mình thì nút quản lý biến mất ngay. Trả promise để mutation còn ở trạng thái chờ cho tới khi
// danh sách tải lại xong: nếu không, ô chọn vai hiện lại vai cũ một lúc, giữa lúc backend trả lời và lúc danh sách mới
// về.
function refreshMembers(queryClient: QueryClient, slug: string) {
  return Promise.all([
    queryClient.invalidateQueries({ queryKey: membersKey(slug) }),
    queryClient.invalidateQueries({ queryKey: meKey }),
  ])
}

export function useAddMember(slug: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (request: AddMemberRequest) => apiPost<void>(`${workspacePath(slug)}/members`, request),
    onSuccess: () => refreshMembers(queryClient, slug),
  })
}

export function useSetMemberRole(slug: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ userId, role }: { userId: string; role: string }) =>
      apiPatch<void>(`${workspacePath(slug)}/members/${userId}`, { role } satisfies Schemas['SetRoleRequest']),
    onSuccess: () => refreshMembers(queryClient, slug),
  })
}

/** Admin xóa một thành viên. Tự rời workspace dùng useLeaveWorkspace. */
export function useRemoveMember(slug: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (userId: string) => apiDelete<void>(`${workspacePath(slug)}/members/${userId}`),
    onSuccess: () => refreshMembers(queryClient, slug),
  })
}

export function useRenameWorkspace(slug: string) {
  const queryClient = useQueryClient()
  return useMutation({
    // Đổi tên dùng cùng body với tạo workspace (CreateWorkspaceRequest ở backend).
    mutationFn: (name: string) =>
      apiPatch<void>(workspacePath(slug), { name } satisfies Schemas['CreateWorkspaceRequest']),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: meKey }),
  })
}

// Sau khi rời hay xóa, trang chuyển về / và RootPage chọn workspace đầu tiên trong /auth/me. Bỏ workspace này khỏi bộ
// đệm ngay, không chờ tải lại: nếu không, RootPage đọc bộ đệm cũ và đưa người dùng quay lại đúng workspace vừa rời.
function forgetWorkspace(queryClient: QueryClient, slug: string) {
  queryClient.setQueryData<Me>(meKey, (me) =>
    me ? { ...me, workspaces: me.workspaces.filter((m) => m.slug !== slug) } : me,
  )
  // Xóa workspace thì slug được trả lại; workspace mới cùng tên lấy lại slug đó thì không được hiện thành viên cũ.
  queryClient.removeQueries({ queryKey: ['workspaces', slug] })
  void queryClient.invalidateQueries({ queryKey: meKey })
}

/** Người đang đăng nhập tự rời workspace. Admin cuối cùng không rời được; backend báo lời. */
export function useLeaveWorkspace(slug: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (ownUserId: string) => apiDelete<void>(`${workspacePath(slug)}/members/${ownUserId}`),
    onSuccess: () => forgetWorkspace(queryClient, slug),
  })
}

/** Xóa workspace cùng mọi dataset và file của nó. Không hoàn tác được; trang hỏi xác nhận trước. */
export function useDeleteWorkspace(slug: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: () => apiDelete<void>(workspacePath(slug)),
    onSuccess: () => forgetWorkspace(queryClient, slug),
  })
}
