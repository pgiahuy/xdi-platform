// SPDX-License-Identifier: Apache-2.0
/* components/ask/UnclearPoints.tsx - các chỗ mô hình chấm của backend (#201) thấy chưa rõ nghĩa trong lời của mô hình,
hiện ngay dưới lời: insight của câu "tại sao" (WhyInsight, explanation.summary_warnings) và đoạn trả lời của câu thường
(AnswerText, insight.summary_warnings). Chỗ chưa rõ không chắc là sai, nên lời vẫn hiện; khối này báo người đọc đối
chiếu với số đã tính. Kết luận đối soát (khớp, chưa rõ,
không khớp) không hiện ở câu trả lời mà ở bước hai cổng kiểm của tab Các bước (EvidenceSteps). */

import { TriangleAlert } from 'lucide-react'

type Props = {
  warnings: string[]
  /** Nơi người đọc đối chiếu, ví dụ "tab Phân rã". */
  checkAgainst: string
}

// Nền vàng như khối Lưu ý (AskWarnings): người đọc nhận ra ngay đây là cảnh báo, không phải chú thích phụ.
export function UnclearPoints({ warnings, checkAgainst }: Props) {
  return (
    <div
      role="note"
      className="flex gap-2 rounded-lg border border-highlight-border bg-highlight-soft px-3 py-2 text-xs leading-relaxed"
    >
      <TriangleAlert aria-hidden className="mt-0.5 size-3.5 shrink-0 text-highlight-text" />
      <div>
        <p>Có chỗ chưa rõ nghĩa, nên đối chiếu với {checkAgainst}:</p>
        <ul className="list-disc pl-4">
          {warnings.map((warning, index) => (
            <li key={index}>{warning}</li>
          ))}
        </ul>
      </div>
    </div>
  )
}
