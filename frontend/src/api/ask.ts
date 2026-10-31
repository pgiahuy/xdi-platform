// SPDX-License-Identifier: Apache-2.0
/* api/ask.ts - hỏi dữ liệu: POST /workspaces/{ws}/datasets/{id}/ask (backend/src/api/query.rs), kiểu response ở
backend/src/service/ask/contract.rs. Câu sau của cùng cuộc trò chuyện gửi kèm conversation_id để mô hình đọc nó như câu
tiếp nối (backend/src/service/conversation/). Quy tắc gửi lựa chọn ở docs/ARCHITECTURE.md mục 8: backend không giữ trạng thái
giữa hai lần hỏi, nên hỏi lại luôn gửi lại câu hỏi kèm choices của response gần nhất, thay hoặc thêm mục vừa chọn. */

import { useIsMutating, useMutation, useQueryClient } from '@tanstack/react-query'
import { apiPost } from './client'
import { conversationsKey } from './conversations'
import type { components } from './schema'

type Schemas = components['schemas']
export type AskResponse = Schemas['AskResponse']
export type AskRequest = Schemas['AskRequest']
export type ChosenOption = Schemas['ChosenOption']
export type Choice = Schemas['Choice']
export type Understanding = Schemas['Understanding']
export type ResultTable = Schemas['ResultTable']
export type ChartSuggestion = Schemas['ChartSuggestion']
export type Insight = Schemas['Insight']

export type AskVariables = {
  datasetId: string
  question: string
  choices: ChosenOption[]
  /** Cuộc trò chuyện đang hỏi tiếp (AskResponse.conversation_id của lượt trước); vắng thì backend mở cuộc mới. */
  conversationId?: string
  /** choice_id của các câu hỏi lại người dùng vừa trả lời trong lần gửi này; chỉ giao diện dùng, không gửi lên. */
  answered?: string[]
}

function askPath(ws: string, datasetId: string): string {
  return `/workspaces/${encodeURIComponent(ws)}/datasets/${encodeURIComponent(datasetId)}/ask`
}

function askKey(ws: string) {
  return ['workspaces', ws, 'ask'] as const
}

/** Gửi một câu hỏi. Không thử lại: mỗi lần hỏi gọi mô hình ngôn ngữ và ghi một dòng lịch sử, lỗi thì người dùng tự
 * bấm hỏi lại. Lỗi 501, 502, 503 có message tiếng Việt của backend trong ApiError. Hỏi xong thì làm mới danh sách
 * cuộc trò chuyện của dataset: câu đầu của cuộc mới làm cuộc đó hiện trong danh sách. */
export function useAsk(ws: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationKey: askKey(ws),
    mutationFn: ({ datasetId, question, choices, conversationId }: AskVariables) => {
      const body: AskRequest = { question, choices, conversation_id: conversationId ?? null }
      return apiPost<AskResponse>(askPath(ws, datasetId), body)
    },
    retry: false,
    onSuccess: (_response, { datasetId }) => {
      void queryClient.invalidateQueries({ queryKey: conversationsKey(ws, datasetId) })
    },
  })
}

/** Có câu hỏi gửi bằng useAsk của workspace đang chờ trả lời; đọc được từ ngoài trang hỏi (bộ đệm mutation của TanStack
 * Query). Menu lịch sử trên thanh bên tắt các nút lúc đó: mở cuộc khác thì trang hỏi dựng lại và không hiện câu trả lời
 * đang chờ. */
export function useAskPending(ws: string): boolean {
  return useIsMutating({ mutationKey: askKey(ws) }) > 0
}

/** Lựa chọn cho lần hỏi lại: lấy choices của response gần nhất, mục cùng choice_id thì thay, mục mới thì thêm. Gửi
 * thiếu một lựa chọn đã trả lời thì backend hỏi lại chỗ đó, nên không được dựng lại danh sách từ đầu. */
export function buildChoicesWith(previous: ChosenOption[], picks: ChosenOption[]): ChosenOption[] {
  const changed = new Set(picks.map((pick) => pick.choice_id))
  return [...previous.filter((choice) => !changed.has(choice.choice_id)), ...picks]
}

/** needs_choice hỏi lại đúng những chỗ vừa trả lời: backend không nhận lựa chọn đó (cảnh báo choice_not_applied).
 * Hiện lại cùng câu hỏi thì người dùng chọn mãi không xong, nên trang chỉ hiện lời báo (hợp đồng, ARCHITECTURE mục 8).
 * Chỉ so với các chỗ vừa trả lời ở lần gửi này, không so với mọi lựa chọn cũ: lựa chọn từ trước có thể hết hiệu lực
 * khi dữ liệu đổi, lúc đó backend hỏi lại một cách bình thường; và còn chỗ mơ hồ mới nào thì vẫn phải hiện form. */
export function isRepeatedClarification(answered: string[], response: AskResponse): boolean {
  const notApplied = response.warnings.some((warning) => warning.code === 'choice_not_applied')
  return (
    response.status === 'needs_choice' &&
    notApplied &&
    response.clarifications.length > 0 &&
    response.clarifications.every((clarification) => answered.includes(clarification.id))
  )
}
