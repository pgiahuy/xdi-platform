// SPDX-License-Identifier: Apache-2.0
/* pages/DatasetsPage.tsx - danh sách dataset và nạp file của workspace. Nội dung làm ở #49. */

import { EmptyState } from '@/components/EmptyState'

export function DatasetsPage() {
  return (
    <EmptyState
      title="Chưa xem được dữ liệu"
      description="Danh sách dataset và nạp file đang được làm, chưa có trong bản này."
    />
  )
}
