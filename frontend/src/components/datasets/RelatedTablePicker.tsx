// SPDX-License-Identifier: Apache-2.0
/* components/datasets/RelatedTablePicker.tsx - chọn bảng chụp kèm khi chụp một bảng Postgres (#165). Bảng liên quan tách
theo chiều của khóa ngoại: bảng mà bảng đang chọn trỏ tới (nhiều-một) và bảng trỏ tới nó (một-nhiều). Mỗi bảng chọn
thành một dataset riêng, khóa ngoại giữa các bảng ghi thành quan hệ giữa các dataset. Bảng không chụp kèm được hiện mờ
kèm lý do backend gửi. Một bảng bước một đã chọn mở thêm được một bước, để chọn nhóm hai bước (order_items -> orders ->
customers). Một bảng có thể hiện ở nhiều chỗ (nối theo hai khóa, hay thấy lại khi mở thêm); mọi ô của nó cùng một trạng
thái. Dữ liệu và lựa chọn do trang dựng (pages/relatedTablesView.ts). */

import { ChevronDown, ChevronRight } from 'lucide-react'
import { useId } from 'react'
import { FormMessage } from '@/components/forms/FormMessage'
import { Button } from '@/components/ui/button'
import type { RelationNode, RelationNodeState, RelationOption } from './types'

type Props = {
  start: RelationNode
  /** Bảng liên quan của các bảng bước một đang mở, theo tên bảng. */
  nested: Record<string, RelationNodeState>
  /** Bảng bước một mở thêm được (đang chọn). */
  expandable: string[]
  /** Bảng chụp kèm, không tính bảng bắt đầu. */
  selected: string[]
  /** Số bảng tối đa một lần chụp, tính cả bảng bắt đầu. */
  max: number
  onToggle: (table: string, checked: boolean) => void
  onExpand: (table: string, open: boolean) => void
}

export function RelatedTablePicker(props: Props) {
  const { start, selected, max } = props
  const total = 1 + selected.length
  const full = total >= max
  return (
    <fieldset className="flex min-w-0 flex-col gap-3">
      <legend className="mb-1 text-sm font-medium">Chụp kèm bảng liên quan</legend>
      <p className="text-xs text-muted-foreground">
        Mỗi bảng chọn thành một dataset riêng; khóa ngoại giữa các bảng được ghi lại. Bảng mà{' '}
        <code className="break-all">{start.label}</code> trỏ tới được chọn sẵn.
      </p>
      <NodeSections node={start} depth={0} picker={props} full={full} />
      <p className="text-sm" aria-live="polite">
        Sẽ chụp {total} bảng thành {total} dataset.
        {full && ` Đã đủ ${max} bảng, mức tối đa của một lần chụp: bỏ chọn một bảng để chọn bảng khác.`}
      </p>
    </fieldset>
  )
}

type NodeProps = {
  node: RelationNode
  /** 0: bảng bắt đầu; 1: bảng mở thêm từ một bảng bước một. */
  depth: number
  picker: Props
  full: boolean
}

/** Hai nhóm bảng liên quan của một bảng; nhóm rỗng không hiện. */
function NodeSections({ node, depth, picker, full }: NodeProps) {
  const id = useId()
  const sections = [
    { key: 'parents', title: `${node.label} trỏ tới (nhiều-một)`, options: node.parents },
    { key: 'children', title: `Trỏ tới ${node.label} (một-nhiều)`, options: node.children },
  ].filter((section) => section.options.length > 0)
  if (sections.length === 0) {
    return (
      <p className="text-sm text-muted-foreground">
        {depth === 0
          ? 'Bảng này không có khóa ngoại nào: chụp riêng bảng.'
          : `${node.label} không nối thêm bảng nào khác.`}
      </p>
    )
  }
  // Bảng nối theo hai khóa hiện hai dòng; nút mở thêm chỉ ở dòng đầu, để không có hai khung giống nhau.
  const seen = new Set<string>()
  const withButton = new Set<string>()
  for (const option of [...node.parents, ...node.children]) {
    if (seen.has(option.table)) continue
    seen.add(option.table)
    withButton.add(option.key)
  }
  return (
    <div className="flex flex-col gap-3">
      {sections.map((section) => (
        <div key={section.key} role="group" aria-labelledby={`${id}-${section.key}`} className="flex flex-col gap-2">
          <p id={`${id}-${section.key}`} className="text-xs font-medium break-words text-muted-foreground">
            {section.title}
          </p>
          <ul className="flex flex-col gap-2">
            {section.options.map((option, index) => (
              <OptionRow
                key={option.key}
                id={`${id}-${section.key}-${index}`}
                option={option}
                canExpand={depth === 0 && withButton.has(option.key)}
                picker={picker}
                full={full}
              />
            ))}
          </ul>
        </div>
      ))}
    </div>
  )
}

type RowProps = {
  id: string
  option: RelationOption
  canExpand: boolean
  picker: Props
  full: boolean
}

function OptionRow({ id, option, canExpand, picker, full }: RowProps) {
  const checked = option.addable && picker.selected.includes(option.table)
  // Đủ giới hạn thì chỉ còn bỏ chọn được; bảng đang chọn vẫn bấm được.
  const disabled = !option.addable || (full && !checked)
  const expandable = canExpand && picker.expandable.includes(option.table)
  const nested = expandable ? picker.nested[option.table] : undefined
  return (
    <li className="min-w-0">
      <div className="flex items-start gap-2">
        <input
          id={id}
          type="checkbox"
          className="mt-1 size-4 shrink-0 accent-primary"
          checked={checked}
          disabled={disabled}
          aria-describedby={option.reason ? `${id}-reason` : undefined}
          onChange={(event) => picker.onToggle(option.table, event.target.checked)}
        />
        <label
          htmlFor={id}
          className={`min-w-0 text-sm break-words ${option.addable ? '' : 'text-muted-foreground'}`}
        >
          <code className="break-all">{option.label}</code> qua{' '}
          <code className="break-all">{option.columns.join(', ')}</code>
          {option.addable && `, ${option.columnCount} cột`}
        </label>
      </div>
      {option.reason && (
        <p id={`${id}-reason`} className="ml-6 text-xs break-words text-muted-foreground">
          {option.reason}
        </p>
      )}
      {expandable && (
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="ml-4 h-auto max-w-full py-1 text-left whitespace-normal"
          aria-expanded={nested !== undefined}
          aria-controls={nested ? `${id}-nested` : undefined}
          onClick={() => picker.onExpand(option.table, nested === undefined)}
        >
          {nested ? <ChevronDown aria-hidden="true" /> : <ChevronRight aria-hidden="true" />}
          <span className="min-w-0 break-words">
            {nested ? 'Ẩn' : 'Xem'} bảng nối với {option.label}
          </span>
        </Button>
      )}
      {nested && (
        <div id={`${id}-nested`} className="mt-2 ml-6 border-l pl-3">
          {nested.kind === 'loading' && <p className="text-sm text-muted-foreground">Đang đọc khóa ngoại...</p>}
          {nested.kind === 'error' && <FormMessage kind="error">{nested.message}</FormMessage>}
          {nested.kind === 'ready' && <NodeSections node={nested.node} depth={1} picker={picker} full={full} />}
        </div>
      )}
    </li>
  )
}
