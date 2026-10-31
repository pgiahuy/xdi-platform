// SPDX-License-Identifier: Apache-2.0
/* components/datasets/SourcePanel.tsx - một nguồn database đã kết nối: thử kết nối lại, xóa nguồn, danh sách bảng để
chụp (phần picker, là SourceTablePicker, #247), tiến trình lúc đang chụp, rồi kết quả chụp: bảng mới, phiên bản mới hay
không đổi, bảng chỉ lưu cấu trúc kèm lý do, bảng không đọc được. Nguồn đã chụp rồi thì danh sách gập lại sau một nút,
vì chụp lại cả dataset nằm ở thẻ dataset. Dữ liệu và kết quả do trang truyền vào (pages/DatabaseSources.tsx). */

import type { ReactNode } from 'react'
import { ChevronDownIcon, ChevronRightIcon, LoaderCircle } from 'lucide-react'
import { ConfirmDialog } from '@/components/ConfirmDialog'
import { FormMessage } from '@/components/forms/FormMessage'
import { Button } from '@/components/ui/button'
import type { SnapshotSummary, SnapshotTableResult } from './types'

type Props = {
  /** máy:cổng/database (tài khoản), không có mật khẩu. */
  label: string
  tablesLoading: boolean
  tablesError?: string
  /** Danh sách bảng; trang dựng sẵn. */
  picker?: ReactNode
  /** undefined: danh sách luôn mở (nguồn chưa chụp bảng nào), không có nút gập. */
  pickerOpen?: boolean
  onPickerOpenChange: (open: boolean) => void
  busy: boolean
  /** Số bảng đang chụp; có thì hiện thanh tiến trình. */
  snapshotting?: number
  message?: { kind: 'error' | 'success'; text: string }
  result?: SnapshotSummary
  onTest: () => void
  onDelete: () => void
}

export function SourcePanel(props: Props) {
  const { label, tablesLoading, tablesError, picker, pickerOpen, busy, message, snapshotting } = props
  const showPicker = pickerOpen ?? true
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
          description="Thông tin kết nối bị xóa. Database còn bảng đã chụp thì phải xóa các bảng đó trước."
          confirmLabel="Xóa nguồn"
          onConfirm={props.onDelete}
        />
      </div>
      {tablesLoading && <p className="text-sm text-muted-foreground">Đang đọc danh sách bảng...</p>}
      {tablesError && <FormMessage kind="error">{tablesError}</FormMessage>}
      {picker && pickerOpen !== undefined && (
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="self-start"
          aria-expanded={pickerOpen}
          onClick={() => props.onPickerOpenChange(!pickerOpen)}
        >
          {pickerOpen ? <ChevronDownIcon aria-hidden="true" /> : <ChevronRightIcon aria-hidden="true" />}
          {pickerOpen ? 'Ẩn danh sách bảng' : 'Chụp thêm bảng'}
        </Button>
      )}
      {picker && showPicker && picker}
      {snapshotting !== undefined && <SnapshotProgress count={snapshotting} />}
      {props.result && <SnapshotResult {...props.result} />}
      {message && <FormMessage kind={message.kind}>{message.text}</FormMessage>}
    </div>
  )
}

/** Backend chụp mọi bảng trong một lần gọi và không báo tiến độ từng bảng, nên chỉ báo đang chạy, không có phần trăm. */
function SnapshotProgress({ count }: { count: number }) {
  return (
    <div role="status" className="flex items-start gap-2 rounded-md bg-muted px-3 py-2 text-sm">
      <LoaderCircle aria-hidden className="mt-0.5 size-4 shrink-0 animate-spin text-primary" />
      <p>Đang chụp {count} bảng trong một giao dịch chỉ đọc. Bảng lớn chụp lâu hơn; giữ trang này mở.</p>
    </div>
  )
}

const numberFormat = new Intl.NumberFormat('vi-VN')

function SnapshotResult({ tables, relationCount, metadataOnly, unreadable }: SnapshotSummary) {
  const relations = relationCount > 0 ? `, ghi ${relationCount} quan hệ giữa các bảng` : ''
  return (
    <div role="status" className="flex flex-col gap-2 rounded-md bg-muted px-3 py-2 text-sm">
      {tables.length > 0 && (
        <div>
          <p>
            Đã chụp {tables.length} bảng{relations}:
          </p>
          <ul className="mt-1 list-disc pl-5">
            {tables.map((t) => (
              <li key={t.datasetName} className="break-words">
                {t.datasetName}: {tableResultText(t)}
              </li>
            ))}
          </ul>
        </div>
      )}
      {metadataOnly.length > 0 && (
        <div>
          <p>{metadataOnly.length} bảng chỉ lưu cấu trúc (cột, kiểu, khóa, mô tả), chưa có dòng nào:</p>
          <ul className="mt-1 list-disc pl-5">
            {metadataOnly.map((t) => (
              <li key={t.table} className="break-words">
                <code>{t.table}</code>: {t.reason}
              </li>
            ))}
          </ul>
        </div>
      )}
      {unreadable.length > 0 && (
        <p className="break-words">
          Tài khoản không còn đọc được {unreadable.length} bảng, các bảng này giữ phiên bản cũ: {unreadable.join(', ')}.
        </p>
      )}
    </div>
  )
}

function tableResultText(t: SnapshotTableResult): string {
  if (!t.created) return `không đổi, giữ phiên bản ${t.version}`
  const rows = t.rowCount === null ? '' : `, ${numberFormat.format(t.rowCount)} dòng`
  return t.version === 1 ? `bảng mới${rows}` : `phiên bản ${t.version}${rows}`
}
