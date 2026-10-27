// SPDX-License-Identifier: Apache-2.0
/* components/ask/AnswerSummary.tsx - lời giải thích của câu "tại sao" (explanation.summary): mô hình ngôn ngữ viết từ
bảng phân rã mà code đã tính, mọi con số trong lời đều có trong bảng phân rã (#41). Khi lời của mô hình không qua được
bước so số, hoặc không gọi được mô hình, backend thay bằng câu viết theo khuôn; ghi rõ điều đó để người đọc biết. */

type Props = {
  summary: string
  /** False: câu viết theo khuôn từ số đã tính, không phải lời của mô hình. */
  byModel: boolean
}

export function AnswerSummary({ summary, byModel }: Props) {
  return (
    // Vạch hổ phách bên trái: lời giải thích là câu trả lời chính của câu "tại sao".
    <section aria-labelledby="answer-summary-heading" className="flex flex-col gap-1 border-l-[3px] border-highlight pl-3.5">
      <h3 id="answer-summary-heading" className="text-xs font-semibold tracking-wider text-label uppercase">
        Giải thích
      </h3>
      <p className="text-[15px] leading-relaxed tabular-nums">{summary}</p>
      {!byModel && (
        <p className="text-xs text-muted-foreground">Câu viết theo khuôn từ số đã tính.</p>
      )}
    </section>
  )
}
