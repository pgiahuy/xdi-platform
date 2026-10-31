// SPDX-License-Identifier: Apache-2.0
/* pages/OverviewPage.tsx - trang "Tổng quan dữ liệu" của workspace (/w/:ws/overview), mục riêng trên thanh bên: chọn
một dataset ở ô Dataset đầu trang rồi xem tổng quan của nó (DatasetOverviewSection.tsx, cùng khung OverviewPanel với
trang hỏi). Dataset đang xem nằm trong đường dẫn (?dataset=...), nên nút Tổng quan ở trang Dữ liệu mở thẳng đúng
dataset và tải lại trang vẫn giữ. Đường dẫn không ghi dataset, hay dataset đó đã bị xóa, thì xem dataset tạo gần nhất,
như trang hỏi chọn sẵn. */

import { Link, useParams, useSearchParams } from 'react-router'
import { useDatasets } from '@/api/datasets'
import { DatasetPicker } from '@/components/ask/DatasetPicker'
import { EmptyState } from '@/components/EmptyState'
import { Button } from '@/components/ui/button'
import { DatasetOverviewSection } from './DatasetOverviewSection'
import { workspaceHref } from './workspaceNav'

export function OverviewPage() {
  const { ws = '' } = useParams()
  const datasets = useDatasets(ws)
  const [params, setParams] = useSearchParams()
  // Backend trả dataset mới tạo trước, nên phần tử đầu là dataset tạo gần nhất.
  const dataset = datasets.data?.find((d) => d.id === params.get('dataset')) ?? datasets.data?.[0]

  if (datasets.error && !datasets.data) {
    return (
      <EmptyState
        title="Không tải được danh sách dataset"
        description={datasets.error.message}
        action={
          <Button variant="outline" onClick={() => datasets.refetch()}>
            Thử lại
          </Button>
        }
      />
    )
  }
  if (!datasets.data) return <p className="text-sm text-muted-foreground">Đang tải...</p>
  if (!dataset) {
    return (
      <EmptyState
        title="Chưa có dataset nào"
        description="Dataset nạp ở trang Dữ liệu sẽ có tổng quan ở đây."
        action={
          <Button variant="outline" asChild>
            <Link to={workspaceHref(ws, 'datasets')}>Tới trang Dữ liệu</Link>
          </Button>
        }
      />
    )
  }
  return (
    <div className="flex max-w-6xl flex-col gap-4">
      <DatasetPicker
        datasets={datasets.data.map((d) => ({ id: d.id, name: d.name }))}
        datasetId={dataset.id}
        onDatasetChange={(id) => setParams({ dataset: id })}
        // Khung tổng quan tự ghi phần hồ sơ chưa có (lời của backend), nên dưới ô chọn không báo lại lần nữa.
        profile="ready"
        disabled={false}
      />
      <DatasetOverviewSection ws={ws} dataset={{ id: dataset.id, name: dataset.name }} />
    </div>
  )
}
