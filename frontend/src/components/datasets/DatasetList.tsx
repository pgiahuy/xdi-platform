// SPDX-License-Identifier: Apache-2.0
/* components/datasets/DatasetList.tsx - danh sách dataset của workspace, mới tạo đứng đầu (backend sắp theo ngày tạo).
Mỗi dataset một dòng xuống hàng được, không dùng bảng: màn 320px không đủ chỗ cho ba cột.
onAddVersion, onSnapshotAgain và onDelete chỉ truyền vào khi người xem là admin; member không thấy các nút đó. Dataset
chụp từ database ghi bảng nguồn, lần chụp gần nhất, các dataset liên quan theo khóa ngoại, và có nút "Chụp lại" thay
cho nạp phiên bản mới; xóa dataset có liên quan thì hộp xác nhận báo quan hệ bị xóa theo. Link Tổng quan (trang Tổng
quan dữ liệu mở sẵn dataset đó, #198) và link Chỉ số (màn chỉ số của dataset) hiện với mọi người khi trang truyền
overviewHref, metricsHref. */

import { ConfirmDialog } from '@/components/ConfirmDialog'
import type { LinkComponent } from '@/components/shell/types'
import { Button, buttonVariants } from '@/components/ui/button'
import type { DatasetRow } from './types'

type Props = {
  datasets: DatasetRow[]
  /** Đường dẫn tới trang Tổng quan dữ liệu của dataset; mở bằng component link như metricsHref. */
  overviewHref?: (dataset: DatasetRow) => string
  onAddVersion?: (dataset: DatasetRow) => void
  onSnapshotAgain?: (dataset: DatasetRow) => void
  snapshotPending?: boolean
  onDelete?: (dataset: DatasetRow) => void
  deletePending?: boolean
  /** Đường dẫn tới màn Chỉ số của dataset, và component link của router để mở nó. */
  metricsHref?: (dataset: DatasetRow) => string
  link?: LinkComponent
}

/** Dataset có liên quan thì nói rõ quan hệ mất theo, còn các dataset kia vẫn giữ. */
function deleteText(dataset: DatasetRow): string {
  const base = 'Dataset, mọi phiên bản, file và lịch sử câu hỏi của nó bị xóa, kể cả các cuộc trò chuyện.'
  const count = dataset.related.length
  const links =
    count > 0
      ? ` ${count} dataset khác đang nối với dataset này (${dataset.related.map((r) => r.name).join(', ')}): quan hệ` +
        ' với chúng bị xóa theo, các dataset đó vẫn giữ nguyên.'
      : ''
  return `${base}${links} Không hoàn tác được.`
}

const dateFormat = new Intl.DateTimeFormat('vi-VN', { dateStyle: 'short', timeStyle: 'short' })

export function DatasetList(props: Props) {
  const { datasets, onAddVersion, onSnapshotAgain, snapshotPending, onDelete, deletePending } = props
  const { overviewHref, metricsHref, link: Link } = props
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
            {dataset.related.length > 0 && (
              <p className="text-xs break-words text-muted-foreground">
                Liên quan: {dataset.related.map((r) => r.name).join(', ')}
              </p>
            )}
          </div>
          {overviewHref && Link && (
            <Link
              href={overviewHref(dataset)}
              aria-label={`Tổng quan dữ liệu của ${dataset.name}`}
              className={buttonVariants({ variant: 'outline', size: 'sm' })}
            >
              Tổng quan
            </Link>
          )}
          {metricsHref && Link && (
            <Link
              href={metricsHref(dataset)}
              aria-label={`Chỉ số của ${dataset.name}`}
              className={buttonVariants({ variant: 'outline', size: 'sm' })}
            >
              Chỉ số
            </Link>
          )}
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
              description={deleteText(dataset)}
              confirmLabel="Xóa dataset"
              onConfirm={() => onDelete(dataset)}
            />
          )}
        </li>
      ))}
    </ul>
  )
}
