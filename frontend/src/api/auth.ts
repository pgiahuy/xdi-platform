// SPDX-License-Identifier: Apache-2.0
/* api/auth.ts - hook cho người dùng đang đăng nhập và đăng xuất (backend/src/api/auth.rs).
Kiểu dữ liệu viết tay theo MeResponse của backend; sẽ sinh từ openapi.json khi có (#53). */

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { apiGet, apiPost } from './client'

export type User = {
  id: string
  email: string
  display_name: string
  is_system_admin: boolean
}

/** Một workspace người dùng thuộc về, kèm vai của họ trong workspace đó. */
export type Membership = {
  workspace_id: string
  name: string
  slug: string
  role: string
}

export type Me = {
  user: User
  workspaces: Membership[]
}

const meKey = ['me']

/** Người đang đăng nhập và các workspace của họ. Chưa đăng nhập thì lỗi ApiError status 401. */
export function useMe() {
  return useQuery({ queryKey: meKey, queryFn: () => apiGet<Me>('/auth/me') })
}

export function useLogout() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: () => apiPost<void>('/auth/logout'),
    // Xóa mọi dữ liệu đã đệm: người đăng nhập kế tiếp trên cùng trình duyệt không thấy dữ liệu của người trước.
    onSuccess: () => queryClient.clear(),
  })
}
