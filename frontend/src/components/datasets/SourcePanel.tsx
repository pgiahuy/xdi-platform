// SPDX-License-Identifier: Apache-2.0
/* components/datasets/SourcePanel.tsx - một nguồn database đã kết nối: thử kết nối lại, chọn bảng, chọn bảng chụp kèm
(phần relations, thường là RelationPicker), đặt tên, chụp bảng thành dataset, xóa nguồn. Danh sách bảng và kết quả do trang
truyền vào (pages/DatabaseSources.tsx). */

import { useId, type ReactNode } from 'react'
import { ConfirmDialog } from '@/components/ConfirmDialog'
import { FormField } from '@/components/forms/FormField'
import { FormMessage } from '@/components/forms/FormMessage'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'

export type TableOption = {
  /** schema.table, đúng chuỗi gửi lại khi chụp. */
  fullName: string
  kind: string
}

type Props = {
  /** máy:cổng/database (tài khoản), không có mật khẩu. */
  label: string
  tables?: TableOption[]
  tablesLoading: boolean
  tablesError?: string
  table: string
  onTableChange: (table: string) => void
  /** Phần chọn bảng nối của bảng đang chọn; trang dựng sẵn. */
  relations?: ReactNode
  name: string
  onNameChange: (name: string) => void
  busy: boolean
  message?: { kind: 'error' | 'success'; text: string }
  onTest: () => void
  onSnapshot: () => void
  onDelete: () => void
}

export function SourcePanel(props: Props) {
  const { label, tables, tablesLoading, tablesError, table, name, busy, message } = props
  const id = useId()
  return (
    <div className="flex flex-col gap-3 rounded-lg border p-3">
      <div className="flex flex-wrap items-center gap-2">
        <p className="min-w-0 flex-1 basis-48 text-sm font-medium break-all">{label}</p>
        <Button type="button" variant="outline" size="sm" onClick={props.onTest} disabled={busy}>
          Thử kết nối
        </Button>
        <ConfirmDialog
          trigger={
            <Button type="button" variant="outline" size="sm" aria-label={`Xóa nguồn ${label}`} disabled={busy}>
              Xóa nguồn
            </Button>
          }
          title="Xóa nguồn này?"
          description="Thông tin kết nối bị xóa. Nguồn còn dataset chụp từ nó thì phải xóa các dataset đó trước."
          confirmLabel="Xóa nguồn"
          onConfirm={props.onDelete}
        />
      </div>
      {tablesLoading && <p className="text-sm text-muted-foreground">Đang đọc danh sách bảng...</p>}
      {tablesError && <FormMessage kind="error">{tablesError}</FormMessage>}
      {tables && (
        <form
          className="flex flex-col gap-3"
          onSubmit={(event) => {
            event.preventDefault()
            props.onSnapshot()
          }}
        >
          <div className="flex flex-col gap-1.5">
            <Label htmlFor={`${id}-table`}>Bảng hoặc view</Label>
            <select
              id={`${id}-table`}
              className="h-9 rounded-md border bg-background px-2 text-sm"
              value={table}
              onChange={(event) => props.onTableChange(event.target.value)}
            >
              <option value="">Chọn bảng</option>
              {tables.map((t) => (
                <option key={t.fullName} value={t.fullName}>
                  {t.kind === 'view' ? `${t.fullName} (view)` : t.fullName}
                </option>
              ))}
            </select>
          </div>
          {props.relations}
          <FormField
            id={`${id}-name`}
            label="Tên dataset"
            value={name}
            onChange={(event) => props.onNameChange(event.target.value)}
            hint="Bỏ trống thì lấy tên bảng."
          />
          <div>
            <Button type="submit" disabled={busy || table === ''}>
              {busy ? 'Đang chụp...' : 'Chụp bảng thành dataset'}
            </Button>
          </div>
        </form>
      )}
      {message && <FormMessage kind={message.kind}>{message.text}</FormMessage>}
    </div>
  )
}
