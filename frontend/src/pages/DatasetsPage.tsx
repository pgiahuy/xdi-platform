// SPDX-License-Identifier: Apache-2.0
/* pages/DatasetsPage.tsx - trang Dữ liệu của workspace: danh sách dataset, khung nạp file và khung kết nối database
(DatabaseSources.tsx) cho admin.
Vai lấy từ /auth/me (đã có trong bộ đệm, AppLayout vừa đọc). Ẩn khung nạp với member chỉ để giao diện gọn;
quyền thật nằm ở backend, member gọi thẳng API vẫn nhận 403.
Hồ sơ cột (kiểu, null, min, max, vai trò cột) chưa có endpoint (#22); tới lúc đó cột chỉ hiện ngay sau khi nạp. */

import { useRef, useState } from 'react'
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
  const uploadHeading = useRef<HTMLHeadingElement>(null)

  const isAdmin = me.data?.workspaces.find((m) => m.slug === ws)?.role === 'admin'

  const addVersion = (row: DatasetRow) => {
    setTarget(datasets.data?.find((d) => d.id === row.id))
    // Khung nạp nằm đầu trang, có thể đã cuộn khuất; đưa tiêu điểm lên đó để người dùng thấy ngay mình đang nạp cho
    // dataset nào, và người dùng bàn phím không phải lần ngược lên.
    uploadHeading.current?.focus()
    uploadHeading.current?.scrollIntoView({ block: 'start' })
  }

  // Chụp lại dùng đúng bảng của lần chụp trước (backend nhớ theo dataset), nên chỉ cần nguồn và dataset.
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
      {isAdmin && (
        <DatasetUpload
          ws={ws}
          target={target}
          onClearTarget={(id) => setTarget((current) => (id === undefined || current?.id === id ? undefined : current))}
          headingRef={uploadHeading}
        />
      )}
      {isAdmin && <DatabaseSources ws={ws} />}

      <section aria-labelledby="dataset-list-heading" className="flex flex-col gap-3">
        <h2 id="dataset-list-heading" className="text-[15px] font-semibold tracking-tight">
          Dataset của workspace
        </h2>
        <ListBody
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

type ListBodyProps = {
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
  const { datasets, error, onRetry, isAdmin, onAddVersion, onSnapshotAgain, snapshotPending } = props
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
            ? 'Nạp file CSV, Parquet hoặc Excel ở khung phía trên.'
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
  }))
  return (
    <DatasetList
      datasets={rows}
      onAddVersion={isAdmin ? onAddVersion : undefined}
      onSnapshotAgain={isAdmin ? onSnapshotAgain : undefined}
      snapshotPending={snapshotPending}
      onDelete={isAdmin ? onDelete : undefined}
      deletePending={deletePending}
    />
  )
}
