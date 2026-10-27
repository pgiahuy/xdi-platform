// SPDX-License-Identifier: Apache-2.0
/* components/ask/UnderstandingCard.tsx - thẻ "Đã hiểu là": câu hỏi được hiểu thành đo gì, lọc gì, nhóm theo gì, sắp
xếp, giới hạn, khoảng thời gian, và các giả định đổi được. Nhãn do backend viết từ câu SQL đã chạy (#35), nên thẻ luôn
khớp với kết quả bên dưới; đoạn SQL của từng mục để pane bằng chứng (#52) hiện. Mục trống thì không hiện dòng. Mục lấy
từ lượt trước của cuộc trò chuyện (#164) có nhãn chữ "từ câu trước", không chỉ đổi màu, để đọc được cả khi không phân
biệt màu.
Khối nền xanh ngọc nhạt: xanh ngọc là màu của nhãn và cách hiểu, tách khỏi hổ phách dành cho số cần nhấn. */

import { CircleCheck } from 'lucide-react'
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
  const fromPrevious = new Set(u.fromPrevious ?? [])
  return (
    <section
      aria-labelledby="understanding-heading"
      className="flex flex-col gap-3 rounded-[10px] border border-label-border bg-label-soft px-4 py-3.5"
    >
      <h3
        id="understanding-heading"
        className="flex items-center gap-2 text-xs font-semibold tracking-wider text-label uppercase"
      >
        <CircleCheck aria-hidden className="size-4" />
        Đã hiểu là
      </h3>
      <dl className="grid grid-cols-1 gap-x-4 gap-y-1 text-sm sm:grid-cols-[auto_1fr]">
        {lines
          .filter(([, items]) => items.length > 0)
          .map(([title, items]) => (
            <div key={title} className="contents">
              <dt className="font-semibold text-label">{title}</dt>
              <dd className="mb-1 font-medium sm:mb-0">
                {items.map((item, i) => (
                  <span key={`${i}-${item}`}>
                    {i > 0 && '; '}
                    {item}
                    {fromPrevious.has(item) && (
                      <span className="ml-1.5 rounded border border-label-border px-1 py-px text-xs font-normal text-muted-foreground">
                        từ câu trước
                      </span>
                    )}
                  </span>
                ))}
              </dd>
            </div>
          ))}
      </dl>
      {u.assumptions.length > 0 && (
        <div className="flex flex-col gap-3 border-t border-label-border pt-3">
          <p className="text-xs text-muted-foreground">Giả định, bấm để đổi rồi hỏi lại</p>
          {u.assumptions.map((choice) => (
            <ChoiceChips key={choice.id} choice={choice} onPick={onChangeAssumption} disabled={disabled} />
          ))}
        </div>
      )}
    </section>
  )
}
