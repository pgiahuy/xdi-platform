// SPDX-License-Identifier: Apache-2.0
/* components/datasets/DatasetList.tsx - danh sách dataset của workspace, mới tạo đứng đầu (backend sắp theo ngày tạo).
Mỗi dataset một dòng xuống hàng được, không dùng bảng: màn 320px không đủ chỗ cho ba cột.
onAddVersion và onDelete chỉ truyền vào khi người xem là admin; member không thấy các nút đó. */

import { ConfirmDialog } from '@/components/ConfirmDialog'
import { Button } from '@/components/ui/button'
import type { DatasetRow } from './types'

type Props = {
  datasets: DatasetRow[]
  onAddVersion?: (dataset: DatasetRow) => void
  onDelete?: (dataset: DatasetRow) => void
  deletePending?: boolean
}

const dateFormat = new Intl.DateTimeFormat('vi-VN', { dateStyle: 'short', timeStyle: 'short' })

export function DatasetList({ datasets, onAddVersion, onDelete, deletePending }: Props) {
  return (
    <ul className="divide-y rounded-xl border bg-card shadow-xs">
      {datasets.map((dataset) => (
        <li key={dataset.id} className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3">
          <div className="min-w-0 flex-1 basis-56">
            <p className="truncate text-sm font-medium" title={dataset.name}>
              {dataset.name}
            </p>
            <p className="text-xs text-muted-foreground">
              {/* Chỉ tên bảng được ngắt giữa chữ: tên bảng dài không có khoảng trắng; ngày giờ thì không tách. */}
              Bảng <code className="break-all">{dataset.tableName}</code>, tạo lúc{' '}
              <span className="whitespace-nowrap">{dateFormat.format(new Date(dataset.createdAt))}</span>
            </p>
          </div>
          {onAddVersion && (
            // Mỗi dòng một nút cùng chữ; aria-label thêm tên dataset để trình đọc màn hình phân biệt. Chữ hiện ra
            // đứng đầu aria-label, để người dùng điều khiển bằng giọng nói vẫn gọi được nút.
            <Button
              variant="outline"
              size="sm"
              aria-label={`Nạp phiên bản mới cho ${dataset.name}`}
              onClick={() => onAddVersion(dataset)}
            >
              Nạp phiên bản mới
            </Button>
          )}
          {onDelete && (
            <ConfirmDialog
              trigger={
                <Button
                  variant="outline"
                  size="sm"
                  aria-label={`Xóa ${dataset.name}`}
                  aria-disabled={deletePending}
                >
                  Xóa
                </Button>
              }
              title={`Xóa ${dataset.name}?`}
              description="Dataset, mọi phiên bản và file của nó bị xóa. Không hoàn tác được."
              confirmLabel="Xóa dataset"
              onConfirm={() => onDelete(dataset)}
            />
          )}
        </li>
      ))}
    </ul>
  )
}
