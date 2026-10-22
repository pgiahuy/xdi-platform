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
    <section aria-labelledby="answer-summary-heading" className="flex flex-col gap-1">
      <h2 id="answer-summary-heading" className="text-sm font-semibold">
        Giải thích
      </h2>
      <p className="text-sm">{summary}</p>
      {!byModel && (
        <p className="text-xs text-muted-foreground">Câu này viết theo khuôn từ các số đã tính, không phải lời của mô hình.</p>
      )}
    </section>
  )
}
