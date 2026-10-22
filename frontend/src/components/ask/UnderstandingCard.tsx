// SPDX-License-Identifier: Apache-2.0
/* components/ask/UnderstandingCard.tsx - thẻ "Đã hiểu là": câu hỏi được hiểu thành đo gì, lọc gì, nhóm theo gì, sắp
xếp, giới hạn, khoảng thời gian, và các giả định đổi được. Nhãn do backend viết từ câu SQL đã chạy (#35), nên thẻ luôn
khớp với kết quả bên dưới; đoạn SQL của từng mục để pane bằng chứng (#52) hiện. Mục trống thì không hiện dòng. */

import { ChoiceChips } from './ChoiceChips'
import type { UnderstandingView } from './types'

type Props = {
  understanding: UnderstandingView
  onChangeAssumption: (choiceId: string, optionId: string) => void
  disabled: boolean
}

export function UnderstandingCard({ understanding: u, onChangeAssumption, disabled }: Props) {
  const lines: [string, string[]][] = [
    ['Đo', u.measures],
    ['Lọc', u.filters],
    ['Nhóm theo', u.groupBy],
    ['Sắp xếp', u.orderBy],
    ['Giới hạn', u.limit ? [u.limit] : []],
    ['Khoảng thời gian', u.timeRange ? [u.timeRange] : []],
  ]
  return (
    <section aria-labelledby="understanding-heading" className="flex flex-col gap-3 rounded-lg border p-4">
      <h2 id="understanding-heading" className="text-sm font-semibold">
        Đã hiểu là
      </h2>
      <dl className="grid grid-cols-1 gap-x-4 gap-y-1 text-sm sm:grid-cols-[auto_1fr]">
        {lines
          .filter(([, items]) => items.length > 0)
          .map(([title, items]) => (
            <div key={title} className="contents">
              <dt className="text-muted-foreground">{title}</dt>
              <dd className="mb-1 sm:mb-0">{items.join('; ')}</dd>
            </div>
          ))}
      </dl>
      {u.assumptions.length > 0 && (
        <div className="flex flex-col gap-3 border-t pt-3">
          <p className="text-xs text-muted-foreground">Giả định, bấm để đổi rồi hỏi lại</p>
          {u.assumptions.map((choice) => (
            <ChoiceChips key={choice.id} choice={choice} onPick={onChangeAssumption} disabled={disabled} />
          ))}
        </div>
      )}
    </section>
  )
}
