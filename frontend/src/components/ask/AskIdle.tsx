// SPDX-License-Identifier: Apache-2.0
/* components/ask/AskIdle.tsx - chỗ của kết quả khi chưa hỏi gì: một dòng nhạt nói bước tiếp theo (dataset đã chọn ở đầu trang), thay cho khoảng trắng lớn
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
      ? 'Gõ câu hỏi ở ô bên dưới hoặc bấm một câu gợi ý. Kết quả và minh chứng hiện ở đây.'
      : 'Gõ câu hỏi ở ô bên dưới. Kết quả và minh chứng hiện ở đây.'
  // Một dòng chữ nhạt, không khung: khung viền đứt trước đây trông như một ô nhập, lẫn với ô hỏi bên dưới.
  return <p className="text-sm text-muted-foreground">{text}</p>
}
