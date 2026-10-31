// SPDX-License-Identifier: Apache-2.0
/* components/ask/AnswerText.tsx - đoạn trả lời bằng lời của câu hỏi thường (AskResponse.answer_text): lời mô hình
ngôn ngữ viết lại từ các câu backend đã tính cho phần giải thích, mọi con số trong đoạn đều có trong các câu đó. Khi lời
của mô hình không qua bước kiểm, không gọi được mô hình, câu hỏi là danh sách dòng hay người quản trị tắt ANSWER_TEXT,
backend thay bằng câu viết theo khuôn; ghi rõ điều đó để người đọc biết. */

type Props = {
  text: string
  /** False: câu viết theo khuôn từ số đã tính, không phải lời của mô hình. */
  byModel: boolean
}

export function AnswerText({ text, byModel }: Props) {
  return (
    <div className="flex flex-col gap-1">
      <p className="text-[15px] leading-relaxed tabular-nums">{text}</p>
      {!byModel && <p className="text-xs text-muted-foreground">Câu viết theo khuôn từ số đã tính.</p>}
    </div>
  )
}
