// SPDX-License-Identifier: Apache-2.0
/* components/ask/WhyInsight.tsx - insight của câu hỏi tìm nguyên nhân ở đầu thẻ trả lời: câu chính (summary_title)
rồi mục "Nguyên nhân chính" (summary_points, mỗi ý có đầu ý in đậm). Khi byModel, mô hình viết cả hai phần từ số code
đã tính, và insight đã qua hai cổng của backend (#201): khẳng định của mô hình so với bảng, rồi mô hình chấm so nghĩa
của lời với khẳng định; chỗ mô hình chấm thấy chưa rõ (warnings) hiện ngay dưới. Không có insight của mô hình (mô hình
lỗi, trượt cổng ở mọi lần viết hay chênh lệch bằng 0) thì hiện câu khuôn do code viết, kèm dòng ghi câu viết theo
khuôn. */

import type { SummaryPointView } from './evidenceTypes'
import { UnclearPoints } from './UnclearPoints'

type Props = {
  title: string
  points: SummaryPointView[]
  byModel: boolean
  warnings: string[]
}

export function WhyInsight({ title, points, byModel, warnings }: Props) {
  return (
    // Vạch hổ phách bên trái: insight là câu trả lời chính của câu hỏi tìm nguyên nhân.
    <section aria-labelledby="why-insight-heading" className="flex flex-col gap-2 border-l-[3px] border-highlight pl-3.5">
      <h3 id="why-insight-heading" className="text-[15px] leading-relaxed font-semibold tabular-nums">
        {title}
      </h3>
      {points.length > 0 && (
        <div className="flex flex-col gap-1">
          {byModel && <h4 className="text-xs font-semibold tracking-wider text-label uppercase">Nguyên nhân chính</h4>}
          <Points points={points} />
        </div>
      )}
      {byModel && warnings.length > 0 && <UnclearPoints warnings={warnings} checkAgainst="tab Phân rã" />}
      {!byModel && <p className="text-xs text-muted-foreground">Câu viết theo khuôn từ số đã tính.</p>}
    </section>
  )
}

function Points({ points }: { points: SummaryPointView[] }) {
  return (
    <ul className="flex list-disc flex-col gap-1 pl-5 text-sm leading-relaxed tabular-nums marker:text-muted-foreground">
      {points.map((point, index) => (
        <li key={index}>
          <span className="font-semibold">{point.lead}:</span> {point.text}
        </li>
      ))}
    </ul>
  )
}
