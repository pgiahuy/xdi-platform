// SPDX-License-Identifier: Apache-2.0
/* components/ask/AskIdle.tsx - chỗ của kết quả khi cuộc còn trống (mới vào hệ thống hay mở cuộc mới): một câu mời hỏi
ngắn, dùng được mọi lúc chứ không phải lời chào mỗi lần mở cuộc, kèm một dòng nhạt nói cách hỏi. Đang hỏi câu đầu tiên
thì chỉ báo đang hỏi ở cùng chỗ, để người dùng thấy kết quả sẽ hiện ở đâu. */

type Props = {
  pending: boolean
  /** Dataset đang chọn có câu gợi ý: nhắc bấm thử một câu. */
  hasSuggestions: boolean
}

export function AskIdle({ pending, hasSuggestions }: Props) {
  const text = pending
    ? 'Đang hỏi...'
    : hasSuggestions
      ? 'Gõ câu hỏi ở ô bên dưới hoặc chọn một câu gợi ý.'
      : 'Gõ câu hỏi ở ô bên dưới.'
  // Chữ nhạt, không khung: khung viền đứt trước đây trông như một ô nhập, lẫn với ô hỏi bên dưới.
  if (pending) return <p className="text-sm text-muted-foreground">{text}</p>
  return (
    <div className="flex flex-col gap-1">
      <h2 className="text-xl font-semibold">Bạn muốn tìm hiểu gì từ dữ liệu?</h2>
      <p className="text-sm text-muted-foreground">{text}</p>
    </div>
  )
}
