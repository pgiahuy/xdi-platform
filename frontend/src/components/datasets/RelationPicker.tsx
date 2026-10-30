// SPDX-License-Identifier: Apache-2.0
/* components/datasets/RelationPicker.tsx - chọn bảng nối nhiều-một khi chụp một bảng Postgres (#165). Mỗi khóa ngoại rời
bảng chính là một ô chọn; bước 2 (bảng nối tiếp từ bảng vừa nối) thụt vào và chỉ chọn được khi bước 1 của nó đang chọn.
Khóa ngoại không nối được, và khóa ngoại của bảng khác trỏ vào bảng chính (một-nhiều), hiện mờ kèm lý do backend gửi.
Dòng cuối cho biết dataset sẽ có bao nhiêu cột. Dữ liệu do trang truyền vào (pages/DatabaseSources.tsx). */

import { useId } from 'react'
import type { RelationOption } from './types'

type Props = {
  manyToOne: RelationOption[]
  oneToMany: RelationOption[]
  /** Số cột đọc được của bảng chính. */
  factColumns: number
  selected: string[]
  onToggle: (relation: RelationOption, checked: boolean) => void
}

function keyText(columns: string[]): string {
  return columns.join(', ')
}

export function RelationPicker({ manyToOne, oneToMany, factColumns, selected, onToggle }: Props) {
  const id = useId()
  const chosen = manyToOne.filter((r) => selected.includes(r.id))
  const added = chosen.reduce((sum, r) => sum + r.columnCount, 0)
  if (manyToOne.length === 0 && oneToMany.length === 0) {
    return <p className="text-sm text-muted-foreground">Bảng này không có khóa ngoại nào: chụp riêng bảng.</p>
  }
  return (
    <fieldset className="flex flex-col gap-2">
      <legend className="mb-1 text-sm font-medium">Nối thêm bảng theo khóa ngoại</legend>
      <p className="text-xs text-muted-foreground">
        Chỉ nối nhiều-một: mỗi dòng của bảng chính khớp tối đa một dòng bảng kia, nên số dòng giữ nguyên. Cột của bảng
        nối có tên dạng bảng.cột.
      </p>
      {manyToOne.map((relation) => {
        const parentChosen = relation.parent === null || selected.includes(relation.parent)
        const enabled = relation.joinable && parentChosen
        const inputId = `${id}-${relation.id}`
        return (
          <div key={relation.id} className={relation.depth > 1 ? 'ml-6' : undefined}>
            <div className="flex items-start gap-2">
              <input
                id={inputId}
                type="checkbox"
                className="mt-1"
                checked={enabled && selected.includes(relation.id)}
                disabled={!enabled}
                onChange={(event) => onToggle(relation, event.target.checked)}
              />
              <label htmlFor={inputId} className={enabled ? 'text-sm' : 'text-sm text-muted-foreground'}>
                <code className="break-all">{relation.toTable}</code> theo{' '}
                <code className="break-all">{keyText(relation.fromColumns)}</code>
                {relation.depth > 1 && (
                  <>
                    {' '}
                    của <code className="break-all">{relation.fromTable}</code>
                  </>
                )}
                {relation.joinable && <>, thêm {relation.columnCount} cột</>}
              </label>
            </div>
            {relation.reason && <p className="ml-6 text-xs text-muted-foreground">{relation.reason}</p>}
          </div>
        )
      })}
      {oneToMany.length > 0 && (
        <div className="flex flex-col gap-1 pt-1">
          <p className="text-sm font-medium text-muted-foreground">Không nối (một-nhiều)</p>
          {oneToMany.map((relation) => (
            <p key={relation.id} className="text-xs text-muted-foreground">
              <code className="break-all">{relation.fromTable}</code> ({keyText(relation.fromColumns)}): {relation.reason}
            </p>
          ))}
        </div>
      )}
      <p className="text-sm" aria-live="polite">
        Dataset sẽ có {factColumns + added} cột: {factColumns} của bảng chính, {added} từ {chosen.length} bảng nối.
      </p>
    </fieldset>
  )
}
