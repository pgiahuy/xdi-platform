// SPDX-License-Identifier: Apache-2.0
/* components/ask/ChoiceChips.tsx - các lựa chọn của một giả định, mỗi lựa chọn một chip; chip đang dùng được tô đậm.
Bấm chip khác là hỏi lại ngay với lựa chọn đó (trang gửi), nên chip là nút có aria-pressed chứ không phải ô radio
trong form cần thêm nút gửi. */

import { useId } from 'react'
import { Button } from '@/components/ui/button'
import type { ChoiceView } from './types'

type Props = {
  choice: ChoiceView
  onPick: (choiceId: string, optionId: string) => void
  disabled: boolean
}

export function ChoiceChips({ choice, onPick, disabled }: Props) {
  // useId thay vì dựng id từ choice.id: id của backend có thể chứa khoảng trắng, aria-labelledby sẽ tách nó thành
  // nhiều id và nhóm chip mất tên.
  const labelId = useId()
  return (
    <div className="flex flex-col gap-1.5">
      <p id={labelId} className="text-sm">
        {choice.label}
      </p>
      <div role="group" aria-labelledby={labelId} className="flex flex-wrap gap-2">
        {choice.options.map((option) => {
          const selected = option.id === choice.selected
          return (
            <Button
              key={option.id}
              type="button"
              size="sm"
              variant={selected ? 'default' : 'outline'}
              aria-pressed={selected}
              disabled={disabled}
              // Bấm lại chip đang dùng thì không hỏi lại: kết quả sẽ y như cũ.
              onClick={() => !selected && onPick(choice.id, option.id)}
              className="h-auto max-w-full py-1 text-left whitespace-normal"
            >
              {option.label}
            </Button>
          )
        })}
      </div>
    </div>
  )
}
