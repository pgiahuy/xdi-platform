// SPDX-License-Identifier: Apache-2.0
/* components/ask/ClarificationForm.tsx - màn câu hỏi lại khi câu hỏi hiểu được nhiều cách (status needs_choice). Chỉ
một chỗ cần chọn (trường hợp thường gặp: mô hình hỏi lại một câu) thì mỗi phương án là một nút, bấm là hỏi lại ngay
với phương án đó, không phải chọn rồi bấm thêm nút. Phương án do backend lọc: chỉ cột, chỉ số hay kỳ có thật
(ask/choices.rs), nên bấm phương án nào cũng chạy được. Nhiều chỗ cùng lúc thì chọn đủ mọi nhóm rồi bấm Hỏi lại, vì hỏi
lại khi mới chọn một chỗ thì backend lại hỏi chỗ kia. Trang gắn key theo response để lựa chọn cũ không sót sang lần hỏi
lại sau. */

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

  if (clarifications.length === 1) {
    const [choice] = clarifications
    return (
      <section aria-labelledby="clarification-heading" className="flex flex-col gap-3 rounded-lg border p-4">
        <h3 id="clarification-heading" className="text-sm font-semibold">
          {message}
        </h3>
        {choice.label !== message && <p className="text-sm text-muted-foreground">{choice.label}</p>}
        <div className="flex flex-wrap gap-2">
          {choice.options.map((option) => (
            <Button
              key={option.id}
              type="button"
              variant="outline"
              size="sm"
              disabled={disabled}
              onClick={() => onSubmit([{ choiceId: choice.id, optionId: option.id }])}
            >
              {option.label}
            </Button>
          ))}
        </div>
      </section>
    )
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
