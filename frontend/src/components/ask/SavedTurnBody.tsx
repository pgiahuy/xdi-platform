// SPDX-License-Identifier: Apache-2.0
/* components/ask/SavedTurnBody.tsx - khối trả lời của một lượt đã lưu khi mở lại cuộc trò chuyện cũ (#164). Lượt đã
lưu chỉ có câu hỏi, trạng thái và dòng tóm tắt "Đã hiểu là" (TurnItem, api/conversations.ts), không có bảng kết quả,
nên khối này nói rõ điều đó và cho hỏi lại câu đó trên dữ liệu hiện tại, thay vì dựng một câu trả lời không có thật. */

import { RotateCcw } from 'lucide-react'
import { Button } from '@/components/ui/button'

type Props = {
  status: 'answered' | 'needs_choice' | 'rejected' | 'failed'
  /** Dòng tóm tắt "Đã hiểu là" của lượt; null khi lượt không có câu trả lời. */
  understood: string | null
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
      {understood && (
        <p>
          <span className="font-semibold text-label">Đã hiểu là: </span>
          {understood}
        </p>
      )}
      <p className="text-muted-foreground">
        {statusText[status]} Kết quả của lượt này không được lưu; bấm Hỏi lại để chạy lại câu hỏi trên dữ liệu hiện tại.
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
