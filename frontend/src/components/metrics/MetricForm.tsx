// SPDX-License-Identifier: Apache-2.0
/* components/metrics/MetricForm.tsx - form thêm hay sửa một chỉ số: tên, mô tả, cách tính (FormulaInput), điều kiện,
cột thời gian, đơn vị. Sửa thì tên khóa lại: backend lưu phiên bản mới cùng tên, đổi tên là tạo chỉ số khác. Backend
chạy thử công thức trước khi lưu; lỗi của một trường hiện ngay dưới ô đó và focus về ô (forms/formError.ts). */

import { useState, type FormEvent } from 'react'
import { FormField } from '@/components/forms/FormField'
import { type FormError, useFocusOnError } from '@/components/forms/formError'
import { FormMessage } from '@/components/forms/FormMessage'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { FormulaInput } from './FormulaInput'
import { buildExpression, columnsFor } from './metricFormula'
import { NativeSelect } from './NativeSelect'
import type { ColumnItem, MetricDraft, MetricField, MetricItem } from './types'

const fieldIds: Record<MetricField, string> = {
  name: 'metric-name',
  description: 'metric-description',
  expression: 'metric-expression',
  filter: 'metric-filter',
  time_column: 'metric-time-column',
  unit: 'metric-unit',
}

type Props = {
  columns: ColumnItem[]
  /** Chỉ số đang sửa; không có là thêm mới. */
  editing?: MetricItem
  pending: boolean
  error?: FormError<MetricField>
  onSubmit: (draft: MetricDraft) => void
  onCancel: () => void
}

function buildDraft(columns: ColumnItem[], editing?: MetricItem): MetricDraft {
  if (editing) {
    return {
      name: editing.name,
      description: editing.description,
      expression: editing.expression,
      filter: editing.filter ?? '',
      timeColumn: editing.timeColumn ?? '',
      unit: editing.unit ?? '',
    }
  }
  const first = columnsFor('sum', columns)[0]
  const expression = first ? buildExpression('sum', first.name) : buildExpression('count', '')
  const time = columns.find((c) => c.role === 'time')
  return { name: '', description: '', expression, filter: '', timeColumn: time?.name ?? '', unit: '' }
}

export function MetricForm({ columns, editing, pending, error: serverError, onSubmit, onCancel }: Props) {
  const [draft, setDraft] = useState(() => buildDraft(columns, editing))
  // Lỗi kiểm ngay trên form trước khi gửi (công thức rỗng khi phép tính không có cột hợp); có thì hiện thay lỗi backend.
  const [localError, setLocalError] = useState<FormError<MetricField>>()
  const error = localError ?? serverError
  useFocusOnError(error, fieldIds)
  const set = (key: keyof MetricDraft) => (value: string) => setDraft((d) => ({ ...d, [key]: value }))
  const errorOf = (field: MetricField) => (error?.field === field ? error.message : undefined)
  const timeColumns = columns.filter((c) => c.role === 'time')
  // Cột thời gian đã lưu mà không còn là cột thời gian (hay không còn trong dataset): vẫn hiện tên đó, kèm ghi chú, thay
  // vì để ô chọn hiện "Không có" trong khi giá trị gửi đi vẫn là cột cũ.
  const timeColumnMissing = draft.timeColumn !== '' && !timeColumns.some((c) => c.name === draft.timeColumn)

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (pending) return
    if (!draft.expression.trim()) {
      setLocalError({ field: 'expression', message: 'Chọn cột cho phép tính, hoặc viết công thức bằng SQL' })
      return
    }
    setLocalError(undefined)
    onSubmit(draft)
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-4 rounded-xl border bg-card p-4 shadow-xs">
      <h3 className="text-sm font-semibold">{editing ? `Sửa chỉ số ${editing.name}` : 'Thêm chỉ số'}</h3>
      <FormField
        id={fieldIds.name}
        label="Tên"
        required
        disabled={Boolean(editing)}
        hint={editing ? 'Không đổi tên được; muốn tên khác thì thêm chỉ số mới rồi xóa chỉ số này.' : 'Tên người dùng gõ khi hỏi, ví dụ Doanh thu.'}
        error={errorOf('name')}
        value={draft.name}
        onChange={(e) => set('name')(e.target.value)}
      />
      <FormField
        id={fieldIds.description}
        label="Mô tả"
        hint="Hiện ở dòng Đo của thẻ Cách tính và gửi kèm cho mô hình."
        error={errorOf('description')}
        value={draft.description}
        onChange={(e) => set('description')(e.target.value)}
      />
      <FormulaInput
        id={fieldIds.expression}
        value={draft.expression}
        onChange={(expression) => {
          setLocalError(undefined)
          set('expression')(expression)
        }}
        columns={columns}
        error={errorOf('expression')}
      />
      <FormField
        id={fieldIds.filter}
        label="Điều kiện (SQL, không bắt buộc)"
        className="h-9 font-mono"
        placeholder={`"Order Status" <> 'CANCELED'`}
        hint="Chỉ tính các dòng thỏa điều kiện này."
        error={errorOf('filter')}
        value={draft.filter}
        onChange={(e) => set('filter')(e.target.value)}
      />
      <div className="flex flex-wrap gap-3">
        <div className="flex min-w-0 flex-1 basis-48 flex-col gap-1.5">
          <Label htmlFor={fieldIds.time_column}>Cột thời gian</Label>
          <NativeSelect
            id={fieldIds.time_column}
            value={draft.timeColumn}
            aria-invalid={errorOf('time_column') ? true : undefined}
            onChange={(e) => set('timeColumn')(e.target.value)}
            options={[
              { value: '', label: 'Không có' },
              ...timeColumns.map((c) => ({ value: c.name, label: c.name })),
              ...(timeColumnMissing
                ? [{ value: draft.timeColumn, label: `${draft.timeColumn} (không phải cột thời gian)` }]
                : []),
            ]}
          />
          {errorOf('time_column') && (
            <p role="alert" className="text-sm text-destructive">
              {errorOf('time_column')}
            </p>
          )}
        </div>
        <div className="min-w-0 flex-1 basis-32">
          <FormField
            id={fieldIds.unit}
            label="Đơn vị"
            placeholder="USD, đơn"
            error={errorOf('unit')}
            value={draft.unit}
            onChange={(e) => set('unit')(e.target.value)}
          />
        </div>
      </div>
      {error && !error.field && <FormMessage kind="error">{error.message}</FormMessage>}
      <div className="flex flex-wrap gap-2">
        <Button type="submit" aria-disabled={pending}>
          {pending ? 'Đang kiểm và lưu...' : editing ? 'Lưu phiên bản mới' : 'Thêm chỉ số'}
        </Button>
        <Button type="button" variant="outline" onClick={onCancel}>
          Thôi
        </Button>
      </div>
    </form>
  )
}
