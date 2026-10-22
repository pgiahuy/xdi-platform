// SPDX-License-Identifier: Apache-2.0
/* components/datasets/UploadForm.tsx - khung nạp file: chọn nơi nạp (dataset mới hay phiên bản mới của dataset có sẵn),
chọn file, đặt tên, rồi gửi. Phần chọn sheet của file Excel do trang dựng và truyền vào qua sheetSection, vì nó phụ
thuộc lời gọi backend sau khi chọn file. Chỉ admin thấy khung này. */

import { useEffect, useId, useRef, type ReactNode, type Ref } from 'react'
import { Button } from '@/components/ui/button'

type Props = {
  /** Dataset sẽ nhận phiên bản mới; không có thì tạo dataset mới. */
  target?: { name: string }
  onClearTarget: () => void
  name: string
  onNameChange: (name: string) => void
  maxNameLength: number
  files: File[]
  onFilesChange: (files: File[]) => void
  /** Đuôi file chọn được, cho hộp chọn file của trình duyệt. */
  accept: string
  /** Việc đang chạy sau khi chọn file, ví dụ đang đọc danh sách sheet. Nằm trong vùng trình đọc màn hình tự đọc. */
  status?: ReactNode
  sheetSection?: ReactNode
  /** Lý do chưa nạp được với lựa chọn hiện tại. */
  blocker?: string
  busy: boolean
  onSubmit: () => void
  headingRef?: Ref<HTMLHeadingElement>
}

const sizeFormat = new Intl.NumberFormat('vi-VN', { maximumFractionDigits: 1 })

/** Dung lượng để đối chiếu với giới hạn nạp (tính bằng MB). File nhỏ ghi KB hay byte, không thì hiện 0 MB. */
function formatSize(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${sizeFormat.format(bytes / 1024 / 1024)} MB`
  if (bytes >= 1024) return `${sizeFormat.format(bytes / 1024)} KB`
  return `${bytes} byte`
}

export function UploadForm(props: Props) {
  const { target, files, busy, blocker, headingRef } = props
  const inputRef = useRef<HTMLInputElement>(null)
  const fileInputId = useId()
  const nameInputId = useId()
  const blockerId = useId()

  // Hộp chọn file không điều khiển được từ state; trang xóa danh sách file sau khi nạp thì xóa cả lựa chọn trong hộp,
  // không thì hộp vẫn ghi tên file cũ.
  useEffect(() => {
    if (files.length === 0 && inputRef.current) inputRef.current.value = ''
  }, [files])

  return (
    <form
      className="flex flex-col gap-4 rounded-lg border p-4"
      onSubmit={(event) => {
        event.preventDefault()
        props.onSubmit()
      }}
    >
      <h2 ref={headingRef} tabIndex={-1} className="text-sm font-semibold focus:outline-none">
        {target ? `Nạp phiên bản mới cho ${target.name}` : 'Nạp dữ liệu'}
      </h2>
      {target && (
        <p className="-mt-2 text-sm text-muted-foreground">
          Phiên bản mới thành dữ liệu hiện hành của dataset; phiên bản cũ vẫn giữ.{' '}
          <Button type="button" variant="link" className="h-auto p-0" onClick={props.onClearTarget} disabled={busy}>
            Tạo dataset mới thay vì thế
          </Button>
        </p>
      )}

      <div className="flex flex-col gap-1.5">
        <label htmlFor={fileInputId} className="text-sm font-medium">
          File
        </label>
        <input
          ref={inputRef}
          id={fileInputId}
          type="file"
          multiple
          accept={props.accept}
          disabled={busy}
          onChange={(event) => props.onFilesChange([...(event.target.files ?? [])])}
          aria-describedby={`${fileInputId}-hint`}
          className="max-w-full text-sm file:mr-3 file:rounded-md file:border file:bg-background file:px-2.5 file:py-1 file:text-sm file:font-medium"
        />
        <p id={`${fileInputId}-hint`} className="text-xs text-muted-foreground">
          Một hoặc nhiều file CSV hoặc Parquet cùng cột (gộp thành một bảng), hoặc một file Excel.
        </p>
        {files.length > 0 && (
          <ul className="text-xs text-muted-foreground">
            {files.map((file) => (
              <li key={`${file.name}-${file.size}-${file.lastModified}`} className="break-all">
                {file.name}, {formatSize(file.size)}
              </li>
            ))}
          </ul>
        )}
        {/* Vùng này có sẵn từ đầu, kể cả khi rỗng: vùng được chèn vào cùng lúc với chữ thì trình đọc màn hình thường
            không đọc chữ đầu tiên. Không ẩn bằng display: none khi rỗng, vì vùng bị ẩn cũng coi như chưa có. */}
        <div aria-live="polite" className="text-sm text-muted-foreground">
          {props.status}
        </div>
      </div>

      {props.sheetSection}

      {!target && (
        <div className="flex flex-col gap-1.5">
          <label htmlFor={nameInputId} className="text-sm font-medium">
            Tên dataset (không bắt buộc)
          </label>
          <input
            id={nameInputId}
            type="text"
            value={props.name}
            maxLength={props.maxNameLength}
            disabled={busy}
            onChange={(event) => props.onNameChange(event.target.value)}
            placeholder="Để trống thì lấy tên file"
            className="h-8 w-full max-w-sm rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
          />
        </div>
      )}

      {blocker && (
        <p id={blockerId} className="text-sm text-muted-foreground">
          {blocker}
        </p>
      )}

      <div>
        {/* Chưa đủ điều kiện thì dùng aria-disabled thay cho disabled: nút disabled không nhận tiêu điểm bằng Tab,
            người dùng bàn phím không tới được nút để nghe lý do. Trang bỏ qua lần bấm khi còn lý do chặn. */}
        <Button
          type="submit"
          disabled={busy}
          aria-disabled={blocker !== undefined}
          aria-describedby={blocker ? blockerId : undefined}
          className="aria-disabled:cursor-not-allowed aria-disabled:opacity-50"
        >
          {busy ? 'Đang nạp...' : 'Nạp'}
        </Button>
      </div>
    </form>
  )
}
