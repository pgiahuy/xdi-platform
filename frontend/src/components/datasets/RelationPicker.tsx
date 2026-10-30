// SPDX-License-Identifier: Apache-2.0
/* components/datasets/RelationPicker.tsx - chọn bảng chụp kèm khi chụp một bảng Postgres (#165). Mỗi bảng nối với bảng
đang chọn theo một khóa ngoại là một ô chọn; mỗi bảng chọn thành một dataset riêng, khóa ngoại ghi thành quan hệ giữa các
dataset. Bảng không chụp kèm được hiện mờ kèm lý do backend gửi. Dữ liệu do trang truyền vào (pages/DatabaseSources.tsx).
Bản này còn đơn giản: chưa tách hai chiều, chưa đi tiếp từ bảng đã chọn. */

import { useId } from 'react'
import type { RelationOption } from './types'

type Props = {
  /** Bảng đang chọn để chụp. */
  table: string
  options: RelationOption[]
  selected: string[]
  onToggle: (option: RelationOption, checked: boolean) => void
}

export function RelationPicker({ table, options, selected, onToggle }: Props) {
  const id = useId()
  if (options.length === 0) {
    return <p className="text-sm text-muted-foreground">Bảng này không có khóa ngoại nào: chụp riêng bảng.</p>
  }
  return (
    <fieldset className="flex flex-col gap-2">
      <legend className="mb-1 text-sm font-medium">Chụp kèm bảng liên quan</legend>
      <p className="text-xs text-muted-foreground">
        Mỗi bảng chọn thành một dataset riêng; khóa ngoại giữa các bảng được ghi lại.
      </p>
      {options.map((option, index) => {
        const inputId = `${id}-${index}`
        // Khóa ngoại của bảng đang chọn thì ghi cột của nó; khóa của bảng kia trỏ vào thì ghi "trỏ vào".
        const via =
          option.fromTable === table
            ? `theo ${option.fromColumns.join(', ')}`
            : `trỏ vào bảng này theo ${option.fromColumns.join(', ')}`
        return (
          <div key={`${option.table}-${option.fromTable}-${option.fromColumns.join(',')}`}>
            <div className="flex items-start gap-2">
              <input
                id={inputId}
                type="checkbox"
                className="mt-1"
                checked={option.addable && selected.includes(option.table)}
                disabled={!option.addable}
                onChange={(event) => onToggle(option, event.target.checked)}
              />
              <label htmlFor={inputId} className={option.addable ? 'text-sm' : 'text-sm text-muted-foreground'}>
                <code className="break-all">{option.table}</code> {via}
                {option.addable && <>, {option.columnCount} cột</>}
              </label>
            </div>
            {option.reason && <p className="ml-6 text-xs text-muted-foreground">{option.reason}</p>}
          </div>
        )
      })}
      <p className="text-sm" aria-live="polite">
        Sẽ chụp {1 + selected.length} bảng thành {1 + selected.length} dataset.
      </p>
    </fieldset>
  )
}
