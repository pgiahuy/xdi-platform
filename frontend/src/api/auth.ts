// SPDX-License-Identifier: Apache-2.0
/* api/auth.ts - hook cho thiết lập lần đầu, đăng nhập, đăng xuất, đổi mật khẩu và người dùng hiện tại
(backend/src/api/auth.rs). Kiểu request và response lấy từ schema.ts, sinh từ openapi.json; tên ở đây chỉ là bí danh
để trang import gọn. Lỗi từ backend đã là câu tiếng Việt cho người dùng (ApiError.message), trang hiện thẳng. */

import { useMutation, useQuery, useQueryClient, type QueryClient } from '@tanstack/react-query'
import { ApiError, apiGet, apiPost } from './client'
import type { components } from './schema'

type Schemas = components['schemas']
export type User = Schemas['User']
/** Một workspace người dùng thuộc về, kèm vai của họ trong workspace đó. */
export type Membership = Schemas['Membership']
export type Me = Schemas['MeResponse']
export type SetupRequest = Schemas['SetupRequest']
export type LoginRequest = Schemas['LoginRequest']
export type ChangePasswordRequest = Schemas['ChangePasswordRequest']

export const meKey = ['me']
const setupKey = ['auth', 'setup']

/** Độ dài mật khẩu backend chấp nhận (MIN_LENGTH, MAX_LENGTH trong backend/src/auth/password.rs, đếm theo ký tự
 * Unicode). Giao diện dùng để nhắc trước và kiểm trước khi gửi; backend vẫn kiểm lại. schema.ts chỉ mang kiểu, không
 * mang giá trị dùng lúc chạy, nên hai số này ghi tay: đổi ở backend thì đổi cả ở đây. */
export const passwordLength = { min: 8, max: 128 }

/** Người đang đăng nhập và các workspace của họ. Chưa đăng nhập thì lỗi ApiError status 401. */
export function useMe() {
  return useQuery({
    queryKey: meKey,
    queryFn: () => apiGet<Me>('/auth/me'),
    // Trang gốc và khung workspace cùng đọc người dùng ngay sau nhau; không đặt staleTime thì mỗi lần một trang mới
    // gắn vào lại gọi /auth/me. Phiên bị thu hồi vẫn được thấy ngay nhờ quy tắc 401 trong queryClient.ts.
    staleTime: 10_000,
  })
}

/** needed = true khi bản cài chưa có tài khoản nào: giao diện hiện màn tạo tài khoản quản trị thay cho đăng nhập. */
export function useSetupStatus() {
  return useQuery({ queryKey: setupKey, queryFn: () => apiGet<Schemas['SetupStatusResponse']>('/auth/setup') })
}

/** Tạo tài khoản quản trị kèm workspace đầu tiên; backend đăng nhập luôn và trả người dùng như /auth/me. */
export function useSetup() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (request: SetupRequest) => apiPost<Me>('/auth/setup', request),
    // Biến của mutation chứa mật khẩu; mặc định TanStack giữ 5 phút sau khi trang gỡ. Không cần giữ lại.
    gcTime: 0,
    onSuccess: async (me) => {
      await signIn(queryClient, me)
      queryClient.setQueryData(setupKey, { needed: false })
    },
    // 403 gồm cả "bản cài đã được thiết lập", ví dụ một tab khác vừa thiết lập xong. Đọc lại trạng thái để trang chuyển
    // sang màn đăng nhập; nếu chỉ là mã sai thì trạng thái vẫn như cũ, form giữ nguyên.
    onError: (error) => {
      if (error instanceof ApiError && error.status === 403) void queryClient.invalidateQueries({ queryKey: setupKey })
    },
  })
}

export function useLogin() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (request: LoginRequest) => apiPost<Me>('/auth/login', request),
    gcTime: 0,
    onSuccess: (me) => signIn(queryClient, me),
  })
}

/** Ghi người vừa đăng nhập vào bộ đệm, để trang kế tiếp không phải gọi /auth/me lần nữa; việc này cũng xóa lỗi 401 còn
 * lưu từ lúc chưa đăng nhập. Hủy trước lời gọi /auth/me đang chạy: nó trả 401 sau thì sẽ ghi đè người vừa đăng nhập.
 * Người đăng nhập khác người còn trong bộ đệm (phiên trước hết hạn, không qua đăng xuất) thì bỏ hết dữ liệu cũ. */
async function signIn(queryClient: QueryClient, me: Me) {
  await queryClient.cancelQueries({ queryKey: meKey })
  const previous = queryClient.getQueryData<Me>(meKey)
  if (previous && previous.user.id !== me.user.id) queryClient.removeQueries()
  queryClient.setQueryData(meKey, me)
}

/** Đổi mật khẩu của chính mình. Phiên đang dùng giữ lại, các phiên khác của tài khoản bị đăng xuất. */
export function useChangePassword() {
  return useMutation({
    mutationFn: (request: ChangePasswordRequest) => apiPost<void>('/auth/password', request),
    gcTime: 0,
  })
}

/** onSignedOut chuyển trang sau khi đăng xuất. Bắt buộc: clear ở dưới không báo cho component đang hiện, nên không
 * chuyển trang thì trang vẫn hiện người dùng cũ. Trang nên tải lại cả trang (window.location), không chuyển trong ứng
 * dụng: xem signOutTo trong pages/authNav.ts. */
export function useLogout(onSignedOut: () => void) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: () => apiPost<void>('/auth/logout'),
    // Xóa hết dữ liệu đã đệm, để người đăng nhập kế tiếp trên cùng trình duyệt không thấy dữ liệu của người trước,
    // rồi chuyển trang ngay. Không dùng resetQueries: nó tải lại /auth/me, nhận 401, và trang đang mở tự chuyển tới
    // /login?next=<trang cũ>; lần chuyển đó có thể đến sau và thắng, người đăng nhập sau bị đưa vào trang của người
    // trước. Đặt ở đây, không ở callback của mutate: callback đó không chạy khi trang gọi đăng xuất đã gỡ.
    onSuccess: () => {
      queryClient.clear()
      onSignedOut()
    },
  })
}
