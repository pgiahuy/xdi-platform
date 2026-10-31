// SPDX-License-Identifier: Apache-2.0
/* components/domains/ConceptTable.tsx - bảng khái niệm của một gói ngành: tên, từ đồng nghĩa, mô tả, trạng thái. onEdit
và onToggle chỉ truyền vào khi người xem là admin; member chỉ xem. Khái niệm tắt vẫn hiện (mờ đi) để bật lại được. Màn
hẹp thì bảng cuộn ngang trong khung của nó, trang không cuộn ngang. */

import { Pencil } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { ConceptItem } from './types'

type Props = {
  concepts: ConceptItem[]
  onEdit?: (concept: ConceptItem) => void
  onToggle?: (concept: ConceptItem) => void
  /** Id của khái niệm đang được bật hay tắt, để khóa nút của nó. */
  togglingId?: string
}

function statusOf(concept: ConceptItem): string | undefined {
  const parts = [
    !concept.enabled && 'đã tắt, không gửi cho mô hình',
    concept.key === null && 'admin thêm',
    concept.key !== null && concept.updatedAt && 'đã sửa so với gói gốc',
  ].filter(Boolean)
  return parts.length > 0 ? parts.join(', ') : undefined
}

export function ConceptTable({ concepts, onEdit, onToggle, togglingId }: Props) {
  const canEdit = Boolean(onEdit || onToggle)
  return (
    // relative: nhãn sr-only của cột thao tác là position absolute; thiếu nó, nhãn nằm ngoài khung cuộn và làm cả trang
    // cuộn ngang trên màn hẹp.
    <div className="relative overflow-x-auto rounded-xl border bg-card shadow-xs">
      <table className="w-full min-w-[40rem] text-left text-sm">
        <thead className="border-b text-xs text-muted-foreground">
          <tr>
            <th scope="col" className="px-4 py-2 font-medium">Tên</th>
            <th scope="col" className="px-4 py-2 font-medium">Từ đồng nghĩa</th>
            <th scope="col" className="px-4 py-2 font-medium">Mô tả</th>
            {canEdit && (
              <th scope="col" className="px-4 py-2 font-medium">
                <span className="sr-only">Thao tác</span>
              </th>
            )}
          </tr>
        </thead>
        <tbody className="divide-y">
          {concepts.map((concept) => {
            const status = statusOf(concept)
            return (
              <tr key={concept.id} className={cn('align-top', !concept.enabled && 'text-muted-foreground')}>
                <td className="px-4 py-3 font-medium">
                  {concept.name}
                  {status && <span className="block text-xs font-normal text-muted-foreground">{status}</span>}
                </td>
                <td className="px-4 py-3">{concept.synonyms.join(', ')}</td>
                <td className="px-4 py-3 text-muted-foreground">{concept.description}</td>
                {canEdit && (
                  <td className="px-4 py-2">
                    <div className="flex justify-end gap-1">
                      {onEdit && (
                        <Button variant="ghost" size="icon-sm" aria-label={`Sửa ${concept.name}`} onClick={() => onEdit(concept)}>
                          <Pencil aria-hidden />
                        </Button>
                      )}
                      {onToggle && (
                        <Button
                          variant="ghost"
                          size="sm"
                          aria-label={`${concept.enabled ? 'Tắt' : 'Bật'} ${concept.name}`}
                          disabled={togglingId === concept.id}
                          onClick={() => onToggle(concept)}
                        >
                          {concept.enabled ? 'Tắt' : 'Bật'}
                        </Button>
                      )}
                    </div>
                  </td>
                )}
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}
