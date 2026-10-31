// SPDX-License-Identifier: Apache-2.0
/* api/feedback.ts - đánh giá câu trả lời và câu mẫu (#212), route ở backend/src/api/feedback.rs. Người hỏi bấm "Tốt"
hay "Chưa đạt" trên câu trả lời của mình (PUT, bấm lại nút đang chọn là DELETE); admin xem câu "Tốt", duyệt làm câu
mẫu, và xuất câu mẫu của một dataset theo khuôn ca của bộ đánh giá (eval/). Quyền do backend kiểm: member gọi route
của admin nhận 403. */

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { apiDelete, apiGet, apiPut } from './client'
import type { components } from './schema'

type Schemas = components['schemas']
export type QueryFeedback = Schemas['QueryFeedback']
export type FeedbackReason = Schemas['FeedbackReason']
export type Rating = Schemas['Rating']
export type ExampleCandidate = Schemas['ExampleCandidate']
export type EvalCaseCandidate = Schemas['EvalCaseCandidate']

function workspacePath(ws: string): string {
  return `/workspaces/${encodeURIComponent(ws)}`
}

function examplesKey(ws: string) {
  return ['workspaces', ws, 'examples'] as const
}

/** Đánh giá cần gửi; null là bỏ đánh giá. */
export type RatingChange = { rating: Rating; reason?: FeedbackReason; note?: string } | null

/** Ghi hay bỏ đánh giá của một câu trả lời; kết quả là đánh giá đã lưu (null khi đã bỏ). */
export function useRateAnswer(ws: string, queryId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (change: RatingChange): Promise<QueryFeedback | null> => {
      const path = `${workspacePath(ws)}/queries/${encodeURIComponent(queryId)}/feedback`
      if (change === null) {
        await apiDelete<void>(path)
        return null
      }
      return apiPut<QueryFeedback>(path, {
        rating: change.rating,
        reason: change.reason ?? null,
        note: change.note ?? null,
      } satisfies Schemas['FeedbackRequest'])
    },
    // Danh sách duyệt của admin đổi theo: câu mới "Tốt" hay câu vừa thôi "Tốt".
    onSuccess: () => queryClient.invalidateQueries({ queryKey: examplesKey(ws) }),
  })
}

/** Câu "Tốt" của workspace (chỉ admin). */
export function useExamples(ws: string, enabled: boolean) {
  return useQuery({
    queryKey: examplesKey(ws),
    queryFn: () => apiGet<Schemas['ExampleCandidatesResponse']>(`${workspacePath(ws)}/examples`).then((r) => r.examples),
    enabled,
  })
}

/** Duyệt (approve true) hay bỏ duyệt một câu làm câu mẫu. */
export function useSetExampleApproval(ws: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ queryId, approve }: { queryId: string; approve: boolean }) => {
      const path = `${workspacePath(ws)}/examples/${encodeURIComponent(queryId)}/approval`
      return approve ? apiPut<void>(path) : apiDelete<void>(path)
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: examplesKey(ws) }),
  })
}

/** Câu mẫu đã duyệt của một dataset theo khuôn ca của bộ đánh giá; gọi khi bấm nút xuất, không giữ trong bộ đệm. */
export function useExportExamples(ws: string) {
  return useMutation({
    mutationFn: (datasetId: string) =>
      apiGet<EvalCaseCandidate[]>(`${workspacePath(ws)}/datasets/${encodeURIComponent(datasetId)}/examples/export`),
  })
}
