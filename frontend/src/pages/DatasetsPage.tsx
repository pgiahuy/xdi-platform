// SPDX-License-Identifier: Apache-2.0
/* pages/DatasetsPage.tsx - trang Dữ liệu của workspace: khung kết nối database (DatabaseSources.tsx, luồng chính, #230),
danh sách dataset, và nạp file là lựa chọn phụ cho admin: một nút mở khung nạp (DatasetUpload.tsx). Bấm "Nạp phiên bản
mới" ở một dataset nạp từ file thì khung nạp tự mở.
Vai lấy từ /auth/me (đã có trong bộ đệm, AppLayout vừa đọc). Ẩn khung nạp với member chỉ để giao diện gọn;
quyền thật nằm ở backend, member gọi thẳng API vẫn nhận 403.
Hồ sơ cột, chỉ số và biểu đồ dựng sẵn của từng dataset ở trang Tổng quan dữ liệu (OverviewPage.tsx, #198); nút Tổng
quan của dòng dataset mở trang đó với đúng dataset. */

import { useEffect, useRef, useState } from 'react'
import { useParams } from 'react-router'
import { useMe } from '@/api/auth'
import { useDatasets, useDeleteDataset, type Dataset } from '@/api/datasets'
import { useSnapshot } from '@/api/sources'
import { DatasetList } from '@/components/datasets/DatasetList'
import type { DatasetRow } from '@/components/datasets/types'
import { EmptyState } from '@/components/EmptyState'
import { FormMessage } from '@/components/forms/FormMessage'
import { Button } from '@/components/ui/button'
import { DatabaseSources } from './DatabaseSources'
import { DatasetUpload } from './DatasetUpload'
import { RouterLink } from './RouterLink'
import { overviewHref } from './workspaceNav'

export function DatasetsPage() {
  const { ws = '' } = useParams()
  // Đổi workspace vẫn giữ trang này (cùng route), nên state cũ (file đã chọn, kết quả nạp, dataset đang nhận phiên
  // bản mới) sẽ đi theo sang workspace kia. key theo workspace để dựng lại từ đầu.
  return <WorkspaceDatasets key={ws} ws={ws} />
}

function WorkspaceDatasets({ ws }: { ws: string }) {
  const me = useMe()
  const datasets = useDatasets(ws)
  const deleteDataset = useDeleteDataset(ws)
  const snapshot = useSnapshot(ws)
  const [listError, setListError] = useState<string>()
  const [target, setTarget] = useState<Dataset>()
  const [uploadOpen, setUploadOpen] = useState(false)
  const uploadHeading = useRef<HTMLHeadingElement>(null)

  const isAdmin = me.data?.workspaces.find((m) => m.slug === ws)?.role === 'admin'
  const showUpload = uploadOpen || target !== undefined

  // Khung nạp có thể vừa mới mở (chưa có trong trang lúc bấm) hoặc đã cuộn khuất; sau khi vẽ thì đưa tiêu điểm lên đó
  // để người dùng thấy ngay mình đang nạp cho dataset nào, và người dùng bàn phím không phải lần ngược lên.
  useEffect(() => {
    if (!target) return
    uploadHeading.current?.focus()
    uploadHeading.current?.scrollIntoView({ block: 'start' })
  }, [target])

  const addVersion = (row: DatasetRow) => setTarget(datasets.data?.find((d) => d.id === row.id))

  // Chụp lại dùng đúng bảng của lần chụp trước (backend nhớ theo dataset) và chụp lại cả các dataset liên quan, nên
  // chỉ cần nguồn và dataset.
  const snapshotAgain = (row: DatasetRow) => {
    const sourceId = datasets.data?.find((d) => d.id === row.id)?.source_id
    if (!sourceId) return
    snapshot.mutate(
      { sourceId, dataset_id: row.id },
      {
        onError: (error) => setListError(error.message),
        onSuccess: () => setListError(undefined),
      },
    )
  }

  const remove = (row: DatasetRow) => {
    deleteDataset.mutate(row.id, {
      onError: (error) => setListError(error.message),
      onSuccess: () => setListError(undefined),
    })
  }

  return (
    <div className="flex max-w-3xl flex-col gap-8">
      {isAdmin && <DatabaseSources ws={ws} />}
      {isAdmin && showUpload && (
        <div className="flex flex-col gap-2">
          <DatasetUpload
            ws={ws}
            target={target}
            onClearTarget={(id) => setTarget((current) => (id === undefined || current?.id === id ? undefined : current))}
            headingRef={uploadHeading}
          />
          {!target && (
            <Button variant="ghost" size="sm" className="self-start" onClick={() => setUploadOpen(false)}>
              Đóng khung nạp file
            </Button>
          )}
        </div>
      )}
      {isAdmin && !showUpload && (
        <div className="flex flex-wrap items-center gap-3 text-sm text-muted-foreground">
          <span>Dữ liệu chưa nằm trong database?</span>
          <Button variant="outline" size="sm" onClick={() => setUploadOpen(true)}>
            Nạp file CSV, Parquet hoặc Excel
          </Button>
        </div>
      )}

      <section aria-labelledby="dataset-list-heading" className="flex flex-col gap-3">
        <h2 id="dataset-list-heading" className="text-[15px] font-semibold tracking-tight">
          Dataset của workspace
        </h2>
        <ListBody
          ws={ws}
          datasets={datasets.data}
          error={datasets.error}
          onRetry={() => datasets.refetch()}
          isAdmin={isAdmin}
          onAddVersion={addVersion}
          onSnapshotAgain={snapshotAgain}
          snapshotPending={snapshot.isPending}
          onDelete={remove}
          deletePending={deleteDataset.isPending}
        />
        {listError && <FormMessage kind="error">{listError}</FormMessage>}
      </section>
    </div>
  )
}

/** Hai dataset có thể nối nhau theo nhiều khóa ngoại (hai chiều, hai cột); danh sách chỉ ghi mỗi dataset một lần. */
function relatedDatasets(dataset: Dataset): { id: string; name: string }[] {
  const byId = new Map(dataset.relations.map((r) => [r.dataset_id, { id: r.dataset_id, name: r.name }]))
  return [...byId.values()]
}

type ListBodyProps = {
  ws: string
  datasets: Dataset[] | undefined
  error: Error | null
  onRetry: () => void
  isAdmin: boolean
  onAddVersion: (row: DatasetRow) => void
  onSnapshotAgain: (row: DatasetRow) => void
  snapshotPending: boolean
  onDelete: (row: DatasetRow) => void
  deletePending: boolean
}

function ListBody(props: ListBodyProps) {
  const { ws, datasets, error, onRetry, isAdmin, onAddVersion, onSnapshotAgain, snapshotPending } = props
  const { onDelete, deletePending } = props
  if (error && !datasets) {
    return (
      <EmptyState
        title="Không tải được danh sách dataset"
        description={error.message}
        action={
          <Button variant="outline" onClick={onRetry}>
            Thử lại
          </Button>
        }
      />
    )
  }
  if (!datasets) return <p className="text-sm text-muted-foreground">Đang tải...</p>
  if (datasets.length === 0) {
    return (
      <EmptyState
        title="Chưa có dataset nào"
        description={
          isAdmin
            ? 'Kết nối database ở khung phía trên rồi chụp bảng, hoặc nạp file CSV, Parquet, Excel.'
            : 'Admin của workspace nạp dữ liệu; khi có, dataset hiện ở đây.'
        }
      />
    )
  }
  const rows = datasets.map((d) => ({
    id: d.id,
    name: d.name,
    tableName: d.logical_table,
    createdAt: d.created_at,
    snapshot: d.source_table ? { table: d.source_table, at: d.current_version_at } : undefined,
    related: relatedDatasets(d),
  }))
  return (
    <DatasetList
      datasets={rows}
      overviewHref={(row) => overviewHref(ws, row.id)}
      onAddVersion={isAdmin ? onAddVersion : undefined}
      onSnapshotAgain={isAdmin ? onSnapshotAgain : undefined}
      snapshotPending={snapshotPending}
      onDelete={isAdmin ? onDelete : undefined}
      deletePending={deletePending}
      metricsHref={(row) => `${row.id}/metrics`}
      link={RouterLink}
    />
  )
}
