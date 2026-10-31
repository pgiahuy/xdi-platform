// SPDX-License-Identifier: Apache-2.0
/* pages/SidebarConversations.tsx - nội dung khung nổi "Lịch sử trò chuyện" trên thanh bên
(components/shell/ConversationFlyout.tsx): mọi cuộc trò chuyện của người dùng trên mọi dataset của workspace (một dataset
là một database hay một file, #257)
(api/conversations.ts, useWorkspaceConversations), mới trước, vẽ bằng ConversationList; mỗi cuộc ghi dataset của nó, vì
mỗi cuộc hỏi trên dataset riêng. Khung chỉ gắn khi mở, nên danh sách chỉ tải khi người dùng mở menu.
Bấm một cuộc thì chuyển tới trang cuộc trò chuyện với ?dataset=<dataset của cuộc>&c=... kèm dấu của menu (askNav.ts),
rồi đóng khung; trang mở cuộc từ đường dẫn. Nút dấu cộng mở cuộc mới trên dataset đang dùng ở trang cuộc trò chuyện
(AppLayout đọc từ đường dẫn, askNav.ts); chưa có hay dataset đó đã bị xóa thì database đầu tiên, đúng quy tắc trang
đó chọn sẵn (askScope.ts). Đang có câu hỏi chờ trả lời thì các nút tắt (useAskPending). */

import { useNavigate } from 'react-router'
import { useAskPending } from '@/api/ask'
import { useWorkspaceConversations } from '@/api/conversations'
import { useCatalog } from '@/api/catalog'
import { ConversationList } from '@/components/ask/ConversationList'
import { askHref, conversationMenuState } from './askNav'
import { resolveAskScope } from './askScope'

type Props = {
  ws: string
  /** Dataset đang dùng ở trang cuộc trò chuyện, cho cuộc mới; null hay không còn thì dùng database đầu tiên. */
  datasetId: string | null
  /** Cuộc đang mở ở trang cuộc trò chuyện; null khi không ở trang đó hay đang ở cuộc mới. */
  activeId: string | null
  /** Đóng khung nổi sau khi chọn. */
  onDone: () => void
}

export function SidebarConversations({ ws, datasetId, activeId, onDone }: Props) {
  const navigate = useNavigate()
  const datasets = useCatalog(ws)
  const asking = useAskPending(ws)
  const all = (datasets.data ?? []).map((d) => ({ id: d.id, name: d.name }))
  const conversations = useWorkspaceConversations(ws, all)
  // Đường dẫn cũ có thể ghi id bảng; resolveAskScope đổi về dataset chứa bảng đó.
  const current = resolveAskScope(datasets.data ?? [], datasetId, null).dataset

  const go = (dataset: string, conversationId: string | null) => {
    void navigate(askHref(ws, dataset, conversationId), { state: conversationMenuState })
    onDone()
  }

  const error = datasets.error
    ? 'Không tải được danh sách dataset.'
    : conversations.error
      ? 'Không tải được danh sách cuộc trò chuyện.'
      : null
  return (
    <ConversationList
      conversations={conversations.data.map((c) => ({
        id: c.id,
        title: c.title,
        datasetName: c.datasetName,
        createdAt: c.created_at,
      }))}
      activeId={activeId}
      loading={datasets.isPending || (all.length > 0 && conversations.isPending)}
      error={error}
      emptyText={current ? 'Chưa có cuộc trò chuyện nào.' : 'Workspace chưa có dataset nào.'}
      onOpen={(id) => {
        const item = conversations.data.find((c) => c.id === id)
        if (item) go(item.catalogId, id)
      }}
      onNew={current ? () => go(current.id, null) : undefined}
      disabled={asking}
    />
  )
}
