// SPDX-License-Identifier: Apache-2.0
/* pages/AnswerFeedbackBar.tsx - nối hai nút đánh giá (components/ask/AnswerFeedback.tsx) với API (api/feedback.ts) cho
một câu trả lời có query_id. Đánh giá đang hiện giữ trong state của lượt: lượt vừa hỏi bắt đầu chưa có đánh giá, lượt
đã lưu bắt đầu từ TurnItem.feedback; mỗi lần gửi xong thì lấy đánh giá backend trả về, nên nhãn "đã duyệt" mất ngay
khi người hỏi đổi sang "Chưa đạt". */

import { useState } from 'react'
import { useRateAnswer, type QueryFeedback } from '@/api/feedback'
import { AnswerFeedback, type FeedbackView } from '@/components/ask/AnswerFeedback'

type Props = {
  ws: string
  queryId: string
  initial: QueryFeedback | null
}

function toView(feedback: QueryFeedback | null): FeedbackView | null {
  return feedback && { rating: feedback.rating, reason: feedback.reason, approved: feedback.approved }
}

export function AnswerFeedbackBar({ ws, queryId, initial }: Props) {
  const [feedback, setFeedback] = useState(initial)
  const rate = useRateAnswer(ws, queryId)
  const send = (change: Parameters<typeof rate.mutate>[0]) => rate.mutate(change, { onSuccess: setFeedback })
  return (
    <AnswerFeedback
      feedback={toView(feedback)}
      pending={rate.isPending}
      error={rate.error ? rate.error.message : null}
      onRateGood={() => send({ rating: 'good' })}
      onRateBad={(reason, note) => send({ rating: 'bad', reason, note: note.trim() || undefined })}
      onClear={() => send(null)}
    />
  )
}
