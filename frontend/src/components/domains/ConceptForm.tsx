// SPDX-License-Identifier: Apache-2.0
/* components/domains/ConceptForm.tsx - form thêm hay sửa một khái niệm ngành: tên, mô tả, từ đồng nghĩa và chiều phân
rã (ngăn bằng dấu phẩy), tối đa ba cách tính bằng lời. Backend kiểm trước khi lưu (tên và từ không trùng trong gói,
không xuống dòng); lỗi của một trường hiện ngay dưới ô đó và focus về ô (forms/formError.ts). Luật kiểm hợp lý không
sửa ở đây: trang giữ nguyên và gửi lại. */

import { Plus, X } from 'lucide-react'
import { useState, type FormEvent } from 'react'
import { FormField } from '@/components/forms/FormField'
import { type FormError, useFocusOnError } from '@/components/forms/formError'
import { FormMessage } from '@/components/forms/FormMessage'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import type { CalculationItem, ConceptDraft, ConceptField, ConceptItem } from './types'

// Cùng trần với backend (service/domains/check.rs).
const CALCULATIONS_MAX = 3

const fieldIds: Record<ConceptField, string> = {
  name: 'concept-name',
  description: 'concept-description',
  synonyms: 'concept-synonyms',
  dimensions: 'concept-dimensions',
  calculations: 'concept-calculation-0',
  sanity_checks: 'concept-name',
}

type Props = {
  packName: string
  /** Khái niệm đang sửa; không có là thêm mới. */
  editing?: ConceptItem
  pending: boolean
  error?: FormError<ConceptField>
  onSubmit: (draft: ConceptDraft) => void
  onCancel: () => void
}

function buildDraft(editing?: ConceptItem): ConceptDraft {
  if (!editing) return { name: '', description: '', synonyms: '', dimensions: '', calculations: [] }
  return {
    name: editing.name,
    description: editing.description,
    synonyms: editing.synonyms.join(', '),
    dimensions: editing.dimensions.join(', '),
    calculations: editing.calculations,
  }
}

export function ConceptForm({ packName, editing, pending, error, onSubmit, onCancel }: Props) {
  const [draft, setDraft] = useState(() => buildDraft(editing))
  useFocusOnError(error, fieldIds)
  const errorOf = (field: ConceptField) => (error?.field === field ? error.message : undefined)
  const set = (key: 'name' | 'description' | 'synonyms' | 'dimensions') => (value: string) =>
    setDraft((d) => ({ ...d, [key]: value }))
  const setCalculation = (index: number, change: Partial<CalculationItem>) =>
    setDraft((d) => ({
      ...d,
      calculations: d.calculations.map((c, i) => (i === index ? { ...c, ...change } : c)),
    }))
  const removeCalculation = (index: number) =>
    setDraft((d) => ({ ...d, calculations: d.calculations.filter((_, i) => i !== index) }))
  const addCalculation = () =>
    setDraft((d) => ({ ...d, calculations: [...d.calculations, { name: '', description: '' }] }))

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (!pending) onSubmit(draft)
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-4 rounded-xl border bg-card p-4 shadow-xs">
      <h4 className="text-sm font-semibold">
        {editing ? `Sửa khái niệm ${editing.name}` : `Thêm khái niệm vào gói ${packName}`}
      </h4>
      <FormField
        id={fieldIds.name}
        label="Tên"
        required
        hint="Tên người dùng hay gọi, ví dụ Tỷ lệ đạt."
        error={errorOf('name')}
        value={draft.name}
        onChange={(e) => set('name')(e.target.value)}
      />
      <div className="flex flex-col gap-1.5">
        <Label htmlFor={fieldIds.description}>Mô tả</Label>
        <Textarea
          id={fieldIds.description}
          rows={3}
          required
          aria-invalid={errorOf('description') ? true : undefined}
          value={draft.description}
          onChange={(e) => set('description')(e.target.value)}
        />
        {errorOf('description') && (
          <p role="alert" className="text-sm text-destructive">
            {errorOf('description')}
          </p>
        )}
        <p className="text-xs text-muted-foreground">
          Khái niệm nói về điều gì, tính trên tập nào. Mô hình đọc câu này khi gặp tên hay từ đồng nghĩa trong câu hỏi.
        </p>
      </div>
      <FormField
        id={fieldIds.synonyms}
        label="Từ đồng nghĩa"
        required
        placeholder="tỷ lệ đạt, pass rate"
        hint="Ngăn bằng dấu phẩy, tiếng Việt và tiếng Anh. Một từ chỉ thuộc một khái niệm của gói."
        error={errorOf('synonyms')}
        value={draft.synonyms}
        onChange={(e) => set('synonyms')(e.target.value)}
      />
      <FormField
        id={fieldIds.dimensions}
        label="Chiều hay dùng để phân rã"
        placeholder="học phần, kỳ học, vùng"
        hint="Ngăn bằng dấu phẩy; không bắt buộc."
        error={errorOf('dimensions')}
        value={draft.dimensions}
        onChange={(e) => set('dimensions')(e.target.value)}
      />
      <fieldset className="flex flex-col gap-2">
        <legend className="mb-1.5 text-sm font-medium">Cách tính phổ biến (bằng lời, không bắt buộc)</legend>
        {draft.calculations.map((calculation, index) => (
          <div key={index} className="flex flex-wrap items-start gap-2">
            <Input
              id={`concept-calculation-${index}`}
              aria-label={`Tên cách tính ${index + 1}`}
              placeholder="Trên mọi lượt học"
              className="h-9 min-w-0 flex-1 basis-40"
              value={calculation.name}
              onChange={(e) => setCalculation(index, { name: e.target.value })}
            />
            <Input
              aria-label={`Mô tả cách tính ${index + 1}`}
              placeholder="Tính thế nào, khi nào chọn cách này"
              className="h-9 min-w-0 flex-[2] basis-60"
              value={calculation.description}
              onChange={(e) => setCalculation(index, { description: e.target.value })}
            />
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label={`Bỏ cách tính ${index + 1}`}
              onClick={() => removeCalculation(index)}
            >
              <X aria-hidden />
            </Button>
          </div>
        ))}
        {draft.calculations.length < CALCULATIONS_MAX && (
          <div>
            <Button type="button" variant="outline" size="sm" onClick={addCalculation}>
              <Plus aria-hidden />
              Thêm cách tính
            </Button>
          </div>
        )}
        {errorOf('calculations') && (
          <p role="alert" className="text-sm text-destructive">
            {errorOf('calculations')}
          </p>
        )}
      </fieldset>
      {error && (!error.field || error.field === 'sanity_checks') && (
        <FormMessage kind="error">{error.message}</FormMessage>
      )}
      <div className="flex flex-wrap gap-2">
        <Button type="submit" aria-disabled={pending}>
          {pending ? 'Đang kiểm và lưu...' : editing ? 'Lưu khái niệm' : 'Thêm khái niệm'}
        </Button>
        <Button type="button" variant="outline" onClick={onCancel}>
          Thôi
        </Button>
      </div>
    </form>
  )
}
