// SPDX-License-Identifier: Apache-2.0
/* api/conversations.ts - cuộc trò chuyện của người gọi (#164): GET .../catalog/{id}/conversations (mọi cuộc trên các
bảng của một dataset, mới trước, #257) và GET .../datasets/{table_id}/conversations/{conversation_id} (các lượt đã ghi
của một cuộc; route nhận đúng bảng cuộc nằm trên, ConversationItem.table_id), route ở backend/src/api/catalog.rs và
backend/src/api/conversations.rs. Cuộc được tạo và nối lượt qua route hỏi (api/ask.ts), nên mỗi lần hỏi xong thì
useAsk làm mới danh sách của dataset (catalogConversationsKey).
Lượt đã ghi chỉ có câu hỏi, trạng thái, dòng tóm tắt "Cách tính" và SQL đã chạy, không có bảng kết quả: trang hiện
những gì có và cho hỏi lại, không dựng lại câu trả lời. */

import { useQueries, useQuery } from '@tanstack/react-query'
import { catalogPath } from './catalog'
import { apiGet } from './client'
import type { components } from './schema'

type Schemas = components['schemas']
export type ConversationItem = Schemas['ConversationItem']
export type ConversationResponse = Schemas['ConversationResponse']
export type TurnItem = Schemas['TurnItem']

function catalogConversationsPath(ws: string, catalogId: string): string {
  return `${catalogPath(ws, catalogId)}/conversations`
}

// Không đặt dưới catalogKey: chụp lại dataset làm mới danh sách dataset, không cần tải lại mọi danh sách cuộc.
export function catalogConversationsKey(ws: string, catalogId: string) {
  return ['workspaces', ws, 'catalog-conversations', catalogId] as const
}

function fetchConversations(ws: string, catalogId: string): Promise<ConversationItem[]> {
  return apiGet<Schemas['ConversationListResponse']>(catalogConversationsPath(ws, catalogId)).then(
    (r) => r.conversations,
  )
}

/** Cuộc trò chuyện của người gọi trên mọi bảng của dataset; catalogId rỗng thì chưa gọi. */
export function useCatalogConversations(ws: string, catalogId: string) {
  return useQuery({
    queryKey: catalogConversationsKey(ws, catalogId),
    queryFn: () => fetchConversations(ws, catalogId),
    enabled: catalogId !== '',
  })
}

/** Một cuộc trong danh sách của mọi dataset, kèm dataset của nó. */
export type WorkspaceConversation = ConversationItem & { catalogId: string; datasetName: string }

/** Cuộc trò chuyện của người gọi trên mọi dataset của workspace, mới trước: mỗi cuộc thuộc một dataset, nên lịch sử
 * gộp danh sách của từng dataset (cùng khóa catalogConversationsKey, nên hỏi xong làm mới danh sách của dataset đó là
 * đủ). Backend chưa có route liệt kê theo workspace; số dataset của một workspace nhỏ nên mỗi dataset một lần gọi. */
export function useWorkspaceConversations(ws: string, datasets: { id: string; name: string }[]) {
  return useQueries({
    queries: datasets.map((dataset) => ({
      queryKey: catalogConversationsKey(ws, dataset.id),
      queryFn: () => fetchConversations(ws, dataset.id),
    })),
    combine: (results) => ({
      data: results
        .flatMap((result, i) =>
          (result.data ?? []).map((c) => ({ ...c, catalogId: datasets[i].id, datasetName: datasets[i].name })),
        )
        .sort((a, b) => b.created_at.localeCompare(a.created_at)),
      isPending: results.some((result) => result.isPending),
      error: results.find((result) => result.error)?.error ?? null,
    }),
  })
}

/** Một cuộc kèm các lượt đã ghi lúc mở, trên bảng tableId của nó; id null thì chưa gọi. Khóa không nằm dưới
 * catalogConversationsKey và không tự tải
 * lại (staleTime Infinity): trang hiện các lượt hỏi tiếp sau khi mở bằng response của chính chúng, tải lại cuộc thì
 * các lượt đó hiện hai lần. gcTime 0 bỏ bản lưu ngay khi trang thôi mở cuộc, nên mở lại sau đó tải đủ các lượt mới. */
export function useConversation(ws: string, tableId: string, id: string | null) {
  return useQuery({
    queryKey: ['workspaces', ws, 'datasets', tableId, 'conversation', id],
    queryFn: () =>
      apiGet<ConversationResponse>(
        `/workspaces/${encodeURIComponent(ws)}/datasets/${encodeURIComponent(tableId)}/conversations/${encodeURIComponent(id ?? '')}`,
      ),
    enabled: tableId !== '' && id !== null,
    staleTime: Infinity,
    gcTime: 0,
  })
}
