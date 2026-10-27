// SPDX-License-Identifier: Apache-2.0
/* components/ask/SavedTurnBody.tsx - khối trả lời của một lượt đã lưu khi mở lại cuộc trò chuyện cũ (#164). Lượt đã
lưu chỉ có câu hỏi, trạng thái và dòng tóm tắt "Đã hiểu là" (TurnItem, api/conversations.ts), không có bảng kết quả,
nên khối này nói rõ điều đó và cho hỏi lại câu đó trên dữ liệu hiện tại, thay vì dựng một câu trả lời không có thật. */

import { RotateCcw } from 'lucide-react'
import { Button } from '@/components/ui/button'

type Props = {
  status: 'answered' | 'needs_choice' | 'rejected' | 'failed'
  /** Các dòng "Đã hiểu là" của lượt; rỗng khi lượt không có câu trả lời. Dòng không nhãn hiện nguyên chữ. */
  understood: { label: string | null; text: string }[]
  onAskAgain: () => void
  disabled: boolean
}

const statusText = {
  answered: 'Đã trả lời.',
  needs_choice: 'Lúc đó cần chọn cách hiểu.',
  rejected: 'Lúc đó câu SQL bị từ chối.',
  failed: 'Lúc đó không chạy được câu SQL.',
}

export function SavedTurnBody({ status, understood, onAskAgain, disabled }: Props) {
  return (
    <div className="flex flex-col gap-3 text-sm">
      {understood.length > 0 && (
        <div className="flex flex-col gap-1">
          <p className="text-xs font-semibold tracking-wider text-label uppercase">Đã hiểu là</p>
          <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1">
            {understood.map((line, index) =>
              line.label ? (
                <div key={index} className="contents">
                  <dt className="font-semibold text-label">{line.label}</dt>
                  <dd className="wrap-break-word">{line.text}</dd>
                </div>
              ) : (
                <dd key={index} className="col-span-2 wrap-break-word">
                  {line.text}
                </dd>
              ),
            )}
          </dl>
        </div>
      )}
      <p className="text-muted-foreground">
        {statusText[status]}{' '}
        {status === 'answered'
          ? 'Kết quả của lượt này không được lưu; bấm Hỏi lại để chạy lại câu hỏi trên dữ liệu hiện tại.'
          : 'Bấm Hỏi lại để hỏi lại câu này trên dữ liệu hiện tại.'}
      </p>
      <div>
        <Button variant="outline" size="sm" onClick={onAskAgain} disabled={disabled}>
          <RotateCcw aria-hidden />
          Hỏi lại
        </Button>
      </div>
    </div>
  )
}
