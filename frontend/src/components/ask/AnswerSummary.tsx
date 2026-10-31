// SPDX-License-Identifier: Apache-2.0
/* components/ask/AnswerSummary.tsx - phần giải thích theo hình dạng kết quả của câu thường (insight, #227); insight của
câu hỏi tìm nguyên nhân hiện ở WhyInsight.tsx. Có lời của mô hình (khi summary_by_model) thì lời đó đứng đầu: mô hình
chỉ diễn đạt lại các ý code đã tính, và lời đã qua hai cổng kiểm của backend (#201). Ngay dưới là mục "Căn cứ": câu
chính (summary_title) và các ý ngắn (summary_points) do code viết, để người đọc đối chiếu lời của mô hình với số đã
tính. Không có lời của mô hình (mô hình lỗi, lời trượt hai cổng ở mọi lần viết, hay INSIGHT_TEXT=false) thì câu chính
và các ý đứng đầu, không kèm dòng ghi chú (tab Các bước ghi lời của mô hình có được dùng không). */

import type { SummaryPointView } from './evidenceTypes'

type Props = {
  title: string
  points: SummaryPointView[]
  modelSummary: string | null
}

export function AnswerSummary({ title, points, modelSummary }: Props) {
  return (
    // Vạch hổ phách bên trái: lời giải thích là phần chính của câu trả lời, tách khỏi bảng và biểu đồ.
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
