// SPDX-License-Identifier: Apache-2.0
/* components/dataRoles/RuleEditor.tsx - sửa bộ luật của một vai dữ liệu: danh sách luật đang soạn (bỏ được từng luật)
và khung thêm luật chọn dataset, bảng, cột chiều, rồi tick giá trị đã biết của cột hay gõ giá trị khác. Lưu là thay cả
bộ (PUT .../policies). Cột và giá trị của bảng đang chọn do trang tải (onPickTable, columns), vì component không gọi
API. Các luật trên cùng một bảng nối AND: người mang vai phải qua mọi luật của bảng đó. */

import { useId, useState } from 'react'
import { XIcon } from 'lucide-react'
import { FormMessage } from '@/components/forms/FormMessage'
import { NativeSelect } from '@/components/metrics/NativeSelect'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { RuleText } from './RuleText'
import { type ColumnChoice, type DatasetChoice, type RuleDraft, tableLabel } from './types'

type Props = {
  roleName: string
  datasets: DatasetChoice[]
  initialRules: RuleDraft[]
  /** Bảng đang chọn trong khung thêm luật; rỗng là chưa chọn. */
  pickedTableId: string
  onPickTable: (tableId: string) => void
  /** Cột của bảng đang chọn; undefined khi đang tải hay chưa chọn bảng. */
  columns: ColumnChoice[] | undefined
  columnsError?: string
  pending: boolean
  error?: string
  onSave: (rules: RuleDraft[]) => void
  onCancel: () => void
}

export function RuleEditor(props: Props) {
  const { datasets, pickedTableId, columns } = props
  const [rules, setRules] = useState(props.initialRules)
  const [datasetId, setDatasetId] = useState(
    () => datasets.find((d) => d.tables.some((t) => t.id === pickedTableId))?.id ?? '',
  )
  const [column, setColumn] = useState('')
  const [values, setValues] = useState<string[]>([])
  const [otherValue, setOtherValue] = useState('')
  const valuesId = useId()

  const dataset = datasets.find((d) => d.id === datasetId)
  const picked = columns?.find((c) => c.name === column)
  // Giá trị đã tick mà không có trong danh sách (gõ tay) vẫn hiện để bỏ tick được.
  const shownValues = picked ? [...picked.values, ...values.filter((v) => !picked.values.includes(v))] : []
  const duplicate = rules.some((r) => r.tableId === pickedTableId && r.column === column)

  const pickDataset = (id: string) => {
    setDatasetId(id)
    setColumn('')
    setValues([])
    const tables = datasets.find((d) => d.id === id)?.tables ?? []
    props.onPickTable(tables.length === 1 ? tables[0].id : '')
  }
  const pickTable = (id: string) => {
    setColumn('')
    setValues([])
    props.onPickTable(id)
  }
  const toggleValue = (value: string, on: boolean) =>
    setValues((current) => (on ? [...current, value] : current.filter((v) => v !== value)))
  const addOtherValue = () => {
    const value = otherValue.trim()
    if (value && !values.includes(value)) setValues([...values, value])
    setOtherValue('')
  }
  const addRule = () => {
    if (!pickedTableId || !column || values.length === 0 || duplicate) return
    setRules([...rules, { tableId: pickedTableId, column, values }])
    setColumn('')
    setValues([])
  }

  return (
    <div className="flex flex-col gap-4 rounded-xl border bg-card p-4">
      <h4 className="text-sm font-semibold">Luật của vai {props.roleName}</h4>
      {rules.length === 0 ? (
        <p className="text-sm text-muted-foreground">Chưa có luật nào. Vai chưa có luật thì chưa gán được cho ai.</p>
      ) : (
        <ul className="flex flex-col gap-1.5">
          {rules.map((rule) => (
            <li key={`${rule.tableId}/${rule.column}`} className="flex items-start gap-2 text-sm">
              <span className="min-w-0 flex-1">
                <RuleText rule={rule} table={tableLabel(datasets, rule.tableId)} />
              </span>
              <Button
                variant="ghost"
                size="icon"
                aria-label={`Bỏ luật trên cột ${rule.column}`}
                onClick={() => setRules(rules.filter((r) => r !== rule))}
              >
                <XIcon aria-hidden="true" />
              </Button>
            </li>
          ))}
        </ul>
      )}

      <fieldset className="flex flex-col gap-3 rounded-lg border border-dashed p-3">
        <legend className="px-1 text-sm font-medium">Thêm luật</legend>
        <div className="grid gap-2 sm:grid-cols-3">
          <NativeSelect
            aria-label="Dataset"
            value={datasetId}
            onChange={(e) => pickDataset(e.target.value)}
            options={[{ value: '', label: 'Chọn dataset' }, ...datasets.map((d) => ({ value: d.id, label: d.name }))]}
          />
          <NativeSelect
            aria-label="Bảng"
            value={pickedTableId}
            disabled={!dataset}
            onChange={(e) => pickTable(e.target.value)}
            options={[
              { value: '', label: 'Chọn bảng' },
              ...(dataset?.tables ?? []).map((t) => ({ value: t.id, label: t.name })),
            ]}
          />
          <NativeSelect
            aria-label="Cột"
            value={column}
            disabled={!columns || columns.length === 0}
            onChange={(e) => {
              setColumn(e.target.value)
              setValues([])
            }}
            options={[
              { value: '', label: columns?.length === 0 ? 'Bảng không có cột chiều' : 'Chọn cột' },
              ...(columns ?? []).map((c) => ({ value: c.name, label: c.label ? `${c.label} (${c.name})` : c.name })),
            ]}
          />
        </div>
        {props.columnsError && <FormMessage kind="error">{props.columnsError}</FormMessage>}
        {pickedTableId && !columns && !props.columnsError && (
          <p className="text-sm text-muted-foreground">Đang tải cột...</p>
        )}
        {picked && (
          <div className="flex flex-col gap-2">
            <p id={valuesId} className="text-sm">
              Người mang vai chỉ thấy dòng có {picked.label ?? picked.name} là:
            </p>
            {shownValues.length > 0 ? (
              <div role="group" aria-labelledby={valuesId} className="flex max-h-48 flex-wrap gap-x-4 gap-y-1.5 overflow-y-auto">
                {shownValues.map((value) => (
                  <label key={value} className="flex items-center gap-1.5 text-sm">
                    <input
                      type="checkbox"
                      className="size-4 accent-primary"
                      checked={values.includes(value)}
                      onChange={(e) => toggleValue(value, e.target.checked)}
                    />
                    {value}
                  </label>
                ))}
              </div>
            ) : (
              <p className="text-sm text-muted-foreground">Hồ sơ cột không có danh sách giá trị; gõ giá trị bên dưới.</p>
            )}
            <div className="flex flex-wrap gap-2">
              <Input
                aria-label="Giá trị khác"
                placeholder="Giá trị khác"
                className="h-9 max-w-56"
                value={otherValue}
                onChange={(e) => setOtherValue(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    e.preventDefault()
                    addOtherValue()
                  }
                }}
              />
              <Button type="button" variant="outline" onClick={addOtherValue}>
                Thêm giá trị
              </Button>
            </div>
            {duplicate && <FormMessage kind="error">Bảng này đã có luật trên cột này; bỏ luật cũ trước.</FormMessage>}
            <div>
              <Button
                type="button"
                variant="outline"
                aria-disabled={values.length === 0 || duplicate}
                onClick={addRule}
              >
                Thêm luật
              </Button>
            </div>
          </div>
        )}
      </fieldset>

      {props.error && <FormMessage kind="error">{props.error}</FormMessage>}
      <div className="flex flex-wrap gap-2">
        <Button aria-disabled={props.pending} onClick={() => !props.pending && props.onSave(rules)}>
          {props.pending ? 'Đang lưu...' : 'Lưu luật'}
        </Button>
        <Button variant="outline" onClick={props.onCancel}>
          Thôi
        </Button>
      </div>
    </div>
  )
}
