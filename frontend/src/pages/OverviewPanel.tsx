// SPDX-License-Identifier: Apache-2.0
/* pages/OverviewPanel.tsx - khung "Tổng quan dữ liệu" của một dataset: tải tổng quan (api/overview.ts), báo đang tải hay
lỗi, rồi vẽ DataOverview. Trang Tổng quan dữ liệu (DatasetOverviewSection.tsx) dùng, truyền onPick mở trang cuộc trò
chuyện với câu hỏi điền sẵn. Chỉ tải khi khung được mở: trang không gọi route tổng quan cho dataset không ai
xem. */

import { useMemo } from 'react'
import { useDatasetOverview } from '@/api/overview'
import { DataOverview } from '@/components/overview/DataOverview'
import { FormMessage } from '@/components/forms/FormMessage'
import { Button } from '@/components/ui/button'
import { buildOverviewView } from './overviewView'

type Props = {
  ws: string
  datasetId: string
  onPick: (question: string) => void
}

export function OverviewPanel({ ws, datasetId, onPick }: Props) {
  const overview = useDatasetOverview(ws, datasetId, true)
  const view = useMemo(() => overview.data && buildOverviewView(overview.data), [overview.data])
  if (overview.error && !view) {
    return (
      <div className="flex flex-col items-start gap-2">
        <FormMessage kind="error">{`Không tải được tổng quan dữ liệu: ${overview.error.message}`}</FormMessage>
        <Button variant="outline" size="sm" onClick={() => overview.refetch()}>
          Thử lại
        </Button>
      </div>
    )
  }
  if (!view) return <p className="text-sm text-muted-foreground">Đang tải tổng quan dữ liệu...</p>
  return <DataOverview overview={view} onPick={onPick} />
}
