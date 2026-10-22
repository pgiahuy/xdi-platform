// SPDX-License-Identifier: Apache-2.0
/* components/datasets/SheetPicker.tsx - chọn sheet của một file Excel trước khi nạp.
Mỗi sheet hiện cột và số dòng ước lượng để người dùng nhận ra sheet nào là bảng dữ liệu; sheet ẩn được ghi rõ vì người
dùng thường không biết file có sheet ẩn. Các sheet chọn được gom theo bộ cột (sheetGroups.ts); có hơn một nhóm thì báo
trước sẽ tạo bao nhiêu dataset, để người dùng không bất ngờ khi thấy nhiều dataset xuất hiện. */

import { useId } from 'react'
import type { SheetGroup } from './sheetGroups'
import type { SheetOption } from './types'

type Props = {
  sheets: SheetOption[]
  selected: Set<string>
  groups: SheetGroup[]
  onToggle: (sheet: string) => void
  disabled: boolean
}

/** Số cột hiện ra mỗi sheet; còn lại ghi "và N cột khác", để một sheet trăm cột không đẩy danh sách dài cả trang. */
const SHOWN_COLUMNS = 8

const numberFormat = new Intl.NumberFormat('vi-VN')

export function SheetPicker({ sheets, selected, groups, onToggle, disabled }: Props) {
  // id theo thứ tự sheet, không theo tên: tên sheet có thể chứa khoảng trắng, mà id của thẻ HTML thì không được.
  const baseId = useId()
  return (
    <fieldset className="flex flex-col gap-2" disabled={disabled}>
      <legend className="mb-1 text-sm font-medium">Chọn sheet cần nạp</legend>
      <ul className="flex flex-col gap-2">
        {sheets.map((sheet, index) => {
          const empty = sheet.columns.length === 0
          const id = `${baseId}-${index}`
          return (
            <li key={sheet.name} className="flex gap-3 rounded-md border px-3 py-2">
              <input
                id={id}
                type="checkbox"
                className="mt-1 size-4 shrink-0 accent-primary"
                checked={selected.has(sheet.name)}
                disabled={empty}
                onChange={() => onToggle(sheet.name)}
                aria-describedby={`${id}-detail`}
              />
              <div className="min-w-0 flex-1">
                <label htmlFor={id} className="text-sm font-medium break-words">
                  {sheet.name}
                  {sheet.hidden && <span className="ml-2 text-xs font-normal text-muted-foreground">(sheet ẩn)</span>}
                </label>
                <p id={`${id}-detail`} className="text-xs break-words text-muted-foreground">
                  {empty ? 'Sheet trống, không có gì để nạp' : sheetDetail(sheet)}
                </p>
              </div>
            </li>
          )
        })}
      </ul>
      {groups.length > 1 && (
        <div className="rounded-md bg-muted px-3 py-2 text-sm">
          <p>Các sheet đã chọn khác cột nhau, sẽ nạp thành {groups.length} dataset riêng:</p>
          <ul className="mt-1 list-disc pl-5">
            {groups.map((group) => (
              <li key={group.sheets.join('\u0000')} className="break-words">
                {group.sheets.join(', ')}
              </li>
            ))}
          </ul>
        </div>
      )}
    </fieldset>
  )
}

function sheetDetail(sheet: SheetOption): string {
  const shown = sheet.columns.slice(0, SHOWN_COLUMNS).join(', ')
  const more = sheet.columns.length - SHOWN_COLUMNS
  const columns = more > 0 ? `${shown} và ${more} cột khác` : shown
  // approx_rows là số file Excel tự ghi, có thể tính cả dòng trống; ghi "khoảng" để không ai lấy làm số dòng thật.
  const rows = sheet.approxRows > 0 ? `khoảng ${numberFormat.format(sheet.approxRows)} dòng` : 'chưa rõ số dòng'
  return `${sheet.columns.length} cột, ${rows}: ${columns}`
}
