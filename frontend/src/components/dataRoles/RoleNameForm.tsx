// SPDX-License-Identifier: Apache-2.0
/* components/dataRoles/RoleNameForm.tsx - tên và mô tả của một vai dữ liệu: tạo vai mới (trống) hay đổi tên vai đang có
(điền sẵn). Tên trùng trong workspace thì backend báo lời, form hiện dưới ô tên. Chỉ nhận dữ liệu qua props. */

import { useId, useState, type FormEvent } from 'react'
import { FormField } from '@/components/forms/FormField'
import { FormMessage } from '@/components/forms/FormMessage'
import { Button } from '@/components/ui/button'

type Props = {
  initialName?: string
  initialDescription?: string
  submitLabel: string
  pending: boolean
  error?: string
  onSubmit: (name: string, description: string) => void
  onCancel?: () => void
}

// Cùng trần với backend (service/data_roles/mod.rs).
const nameMaxChars = 60
const descriptionMaxChars = 500

export function RoleNameForm(props: Props) {
  const [name, setName] = useState(props.initialName ?? '')
  const [description, setDescription] = useState(props.initialDescription ?? '')
  const nameId = useId()
  const descriptionId = useId()

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (props.pending) return
    props.onSubmit(name.trim(), description.trim())
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-3">
      <div className="flex flex-wrap items-start gap-3">
        <div className="min-w-0 flex-1 basis-48">
          <FormField
            id={nameId}
            label="Tên vai"
            required
            maxLength={nameMaxChars}
            placeholder="Ví dụ: Miền Nam"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </div>
        <div className="min-w-0 flex-[2] basis-64">
          <FormField
            id={descriptionId}
            label="Mô tả (không bắt buộc)"
            maxLength={descriptionMaxChars}
            placeholder="Ví dụ: Nhân viên kinh doanh phụ trách thị trường châu Âu"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
          />
        </div>
      </div>
      {props.error && <FormMessage kind="error">{props.error}</FormMessage>}
      <div className="flex flex-wrap gap-2">
        <Button type="submit" aria-disabled={props.pending}>
          {props.pending ? 'Đang lưu...' : props.submitLabel}
        </Button>
        {props.onCancel && (
          <Button type="button" variant="outline" onClick={props.onCancel}>
            Thôi
          </Button>
        )}
      </div>
    </form>
  )
}
