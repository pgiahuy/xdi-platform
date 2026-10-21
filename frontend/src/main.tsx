// SPDX-License-Identifier: Apache-2.0
/* main.tsx - điểm vào của giao diện: gắn bộ đệm dữ liệu (TanStack Query) và router. */

import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider } from 'react-router'
import { ApiError } from './api/client'
import { router } from './router'
import './index.css'

// Mặc định TanStack thử lại 3 lần: backend tắt thì người dùng chờ lâu mới thấy thông báo lỗi. Một lần là đủ cho lỗi mạng
// thoáng qua. Lỗi 4xx (chưa đăng nhập, không có quyền, không tồn tại) gửi lại vẫn ra kết quả cũ, nên không thử lại.
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: (failureCount, error) => {
        const clientError = error instanceof ApiError && error.status >= 400 && error.status < 500
        return !clientError && failureCount < 1
      },
    },
  },
})

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </StrictMode>,
)
