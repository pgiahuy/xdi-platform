// SPDX-License-Identifier: Apache-2.0
/* router.tsx - bảng đường dẫn của giao diện. Trang làm việc nằm dưới workspaceRoute (/w/:ws, slug của workspace), hiện
trong khung chung AppLayout; thêm trang trong workspace là thêm một route con ở đây và một dòng ở pages/workspaceNav.ts.
Trang quản trị hệ thống và trang tài khoản ngoài workspace cũng hiện trong AppLayout nhưng không cần workspace, đường
dẫn ở pages/globalNav.ts. Màn đăng nhập và thiết lập nằm ngoài khung, đường dẫn ở pages/authNav.ts.
Đăng nhập, thiết lập, tài khoản, thành viên, các trang quản trị, trang Tổng quan dữ liệu và trang Dữ liệu tải riêng khi
mở (lazy): ít khi cần so với hỏi, hoặc kéo theo phần nạp file, nên không đóng vào gói chính. Mọi route nằm dưới một
route gốc có errorElement, để lỗi ở bất kỳ trang nào cũng hiện câu tiếng Việt. */

import { createBrowserRouter } from 'react-router'
import { StatusScreen } from '@/components/StatusScreen'
import { AppLayout } from '@/pages/AppLayout'
import { AskPage } from '@/pages/AskPage'
import { loginRoute, setupRoute } from '@/pages/authNav'
import { accountRoute, adminAccountsRoute, adminWorkspacesRoute } from '@/pages/globalNav'
import { NotFoundPage } from '@/pages/NotFoundPage'
import { RootPage } from '@/pages/RootPage'
import { RouteErrorPage } from '@/pages/RouteErrorPage'
import { accountPath, examplesPath, lessonsPath, membersPath, overviewPath, workspaceRoute } from '@/pages/workspaceNav'
import { WorkspaceNotFoundPage } from '@/pages/WorkspaceNotFoundPage'

async function loadAccountPage() {
  return { Component: (await import('@/pages/AccountPage')).AccountPage }
}

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
      // Một khung cho trang trong workspace và trang ngoài workspace, để chuyển giữa hai nhóm không dựng lại khung
      // (pages/AppLayout.tsx).
      {
        element: <AppLayout />,
        children: [
          {
            path: workspaceRoute,
            children: [
              // Trang hỏi không có mục trên thanh bên (workspaceNav.ts), nên tiêu đề đặt ở đây.
              { index: true, element: <AskPage />, handle: { title: 'Cuộc trò chuyện' } },
              {
                path: overviewPath,
                lazy: async () => ({ Component: (await import('@/pages/OverviewPage')).OverviewPage }),
              },
              // Trang Dữ liệu kéo theo phần nạp file và chọn sheet Excel: tải riêng khi mở, như các màn đăng nhập.
              {
                path: 'datasets',
                lazy: async () => ({ Component: (await import('@/pages/DatasetsPage')).DatasetsPage }),
              },
              {
                path: 'datasets/:datasetId/metrics',
                lazy: async () => ({ Component: (await import('@/pages/MetricsPage')).MetricsPage }),
                handle: { title: 'Chỉ số' },
              },
              {
                path: lessonsPath,
                lazy: async () => ({ Component: (await import('@/pages/LessonsPage')).LessonsPage }),
              },
              {
                path: examplesPath,
                lazy: async () => ({ Component: (await import('@/pages/ExamplesPage')).ExamplesPage }),
              },
              {
                path: membersPath,
                lazy: async () => ({ Component: (await import('@/pages/MembersPage')).MembersPage }),
              },
              { path: accountPath, lazy: loadAccountPage, handle: { title: 'Tài khoản' } },
              // Đường dẫn sai trong workspace vẫn hiện trong khung workspace, không rơi xuống 404 cả trang ở dưới.
              { path: '*', element: <WorkspaceNotFoundPage />, handle: { title: 'Không có trang này' } },
            ],
          },
          {
            path: adminAccountsRoute,
            lazy: async () => ({ Component: (await import('@/pages/AdminAccountsPage')).AdminAccountsPage }),
            // Mục thanh bên chỉ hiện với quản trị hệ thống; ai mở thẳng đường dẫn vẫn cần tiêu đề trang.
            handle: { title: 'Quản lý tài khoản' },
          },
          {
            path: adminWorkspacesRoute,
            lazy: async () => ({ Component: (await import('@/pages/AdminWorkspacesPage')).AdminWorkspacesPage }),
            handle: { title: 'Mọi workspace' },
          },
          // Cùng trang với /w/:ws/account, cho người đang ở ngoài workspace hoặc chưa thuộc workspace nào.
          { path: accountRoute, lazy: loadAccountPage, handle: { title: 'Tài khoản' } },
        ],
      },
      { path: '*', element: <NotFoundPage /> },
    ],
  },
])
