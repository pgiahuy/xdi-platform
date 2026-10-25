// SPDX-License-Identifier: Apache-2.0
/* api/drill.ts - truy dòng nguồn: POST /workspaces/{ws}/datasets/{id}/drill (backend/src/api/query.rs), hợp đồng ở
backend/src/service/drill_contract.rs. Request gửi query_id cùng khóa của chỗ được bấm, không bao giờ gửi SQL. Backend
truy được ô của bảng kết quả (cell); dòng phân rã (breakdown) còn trả 400 tới khi câu "tại sao" ghi lát cắt vào
queries.explanation (#38).
Ở chế độ dev có thể trả vài dòng mẫu (drillSamples.ts) thay vì gọi backend, như askSamples.ts. */

import { useQuery } from '@tanstack/react-query'
import { apiPost } from './client'
import { drillFromSamples } from './drillSamples'
import type { components } from './schema'

type Schemas = components['schemas']
export type BreakdownKey = Schemas['BreakdownKey']
export type DrillRequest = Schemas['DrillRequest']
export type DrillResponse = Schemas['DrillResponse']

export type DrillVariables = {
  datasetId: string
  request: DrillRequest
  /** Chỉ ở chế độ dev: trả dòng mẫu, không gọi backend. */
  fromSamples?: boolean
}

// Cỡ trang mặc định của backend; tab Dòng nguồn chỉ hiện trang đầu.
export const drillPageSize = 100

function drillPath(ws: string, datasetId: string): string {
  return `/workspaces/${encodeURIComponent(ws)}/datasets/${encodeURIComponent(datasetId)}/drill`
}

/** Dòng nguồn của chỗ được bấm; variables null thì chưa gọi. Thử lại theo quy tắc chung ở queryClient.ts: lỗi 400
 * (chưa truy được) không thử lại. */
export function useDrill(ws: string, variables: DrillVariables | null) {
  return useQuery({
    queryKey: ['workspaces', ws, 'drill', variables],
    queryFn: () => {
      // enabled bảo đảm variables có giá trị khi hàm này chạy.
      const { datasetId, request, fromSamples } = variables as DrillVariables
      if (import.meta.env.DEV && fromSamples) return drillFromSamples(request)
      return apiPost<DrillResponse>(drillPath(ws, datasetId), request)
    },
    enabled: variables !== null,
  })
}
