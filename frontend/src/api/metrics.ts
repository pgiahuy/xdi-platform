// SPDX-License-Identifier: Apache-2.0
/* api/metrics.ts - hook cho chỉ số của một dataset và cách gộp mặc định của cột số đo (backend/src/api/metrics.rs).
Mọi thành viên đọc được; tạo, sửa, xóa, đổi cách gộp chỉ admin (backend kiểm). Lỗi của một trường có ApiError.field.
Ghi xong thì tải lại danh sách: sửa là thêm phiên bản, số phiên bản và id mới do backend đặt. */

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { apiDelete, apiGet, apiPost, apiPut } from './client'
import type { components } from './schema'

type Schemas = components['schemas']
export type Metric = Schemas['Metric']
export type MetricColumn = Schemas['MetricColumn']
export type MetricRequest = Schemas['MetricRequest']
export type MetricsResponse = Schemas['MetricsResponse']
export type ColumnAggregation = Schemas['ColumnAggregation']

function datasetPath(ws: string, datasetId: string): string {
  return `/workspaces/${encodeURIComponent(ws)}/datasets/${encodeURIComponent(datasetId)}`
}

function metricsKey(ws: string, datasetId: string) {
  return ['workspaces', ws, 'datasets', datasetId, 'metrics']
}

export function useMetrics(ws: string, datasetId: string) {
  return useQuery({
    queryKey: metricsKey(ws, datasetId),
    queryFn: () => apiGet<MetricsResponse>(`${datasetPath(ws, datasetId)}/metrics`),
  })
}

/** Tạo chỉ số mới (id không có) hoặc lưu phiên bản mới của chỉ số có id này. */
export function useSaveMetric(ws: string, datasetId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ id, request }: { id?: string; request: MetricRequest }) => {
      const base = `${datasetPath(ws, datasetId)}/metrics`
      return id
        ? apiPut<Metric>(`${base}/${encodeURIComponent(id)}`, request)
        : apiPost<Metric>(base, request)
    },
    onSuccess: () => queryClient.invalidateQueries({ queryKey: metricsKey(ws, datasetId) }),
  })
}

export function useDeleteMetric(ws: string, datasetId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (id: string) =>
      apiDelete<void>(`${datasetPath(ws, datasetId)}/metrics/${encodeURIComponent(id)}`),
    onSettled: () => queryClient.invalidateQueries({ queryKey: metricsKey(ws, datasetId) }),
  })
}

export function useSetAggregation(ws: string, datasetId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (request: Schemas['ColumnAggregationRequest']) =>
      apiPut<void>(`${datasetPath(ws, datasetId)}/columns/aggregation`, request),
    onSettled: () => queryClient.invalidateQueries({ queryKey: metricsKey(ws, datasetId) }),
  })
}
