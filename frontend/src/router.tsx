// SPDX-License-Identifier: Apache-2.0
/* router.tsx - bảng đường dẫn của giao diện. Thêm trang mới là thêm một dòng ở đây. */

import { createBrowserRouter } from 'react-router'
import { HomePage } from '@/pages/HomePage'
import { NotFoundPage } from '@/pages/NotFoundPage'

export const router = createBrowserRouter([
  { path: '/', element: <HomePage /> },
  { path: '*', element: <NotFoundPage /> },
])
