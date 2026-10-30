// SPDX-License-Identifier: Apache-2.0
/* components/metrics/FormulaInput.tsx - phần công thức của form chỉ số: chọn phép tính và cột (tổng, trung bình, đếm
dòng, đếm giá trị khác nhau), hoặc viết SQL cho người biết. Giá trị luôn là biểu thức SQL (value, onChange), nên form
cha không cần biết đang ở cách nào. Công thức không đọc được thành phép tính và cột thì mở sẵn ô SQL. Phép tính đang
chọn giữ ở state riêng: khi không có cột hợp với phép đó, giá trị là chuỗi rỗng (form cha chặn gửi), nhưng ô phép tính
vẫn hiện đúng lựa chọn. SQL đang viết dở cũng được giữ khi quay về cách chọn, để mở lại không mất. */

import { useId, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import { buildExpression, columnsFor, formulaOps, needsColumn, readExpression, type FormulaOp } from './metricFormula'
import { NativeSelect } from './NativeSelect'
import type { ColumnItem } from './types'

type Props = {
  id: string
  value: string
  onChange: (expression: string) => void
  columns: ColumnItem[]
  error?: string
}

export function FormulaInput({ id, value, onChange, columns, error }: Props) {
  const [mode, setMode] = useState<'pick' | 'sql'>(() => (value && !readExpression(value) ? 'sql' : 'pick'))
  const [pickedOp, setPickedOp] = useState<FormulaOp>(() => readExpression(value)?.op ?? 'sum')
  // SQL viết tay không đọc được thành phép tính và cột, giữ lại khi chuyển sang cách chọn để mở lại ô SQL không mất.
  const [savedSql, setSavedSql] = useState('')
  const opId = useId()
  const errorId = `${id}-error`
  const picked = readExpression(value) ?? { op: pickedOp, column: '' }
  const choices = columnsFor(picked.op, columns)
  // Cột đã lưu không còn hợp với phép tính (ví dụ không còn là cột số đo): vẫn hiện tên đó, kèm ghi chú, thay vì để ô
  // chọn hiện "Chọn cột" trong khi công thức gửi đi vẫn dùng cột cũ.
  const savedColumnMissing = picked.column !== '' && !choices.some((c) => c.name === picked.column)

  const pick = (op: FormulaOp, column: string) => {
    setPickedOp(op)
    const allowed = columnsFor(op, columns)
    // Đổi phép tính mà cột đang chọn không hợp với phép mới (ví dụ cột chữ khi chuyển sang tổng) thì lấy cột đầu.
    const next = allowed.some((c) => c.name === column) ? column : (allowed[0]?.name ?? '')
    onChange(needsColumn(op) && !next ? '' : buildExpression(op, next))
  }
  const switchMode = () => {
    if (mode === 'pick') {
      setMode('sql')
      if (savedSql) {
        onChange(savedSql)
        setSavedSql('')
      }
      return
    }
    setMode('pick')
    if (!readExpression(value)) {
      if (value.trim()) setSavedSql(value)
      pick('sum', '')
    }
  }

  return (
    <fieldset className="flex flex-col gap-2" aria-describedby={error ? errorId : undefined}>
      <legend className="mb-2 text-sm font-medium">Cách tính</legend>
      {mode === 'pick' ? (
        <div className="flex flex-wrap gap-3">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor={opId}>Phép tính</Label>
            <NativeSelect
              id={opId}
              value={picked.op}
              onChange={(e) => pick(e.target.value as FormulaOp, picked.column)}
              options={formulaOps.map(({ op, label }) => ({ value: op, label }))}
            />
          </div>
          {needsColumn(picked.op) && (
            <div className="flex min-w-0 flex-1 basis-48 flex-col gap-1.5">
              <Label htmlFor={id}>Cột</Label>
              <NativeSelect
                id={id}
                value={picked.column}
                aria-invalid={error ? true : undefined}
                onChange={(e) => pick(picked.op, e.target.value)}
                options={[
                  { value: '', label: choices.length ? 'Chọn cột' : 'Không có cột hợp với phép tính này' },
                  ...choices.map((c) => ({ value: c.name, label: c.name })),
                  ...(savedColumnMissing
                    ? [{ value: picked.column, label: `${picked.column} (không hợp với phép tính này)` }]
                    : []),
                ]}
              />
            </div>
          )}
        </div>
      ) : (
        <div className="flex flex-col gap-1.5">
          <Label htmlFor={id}>Công thức SQL</Label>
          <Textarea
            id={id}
            value={value}
            rows={2}
            spellCheck={false}
            aria-invalid={error ? true : undefined}
            className="font-mono"
            placeholder='SUM("Order Item Total")'
            onChange={(e) => onChange(e.target.value)}
          />
          <p className="text-xs text-muted-foreground">
            Một hàm gộp trên cột của dataset, tên cột trong ngoặc kép, ví dụ SUM("a") / COUNT(*).
          </p>
        </div>
      )}
      {mode === 'pick' && value && (
        <p className="text-xs text-muted-foreground">
          SQL: <code className="break-all">{value}</code>
        </p>
      )}
      {mode === 'pick' && savedSql && (
        <p className="text-xs text-muted-foreground">
          Công thức SQL đã viết được giữ lại; bấm Viết bằng SQL để quay lại nó.
        </p>
      )}
      <div>
        <Button type="button" variant="outline" size="sm" onClick={switchMode}>
          {mode === 'pick' ? 'Viết bằng SQL' : 'Chọn phép tính và cột'}
        </Button>
      </div>
      {error && (
        <p id={errorId} role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
    </fieldset>
  )
}
