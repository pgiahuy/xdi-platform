// SPDX-License-Identifier: Apache-2.0
/* router.tsx - bảng đường dẫn của giao diện. Trang làm việc nằm dưới /w/:ws (slug của workspace), hiện trong khung
chung WorkspaceLayout; thêm trang trong workspace là thêm một route con ở đây và một dòng ở pages/workspaceNav.ts. */

import { createBrowserRouter } from 'react-router'
import { AskPage } from '@/pages/AskPage'
import { DatasetsPage } from '@/pages/DatasetsPage'
import { NotFoundPage } from '@/pages/NotFoundPage'
import { RootPage } from '@/pages/RootPage'
import { WorkspaceLayout } from '@/pages/WorkspaceLayout'

export const router = createBrowserRouter([
  { path: '/', element: <RootPage /> },
  {
    path: '/w/:ws',
    element: <WorkspaceLayout />,
    children: [
      { index: true, element: <AskPage /> },
      { path: 'datasets', element: <DatasetsPage /> },
    ],
  },
  { path: '*', element: <NotFoundPage /> },
])
