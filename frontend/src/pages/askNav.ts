// SPDX-License-Identifier: Apache-2.0
/* pages/askNav.ts - nối menu "Lịch sử trò chuyện" trên thanh bên với trang hỏi qua đường dẫn. Trang hỏi (AskPage.tsx)
giữ dataset và cuộc đang mở trong đường dẫn (?dataset=...&c=...); file này dựng và đọc đúng hai tham số đó, nên đổi tên
tham số thì sửa cả AskPage.tsx.
Menu mở một cuộc bằng cách chuyển tới đường dẫn của cuộc đó kèm conversationMenuState trong state của lần chuyển trang.
AskPage thấy dấu đó thì dựng lại trang hỏi để đọc dataset và cuộc từ đường dẫn, như khi tải lại trang; các lần trang
hỏi tự ghi đường dẫn không kèm dấu nên không bị dựng lại. */

import { useState } from 'react'
import { matchPath, useLocation, useSearchParams } from 'react-router'
import { workspaceHref, workspaceRoute } from './workspaceNav'

/** Trang hỏi trên một dataset, mở cuộc conversationId; null là cuộc mới. */
export function askHref(ws: string, datasetId: string, conversationId: string | null): string {
  const params = new URLSearchParams({ dataset: datasetId })
  if (conversationId) params.set('c', conversationId)
  return `${workspaceHref(ws, '')}?${params.toString()}`
}

/** state của lần chuyển trang từ menu lịch sử. */
export const conversationMenuState = { fromConversationMenu: true }

export function isFromConversationMenu(state: unknown): boolean {
  return typeof state === 'object' && state !== null && 'fromConversationMenu' in state
}

/** Trang hỏi mở gần nhất từ lúc AppLayout dựng: workspace, dataset trong đường dẫn (null khi đường dẫn không ghi,
 * trang hỏi khi đó dùng dataset tạo gần nhất) và cuộc đang mở (chỉ khi đang ở trang hỏi). ws là undefined khi chưa mở
 * trang hỏi lần nào. */
export type AskSelection = {
  ws: string | undefined
  datasetId: string | null
  conversationId: string | null
}

/** Đọc AskSelection từ đường dẫn; ws là :ws của đường dẫn hiện tại (useParams). Rời trang hỏi thì nhớ dataset của lần
 * cuối ở đó, để menu lịch sử mở từ trang khác vẫn nói về dataset người dùng vừa dùng ở trang hỏi. Gọi ở AppLayout:
 * AppLayout không bị gỡ khi chuyển trang nên state còn giữ. Cập nhật ngay lúc render khi đường dẫn đổi (cách React
 * khuyên cho state suy từ props), như lastWorkspace ở AppLayout. */
export function useAskSelection(ws: string | undefined): AskSelection {
  const { pathname } = useLocation()
  const [params] = useSearchParams()
  // Trang hỏi là trang đầu của workspace: đúng /w/:ws, không phải trang con.
  const askWs = ws !== undefined && matchPath({ path: workspaceRoute, end: true }, pathname) ? ws : undefined
  const datasetId = askWs === undefined ? null : params.get('dataset')
  const [last, setLast] = useState<{ ws: string; datasetId: string | null }>()
  if (askWs !== undefined && (last?.ws !== askWs || last.datasetId !== datasetId)) setLast({ ws: askWs, datasetId })
  if (askWs !== undefined) return { ws: askWs, datasetId, conversationId: params.get('c') }
  return { ws: last?.ws, datasetId: last?.datasetId ?? null, conversationId: null }
}
