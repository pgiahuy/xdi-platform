// SPDX-License-Identifier: Apache-2.0
/* api/conversations.ts - đọc cuộc trò chuyện trên một dataset (#164): GET .../datasets/{id}/conversations (danh sách
cuộc của người gọi, mới trước) và GET .../conversations/{conversation_id} (các lượt đã ghi của một cuộc), route ở
backend/src/api/conversations.rs. Cuộc được tạo và nối lượt qua route hỏi (api/ask.ts), nên mỗi lần hỏi xong thì
useAsk làm mới danh sách (conversationsKey).
Lượt đã ghi chỉ có câu hỏi, trạng thái, dòng tóm tắt "Đã hiểu là" và SQL đã chạy, không có bảng kết quả: trang hiện
những gì có và cho hỏi lại, không dựng lại câu trả lời. */

import { useQuery } from '@tanstack/react-query'
import { apiGet } from './client'
import type { components } from './schema'

type Schemas = components['schemas']
export type ConversationItem = Schemas['ConversationItem']
export type ConversationResponse = Schemas['ConversationResponse']
export type TurnItem = Schemas['TurnItem']

function conversationsPath(ws: string, datasetId: string): string {
  return `/workspaces/${encodeURIComponent(ws)}/datasets/${encodeURIComponent(datasetId)}/conversations`
}

export function conversationsKey(ws: string, datasetId: string) {
  return ['workspaces', ws, 'datasets', datasetId, 'conversations'] as const
}

/** Cuộc trò chuyện của người gọi trên dataset; datasetId rỗng thì chưa gọi. */
export function useConversations(ws: string, datasetId: string) {
  return useQuery({
    queryKey: conversationsKey(ws, datasetId),
    queryFn: () =>
      apiGet<Schemas['ConversationListResponse']>(conversationsPath(ws, datasetId)).then((r) => r.conversations),
    enabled: datasetId !== '',
  })
}

/** Một cuộc kèm các lượt đã ghi lúc mở; id null thì chưa gọi. Khóa không nằm dưới conversationsKey và không tự tải
 * lại (staleTime Infinity): trang hiện các lượt hỏi tiếp sau khi mở bằng response của chính chúng, tải lại cuộc thì
 * các lượt đó hiện hai lần. gcTime 0 bỏ bản lưu ngay khi trang thôi mở cuộc, nên mở lại sau đó tải đủ các lượt mới. */
export function useConversation(ws: string, datasetId: string, id: string | null) {
  return useQuery({
    queryKey: ['workspaces', ws, 'datasets', datasetId, 'conversation', id],
    queryFn: () =>
      apiGet<ConversationResponse>(`${conversationsPath(ws, datasetId)}/${encodeURIComponent(id ?? '')}`),
    enabled: datasetId !== '' && id !== null,
    staleTime: Infinity,
    gcTime: 0,
  })
}
