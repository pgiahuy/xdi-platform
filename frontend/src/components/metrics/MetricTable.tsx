// SPDX-License-Identifier: Apache-2.0
/* components/metrics/MetricTable.tsx - bảng chỉ số của một dataset: tên, mô tả, công thức, đơn vị. onEdit và onDelete
chỉ truyền vào khi người xem là admin; member chỉ xem. Màn hẹp thì bảng cuộn ngang trong khung của nó, trang không
cuộn ngang. Xóa hỏi lại trước (ConfirmDialog) vì xóa mọi phiên bản của chỉ số. */

import { Pencil, Trash2 } from 'lucide-react'
import { ConfirmDialog } from '@/components/ConfirmDialog'
import { Button } from '@/components/ui/button'
import type { MetricItem } from './types'

type Props = {
  metrics: MetricItem[]
  onEdit?: (metric: MetricItem) => void
  onDelete?: (metric: MetricItem) => void
  deletePending?: boolean
}

export function MetricTable({ metrics, onEdit, onDelete, deletePending }: Props) {
  const canEdit = Boolean(onEdit || onDelete)
  return (
    // relative: nhãn sr-only của cột thao tác là position absolute; thiếu nó, nhãn nằm ngoài khung cuộn và làm cả trang
    // cuộn ngang trên màn hẹp.
    <div className="relative overflow-x-auto rounded-xl border bg-card shadow-xs">
      <table className="w-full min-w-[36rem] text-left text-sm">
        <thead className="border-b text-xs text-muted-foreground">
          <tr>
            <th scope="col" className="px-4 py-2 font-medium">Tên</th>
            <th scope="col" className="px-4 py-2 font-medium">Mô tả</th>
            <th scope="col" className="px-4 py-2 font-medium">Công thức</th>
            <th scope="col" className="px-4 py-2 font-medium whitespace-nowrap">Đơn vị</th>
            {canEdit && (
              <th scope="col" className="px-4 py-2 font-medium">
                <span className="sr-only">Thao tác</span>
              </th>
            )}
          </tr>
        </thead>
        <tbody className="divide-y">
          {metrics.map((metric) => (
            <tr key={metric.id} className="align-top">
              <td className="px-4 py-3 font-medium">
                {metric.name}
                {metric.version > 1 && (
                  <span className="block text-xs font-normal text-muted-foreground">phiên bản {metric.version}</span>
                )}
              </td>
              <td className="px-4 py-3 text-muted-foreground">{metric.description || 'Chưa có mô tả'}</td>
              <td className="px-4 py-3">
                <code className="break-words">{metric.expression}</code>
                {metric.filter && (
                  <span className="mt-1 block text-xs text-muted-foreground">
                    chỉ tính dòng có <code className="break-words">{metric.filter}</code>
                  </span>
                )}
              </td>
              <td className="px-4 py-3">{metric.unit ?? ''}</td>
              {canEdit && (
                <td className="px-4 py-2">
                  <div className="flex justify-end gap-1">
                    {onEdit && (
                      <Button variant="ghost" size="icon-sm" aria-label={`Sửa ${metric.name}`} onClick={() => onEdit(metric)}>
                        <Pencil aria-hidden />
                      </Button>
                    )}
                    {onDelete && (
                      <ConfirmDialog
                        trigger={
                          <Button variant="ghost" size="icon-sm" aria-label={`Xóa ${metric.name}`} aria-disabled={deletePending}>
                            <Trash2 aria-hidden />
                          </Button>
                        }
                        title={`Xóa chỉ số ${metric.name}?`}
                        description="Chỉ số cùng mọi phiên bản của nó bị xóa; câu hỏi sau không dùng công thức này nữa. Câu hỏi cũ vẫn giữ câu SQL đã chạy."
                        confirmLabel="Xóa chỉ số"
                        onConfirm={() => onDelete(metric)}
                      />
                    )}
                  </div>
                </td>
              )}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
