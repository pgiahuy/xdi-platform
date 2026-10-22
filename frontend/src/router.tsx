// SPDX-License-Identifier: Apache-2.0
/* router.tsx - bảng đường dẫn của giao diện. Trang làm việc nằm dưới workspaceRoute (/w/:ws, slug của workspace), hiện
trong khung chung WorkspaceLayout; thêm trang trong workspace là thêm một route con ở đây và một dòng ở
pages/workspaceNav.ts. Màn đăng nhập và thiết lập nằm ngoài khung, đường dẫn ở pages/authNav.ts.
Đăng nhập, thiết lập và tài khoản tải riêng khi mở (lazy): người đã đăng nhập hiếm khi cần, nên không đóng vào gói
chính. Trang Dữ liệu cũng tải riêng vì kéo theo phần nạp file. Mọi route nằm dưới một route gốc có errorElement, để lỗi ở bất kỳ trang nào cũng hiện câu tiếng Việt. */

import { createBrowserRouter } from 'react-router'
import { StatusScreen } from '@/components/StatusScreen'
import { AskPage } from '@/pages/AskPage'
import { loginRoute, setupRoute } from '@/pages/authNav'
import { NotFoundPage } from '@/pages/NotFoundPage'
import { RootPage } from '@/pages/RootPage'
import { RouteErrorPage } from '@/pages/RouteErrorPage'
import { WorkspaceLayout } from '@/pages/WorkspaceLayout'
import { accountPath, workspaceRoute } from '@/pages/workspaceNav'
import { WorkspaceNotFoundPage } from '@/pages/WorkspaceNotFoundPage'

export const router = createBrowserRouter([
  {
    errorElement: <RouteErrorPage />,
    // Mở thẳng một trang tải riêng (ví dụ /login) thì router chờ tải xong mới vẽ; trong lúc đó hiện màn này.
    hydrateFallbackElement: <StatusScreen busy title="Đang tải..." />,
    children: [
      { path: '/', element: <RootPage /> },
      {
        path: loginRoute,
        lazy: async () => ({ Component: (await import('@/pages/LoginPage')).LoginPage }),
      },
      {
        path: setupRoute,
        lazy: async () => ({ Component: (await import('@/pages/SetupPage')).SetupPage }),
      },
      {
        path: workspaceRoute,
        element: <WorkspaceLayout />,
        children: [
          { index: true, element: <AskPage /> },
          // Trang Dữ liệu kéo theo phần nạp file và chọn sheet Excel: tải riêng khi mở, như các màn đăng nhập.
          {
            path: 'datasets',
            lazy: async () => ({ Component: (await import('@/pages/DatasetsPage')).DatasetsPage }),
          },
          {
            path: accountPath,
            lazy: async () => ({ Component: (await import('@/pages/AccountPage')).AccountPage }),
            handle: { title: 'Tài khoản' },
          },
          // Đường dẫn sai trong workspace vẫn hiện trong khung workspace, không rơi xuống 404 cả trang ở dưới.
          { path: '*', element: <WorkspaceNotFoundPage />, handle: { title: 'Không có trang này' } },
        ],
      },
      { path: '*', element: <NotFoundPage /> },
    ],
  },
])
