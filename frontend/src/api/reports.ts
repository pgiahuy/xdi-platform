// SPDX-License-Identifier: Apache-2.0
/* api/reports.ts - báo cáo của người gọi, route ở backend/src/api/reports.rs: danh sách, tạo, sửa tiêu đề và đoạn mở
đầu, xóa, thêm và bỏ mục, đổi thứ tự, mở (mọi mục dựng lại từ câu hỏi đã lưu, backend chạy lại câu SQL), tạo báo cáo
tự động cho một dataset, và đường dẫn tải file Excel. Mở một báo cáo chạy lại mọi câu SQL của nó, nên bản đã tải giữ
một lúc (staleTime) thay vì tải lại mỗi lần quay lại tab. */

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { apiDelete, apiGet, apiPatch, apiPost, apiPut } from './client'
import type { components } from './schema'

type Schemas = components['schemas']
export type ReportSummary = Schemas['ReportSummary']
export type ReportResponse = Schemas['ReportResponse']
export type ReportItem = Schemas['ReportItem']
export type ReportBreakdown = Schemas['ReportBreakdown']
export type AutoReportResponse = Schemas['AutoReportResponse']
export type NumberCheck = Schemas['NumberCheck']

function reportsPath(ws: string): string {
  return `/workspaces/${encodeURIComponent(ws)}/reports`
}

function reportPath(ws: string, id: string): string {
  return `${reportsPath(ws)}/${encodeURIComponent(id)}`
}

function reportsKey(ws: string) {
  return ['workspaces', ws, 'reports'] as const
}

function reportKey(ws: string, id: string) {
  return [...reportsKey(ws), id] as const
}

/** Đường dẫn tải file Excel của báo cáo: trình duyệt gửi kèm cookie phiên như mọi request cùng địa chỉ. */
export function reportXlsxHref(ws: string, id: string): string {
  return `/api${reportPath(ws, id)}/xlsx`
}

export function useReports(ws: string) {
  return useQuery({
    queryKey: reportsKey(ws),
    queryFn: () => apiGet<Schemas['ReportListResponse']>(reportsPath(ws)).then((r) => r.reports),
  })
}

/** Một báo cáo dựng lại; id rỗng thì chưa gọi. */
export function useReport(ws: string, id: string) {
  return useQuery({
    queryKey: reportKey(ws, id),
    queryFn: () => apiGet<ReportResponse>(reportPath(ws, id)),
    enabled: id !== '',
    // Mỗi lần tải là một lần chạy lại mọi câu SQL; số chỉ đổi khi báo cáo đổi (các mutation dưới đây làm mới).
    staleTime: 5 * 60_000,
  })
}

export function useCreateReport(ws: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (body: Schemas['CreateReportRequest']) => apiPost<ReportSummary>(reportsPath(ws), body),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: reportsKey(ws), exact: true }),
  })
}

export function useCreateAutoReport(ws: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (datasetId: string) =>
      apiPost<AutoReportResponse>(`${reportsPath(ws)}/automatic`, {
        dataset_id: datasetId,
      } satisfies Schemas['AutoReportRequest']),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: reportsKey(ws), exact: true }),
  })
}

export function useUpdateReport(ws: string, id: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (body: Schemas['UpdateReportRequest']) => apiPatch<ReportSummary>(reportPath(ws, id), body),
    // Chỉ tiêu đề và đoạn mở đầu đổi: sửa bản đang có, không chạy lại mọi câu SQL của báo cáo.
    onSuccess: (summary) => {
      queryClient.setQueryData<ReportResponse>(reportKey(ws, id), (old) => old && { ...old, report: summary })
      return queryClient.invalidateQueries({ queryKey: reportsKey(ws), exact: true })
    },
  })
}

export function useDeleteReport(ws: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (id: string) => apiDelete<void>(reportPath(ws, id)),
    onSuccess: (_, id) => {
      queryClient.removeQueries({ queryKey: reportKey(ws, id) })
      return queryClient.invalidateQueries({ queryKey: reportsKey(ws), exact: true })
    },
  })
}

/** Thêm một câu đã trả lời vào cuối một báo cáo. */
export function useAddReportItem(ws: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ reportId, queryId }: { reportId: string; queryId: string }) =>
      apiPost<ReportSummary>(`${reportPath(ws, reportId)}/items`, {
        query_id: queryId,
      } satisfies Schemas['AddReportItemRequest']),
    // Danh sách (số mục) và đúng báo cáo vừa thêm; báo cáo khác không đổi nên không chạy lại.
    onSuccess: (_, { reportId }) =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: reportsKey(ws), exact: true }),
        queryClient.invalidateQueries({ queryKey: reportKey(ws, reportId) }),
      ]),
  })
}

export function useRemoveReportItem(ws: string, reportId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (itemId: string) => apiDelete<void>(`${reportPath(ws, reportId)}/items/${encodeURIComponent(itemId)}`),
    // Bỏ mục khỏi bản đang có, không chạy lại các mục còn lại.
    onSuccess: (_, itemId) => {
      queryClient.setQueryData<ReportResponse>(
        reportKey(ws, reportId),
        (old) => old && { ...old, items: old.items.filter((item) => item.id !== itemId) },
      )
      return queryClient.invalidateQueries({ queryKey: reportsKey(ws), exact: true })
    },
  })
}

/** Ghi thứ tự mới; bản đang có xếp lại ngay, không chạy lại các câu SQL. */
export function useReorderReport(ws: string, reportId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (itemIds: string[]) =>
      apiPut<void>(`${reportPath(ws, reportId)}/order`, { item_ids: itemIds } satisfies Schemas['ReorderReportRequest']),
    onMutate: (itemIds) => {
      queryClient.setQueryData<ReportResponse>(reportKey(ws, reportId), (old) => {
        if (!old) return old
        const byId = new Map(old.items.map((item) => [item.id, item]))
        return { ...old, items: itemIds.flatMap((id) => byId.get(id) ?? []) }
      })
    },
    onError: () => queryClient.invalidateQueries({ queryKey: reportKey(ws, reportId) }),
  })
}
