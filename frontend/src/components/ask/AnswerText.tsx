// SPDX-License-Identifier: Apache-2.0
/* components/ask/AnswerText.tsx - đoạn trả lời bằng lời của câu hỏi thường (AskResponse.answer_text): lời mô hình ngôn
ngữ viết lại từ các câu backend đã tính cho phần giải thích, đã qua hai cổng kiểm (#201): code kiểm số và chiều, mô
hình chấm so nghĩa với các câu đó. Chỗ mô hình chấm thấy chưa rõ (insight.summary_warnings) hiện ngay dưới, nền vàng.
Khi lời của mô hình không qua hai cổng, không gọi được mô hình, câu hỏi là danh sách dòng hay người quản trị tắt
ANSWER_TEXT, backend thay bằng câu viết theo khuôn, không kèm dòng ghi chú (tab Các bước ghi lời của mô hình có được
dùng không). */

import { UnclearPoints } from './UnclearPoints'

type Props = {
  text: string
  /** False: câu viết theo khuôn từ số đã tính, không phải lời của mô hình. */
  byModel: boolean
  warnings: string[]
}

export function AnswerText({ text, byModel, warnings }: Props) {
  return (
    <div className="flex flex-col gap-1">
      <p className="text-[15px] leading-relaxed tabular-nums">{text}</p>
      {byModel && warnings.length > 0 && <UnclearPoints warnings={warnings} checkAgainst="các số đã tính bên dưới" />}
    </div>
  )
}
