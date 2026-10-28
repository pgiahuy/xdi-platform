// SPDX-License-Identifier: Apache-2.0
/* components/ask/AnswerSummary.tsx - lời tóm tắt của câu "tại sao" ở đầu thẻ trả lời: câu trả lời chính in đậm
(explanation.summary_title) rồi vài ý ngắn, mỗi ý có đầu ý in đậm (summary_points, ví dụ "Lý do chính: chủ yếu do số
đơn..."). Backend viết các câu này bằng code từ số đã tính (#186), mỗi ý chỉ nói khi số xác nhận, nên luôn hiện; lời của
mô hình (explanation.summary) chỉ diễn đạt lại cùng các ý nên không hiện ở đây. */

import type { SummaryPointView } from './evidenceTypes'

type Props = {
  title: string
  points: SummaryPointView[]
}

export function AnswerSummary({ title, points }: Props) {
  return (
    // Vạch hổ phách bên trái: lời tóm tắt là câu trả lời chính của câu "tại sao".
    <section aria-labelledby="answer-summary-heading" className="flex flex-col gap-1.5 border-l-[3px] border-highlight pl-3.5">
      <h3 id="answer-summary-heading" className="text-[15px] leading-relaxed font-semibold tabular-nums">
        {title}
      </h3>
      {points.length > 0 && (
        <ul className="flex list-disc flex-col gap-1 pl-5 text-sm leading-relaxed tabular-nums marker:text-muted-foreground">
          {points.map((point, index) => (
            <li key={index}>
              <span className="font-semibold">{point.lead}:</span> {point.text}
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}
