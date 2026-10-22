// SPDX-License-Identifier: Apache-2.0
/* api/queryClient.ts - bộ đệm dữ liệu (TanStack Query) dùng chung cho mọi hook trong src/api; main.tsx gắn nó vào app.
Hai quy tắc chung cho mọi lời gọi: khi nào thử lại, và làm gì khi phiên đăng nhập không còn. */

import { MutationCache, QueryCache, QueryClient } from '@tanstack/react-query'
import { ApiError } from './client'
import { meKey } from './auth'

export function buildQueryClient(): QueryClient {
  const queryClient: QueryClient = new QueryClient({
    queryCache: new QueryCache({
      // Phiên hết hạn hoặc bị đăng xuất ở tab khác thì API dữ liệu trả 401, nhưng người dùng hiện tại vẫn nằm trong
      // bộ đệm và giao diện vẫn hiện như đang đăng nhập. Tải lại người dùng hiện tại để trang chuyển sang màn chưa
      // đăng nhập.
      onError: (error, query) => {
        if (isUnauthorized(error) && query.queryKey[0] !== meKey[0]) {
          void queryClient.invalidateQueries({ queryKey: meKey })
        }
      },
    }),
    // Cùng quy tắc cho thao tác ghi, ví dụ đổi mật khẩu khi quản trị vừa đặt lại mật khẩu (backend trả 401). Chỉ khi
    // bộ đệm còn người dùng: đăng nhập sai cũng là 401, lúc đó chưa có ai để tải lại, và tải lại /auth/me sẽ đưa màn
    // đăng nhập về trạng thái đang tải, xóa các ô đã gõ.
    mutationCache: new MutationCache({
      onError: (error) => {
        if (isUnauthorized(error) && queryClient.getQueryData(meKey) !== undefined) {
          void queryClient.invalidateQueries({ queryKey: meKey })
        }
      },
    }),
    defaultOptions: {
      queries: {
        // Mặc định TanStack thử lại 3 lần: backend tắt thì người dùng chờ lâu mới thấy thông báo lỗi. Một lần là đủ cho
        // lỗi mạng thoáng qua. Lỗi 4xx (chưa đăng nhập, không có quyền, không tồn tại) gửi lại vẫn ra kết quả cũ, nên
        // không thử lại; trừ 408 và 429, hai lỗi tự hết sau một lúc.
        retry: (failureCount, error) => {
          const permanent =
            error instanceof ApiError &&
            error.status >= 400 &&
            error.status < 500 &&
            error.status !== 408 &&
            error.status !== 429
          return !permanent && failureCount < 1
        },
      },
    },
  })
  return queryClient
}

export function isUnauthorized(error: unknown): boolean {
  return error instanceof ApiError && error.status === 401
}
