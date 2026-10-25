// SPDX-License-Identifier: Apache-2.0
/* components/ask/AskIdle.tsx - chỗ của kết quả khi chưa hỏi gì: một dòng nói bước tiếp theo, thay cho khoảng trắng lớn
dưới ô hỏi. Đang hỏi câu đầu tiên thì báo đang hỏi ở cùng chỗ, để người dùng thấy kết quả sẽ hiện ở đâu. */

type Props = {
  pending: boolean
  /** Dataset đang chọn có câu gợi ý: nhắc bấm thử một câu. */
  hasSuggestions: boolean
}

export function AskIdle({ pending, hasSuggestions }: Props) {
  const text = pending
    ? 'Đang hỏi...'
    : hasSuggestions
      ? 'Chọn dataset, gõ câu hỏi rồi bấm Hỏi, hoặc bấm một câu gợi ý để hỏi ngay. Kết quả và bằng chứng hiện ở đây.'
      : 'Chọn dataset, gõ câu hỏi rồi bấm Hỏi. Kết quả và bằng chứng hiện ở đây.'
  return <p className="rounded-xl border border-dashed px-4 py-6 text-sm text-muted-foreground sm:px-5">{text}</p>
}
