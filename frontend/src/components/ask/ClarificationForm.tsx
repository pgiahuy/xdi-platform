// SPDX-License-Identifier: Apache-2.0
/* components/ask/ClarificationForm.tsx - màn câu hỏi lại khi câu hỏi hiểu được nhiều cách (status needs_choice): mỗi
chỗ mơ hồ một nhóm lựa chọn, chọn đủ mọi nhóm rồi bấm Hỏi lại. Khác chip giả định (bấm là hỏi ngay): có thể có nhiều
chỗ mơ hồ cùng lúc, hỏi lại khi mới chọn một chỗ thì backend lại hỏi chỗ kia. Trang gắn key theo response để lựa chọn
cũ không sót sang lần hỏi lại sau. */

import { useState, type FormEvent } from 'react'
import { Button } from '@/components/ui/button'
import type { ChoiceView } from './types'

type Props = {
  message: string
  clarifications: ChoiceView[]
  onSubmit: (picks: { choiceId: string; optionId: string }[]) => void
  disabled: boolean
}

export function ClarificationForm({ message, clarifications, onSubmit, disabled }: Props) {
  const [picks, setPicks] = useState<Record<string, string>>({})
  const complete = clarifications.every((choice) => picks[choice.id] !== undefined)

  const handleSubmit = (event: FormEvent) => {
    event.preventDefault()
    if (!complete) return
    onSubmit(clarifications.map((choice) => ({ choiceId: choice.id, optionId: picks[choice.id] })))
  }

  return (
    <form
      onSubmit={handleSubmit}
      aria-labelledby="clarification-heading"
      className="flex flex-col gap-4 rounded-lg border p-4"
    >
      <h3 id="clarification-heading" className="text-sm font-semibold">
        {message}
      </h3>
      {clarifications.map((choice) => (
        <fieldset key={choice.id} className="flex flex-col gap-2" disabled={disabled}>
          <legend className="mb-1 text-sm">{choice.label}</legend>
          {choice.options.map((option) => (
            <label key={option.id} className="flex items-start gap-2 text-sm">
              <input
                type="radio"
                name={`clarification-${choice.id}`}
                value={option.id}
                checked={picks[choice.id] === option.id}
                onChange={() => setPicks((current) => ({ ...current, [choice.id]: option.id }))}
                className="mt-0.5 size-4 accent-primary"
              />
              {option.label}
            </label>
          ))}
        </fieldset>
      ))}
      <div>
        <Button type="submit" disabled={disabled || !complete}>
          Hỏi lại
        </Button>
      </div>
    </form>
  )
}
