// SPDX-License-Identifier: Apache-2.0
/* components/ask/AnswerSummary.tsx - lời giải thích của câu hỏi tìm nguyên nhân ở đầu thẻ trả lời. Có lời của mô hình
(explanation.summary khi summary_by_model, #186) thì lời đó đứng đầu: mô hình chỉ diễn đạt lại các ý code đã tính, và
backend đã kiểm mọi số cùng chữ tăng, giảm trong đó. Ngay dưới là mục "Căn cứ": câu chính (summary_title) và các ý ngắn
(summary_points) do code viết, để người đọc đối chiếu lời của mô hình với số đã tính. Không có lời của mô hình (so hai
nhóm, mô hình lỗi hay viết sai) thì câu chính và các ý đứng đầu, kèm dòng ghi câu viết theo khuôn. */

import type { SummaryPointView } from './evidenceTypes'

type Props = {
  title: string
  points: SummaryPointView[]
  modelSummary: string | null
}

export function AnswerSummary({ title, points, modelSummary }: Props) {
  return (
    // Vạch hổ phách bên trái: lời giải thích là câu trả lời chính của câu hỏi tìm nguyên nhân.
    <section aria-labelledby="answer-summary-heading" className="flex flex-col gap-2 border-l-[3px] border-highlight pl-3.5">
      {modelSummary ? (
        <>
          <p id="answer-summary-heading" className="text-[15px] leading-relaxed tabular-nums">
            {modelSummary}
          </p>
          <div className="flex flex-col gap-1">
            <h4 className="text-xs font-semibold tracking-wider text-label uppercase">Căn cứ</h4>
            <Facts title={title} points={points} />
          </div>
        </>
      ) : (
        <>
          <h3 id="answer-summary-heading" className="text-[15px] leading-relaxed font-semibold tabular-nums">
            {title}
          </h3>
          <Facts points={points} />
          <p className="text-xs text-muted-foreground">Câu viết theo khuôn từ số đã tính.</p>
        </>
      )}
    </section>
  )
}

// Câu chính (khi lời của mô hình đứng đầu) và các ý ngắn, mỗi ý có đầu ý in đậm.
function Facts({ title, points }: { title?: string; points: SummaryPointView[] }) {
  return (
    <>
      {title && <p className="text-sm leading-relaxed font-semibold tabular-nums">{title}</p>}
      {points.length > 0 && (
        <ul className="flex list-disc flex-col gap-1 pl-5 text-sm leading-relaxed tabular-nums marker:text-muted-foreground">
          {points.map((point, index) => (
            <li key={index}>
              <span className="font-semibold">{point.lead}:</span> {point.text}
            </li>
          ))}
        </ul>
      )}
    </>
  )
}
