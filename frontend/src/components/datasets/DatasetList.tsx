// SPDX-License-Identifier: Apache-2.0
/* components/datasets/DatasetList.tsx - danh sách dataset của workspace, mới tạo đứng đầu (backend sắp theo ngày tạo).
Mỗi dataset một dòng xuống hàng được, không dùng bảng: màn 320px không đủ chỗ cho ba cột.
onAddVersion, onSnapshotAgain và onDelete chỉ truyền vào khi người xem là admin; member không thấy các nút đó. Dataset
chụp từ database ghi bảng nguồn, các bảng đã nối nhiều-một, lần chụp gần nhất, và có nút "Chụp lại" thay cho nạp phiên
bản mới. */

import { ConfirmDialog } from '@/components/ConfirmDialog'
import { Button } from '@/components/ui/button'
import type { DatasetRow, ModelJoinRow } from './types'

type Props = {
  datasets: DatasetRow[]
  onAddVersion?: (dataset: DatasetRow) => void
  onSnapshotAgain?: (dataset: DatasetRow) => void
  snapshotPending?: boolean
  onDelete?: (dataset: DatasetRow) => void
  deletePending?: boolean
}

/** "sales.orders (order_id)"; bảng nối từ một bảng nối khác ghi thêm bảng đó: "sales.customers (customer_id của
 * sales.orders)". */
function joinText(join: ModelJoinRow, fact: string): string {
  const key = join.fromColumns.join(', ')
  return join.fromTable === fact ? `${join.toTable} (${key})` : `${join.toTable} (${key} của ${join.fromTable})`
}

const dateFormat = new Intl.DateTimeFormat('vi-VN', { dateStyle: 'short', timeStyle: 'short' })

export function DatasetList(props: Props) {
  const { datasets, onAddVersion, onSnapshotAgain, snapshotPending, onDelete, deletePending } = props
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
            {dataset.snapshot && (
              <p className="text-xs text-muted-foreground">
                Chụp từ Postgres <code className="break-all">{dataset.snapshot.table}</code>
                {dataset.snapshot.at && (
                  <>
                    , lần chụp gần nhất{' '}
                    <span className="whitespace-nowrap">{dateFormat.format(new Date(dataset.snapshot.at))}</span>
                  </>
                )}
              </p>
            )}
            {dataset.snapshot && dataset.snapshot.joins.length > 0 && (
              <p className="text-xs break-words text-muted-foreground">
                Dựng từ {dataset.snapshot.table}, nối nhiều-một sang{' '}
                {dataset.snapshot.joins.map((join) => joinText(join, dataset.snapshot?.table ?? '')).join(', ')}
              </p>
            )}
          </div>
          {dataset.snapshot && onSnapshotAgain && (
            <Button
              variant="outline"
              size="sm"
              aria-label={`Chụp lại ${dataset.name}`}
              aria-disabled={snapshotPending}
              onClick={() => !snapshotPending && onSnapshotAgain(dataset)}
            >
              Chụp lại
            </Button>
          )}
          {!dataset.snapshot && onAddVersion && (
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
