// SPDX-License-Identifier: Apache-2.0
/* api/overview.ts - hook đọc tổng quan một dataset (GET .../datasets/{id}/overview, backend/src/service/overview/): hồ
sơ cột, chỉ số, biểu đồ dựng sẵn. Kiểu lấy từ schema.ts. Khóa nằm dưới khóa danh sách dataset (api/datasets.ts), nên
nạp hay xóa dataset làm danh sách tải lại thì tổng quan cũng tải lại. */

import { useQuery } from '@tanstack/react-query'
import { apiGet } from './client'
import type { components } from './schema'

type Schemas = components['schemas']
export type DatasetOverview = Schemas['DatasetOverviewResponse']
export type OverviewChart = Schemas['OverviewChart']
export type OverviewColumn = Schemas['OverviewColumn']

// Hồ sơ đang lập thì hỏi lại như danh sách dataset (PROFILE_POLL_MS ở api/datasets.ts): phần còn thiếu hiện ra mà không
// phải tải lại trang. Chỉ hỏi lại khi khung đang mở (enabled), và chỉ với hai trạng thái còn chờ được.
const OVERVIEW_POLL_MS = 10_000
const waitingCodes = ['profile_pending', 'semantic_pending']

export function useDatasetOverview(ws: string, datasetId: string, enabled: boolean) {
  return useQuery({
    queryKey: ['workspaces', ws, 'datasets', datasetId, 'overview'],
    queryFn: () =>
      apiGet<DatasetOverview>(
        `/workspaces/${encodeURIComponent(ws)}/datasets/${encodeURIComponent(datasetId)}/overview`,
      ),
    enabled: enabled && datasetId !== '',
    refetchInterval: (query) => {
      const missing = query.state.data?.missing ?? []
      return missing.some((part) => waitingCodes.includes(part.code)) ? OVERVIEW_POLL_MS : false
    },
  })
}
