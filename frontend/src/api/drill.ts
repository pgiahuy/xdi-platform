// SPDX-License-Identifier: Apache-2.0
/* api/drill.ts - truy dòng nguồn: POST /workspaces/{ws}/datasets/{id}/drill (backend/src/api/query.rs).
CHƯA CÓ HỢP ĐỒNG: route còn trả 501 và chưa có kiểu request, response trong openapi.json (#34). Kiểu dưới đây viết tay
theo đề xuất DrillRequest, DrillResponse của #34, chỉ phần dòng phân rã (breakdown), để giao diện dựng được tab Dòng
nguồn; khi #34 chốt và chạy make openapi thì thay bằng kiểu sinh trong schema.ts và sửa theo hợp đồng đã chốt.
Ở chế độ dev có thể trả vài dòng mẫu (drillSamples.ts) thay vì gọi backend, như askSamples.ts. */

import { useQuery } from '@tanstack/react-query'
import type { ResultTable } from './ask'
import { apiPost } from './client'
import { drillFromSamples } from './drillSamples'
import type { components } from './schema'

type CellValue = components['schemas']['CellValue']

/** Khóa của một dòng phân rã: lát cắt, cột như tên trong dataset, giá trị nhóm. Đề xuất của #34, chưa chốt. */
export type BreakdownKey = {
  slice: 'base' | 'compare'
  column: string
  value: CellValue | null
}

/** Đề xuất của #34, chỉ phần breakdown; phần cell (bấm ô của bảng kết quả) ngoài phạm vi #52. */
export type DrillRequest = {
  query_id: string
  breakdown: BreakdownKey
  offset: number
  limit: number
}

/** Đề xuất của #34: câu SQL đã chạy và các dòng, cùng dạng bảng kết quả của ask. */
export type DrillResponse = {
  sql: string
  result: ResultTable
}

export type DrillVariables = {
  datasetId: string
  request: DrillRequest
  /** Chỉ ở chế độ dev: trả dòng mẫu, không gọi backend. */
  fromSamples?: boolean
}

// Cỡ trang mặc định #34 đề xuất; tab Dòng nguồn chỉ hiện trang đầu.
export const drillPageSize = 100

function drillPath(ws: string, datasetId: string): string {
  return `/workspaces/${encodeURIComponent(ws)}/datasets/${encodeURIComponent(datasetId)}/drill`
}

/** Dòng nguồn của một dòng phân rã; variables null thì chưa gọi. Lỗi 501 (chưa cài đặt) không thử lại, theo quy tắc
 * chung ở queryClient.ts chỉ áp cho 4xx, nên đặt retry riêng: gọi lại vẫn ra 501. */
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
    retry: false,
  })
}
