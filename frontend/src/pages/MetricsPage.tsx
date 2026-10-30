// SPDX-License-Identifier: Apache-2.0
/* pages/MetricsPage.tsx - màn Chỉ số của một dataset (/w/:ws/datasets/:datasetId/metrics, mở từ trang Dữ liệu): bảng
chỉ số, form thêm và sửa, cách gộp mặc định của cột số đo. Mọi thành viên xem; nút thêm, sửa, xóa, ô đổi cách gộp chỉ
hiện với admin, quyền thật do backend kiểm. Chỉ số lưu xong dùng ngay ở câu hỏi kế tiếp trên dataset này. */

import { useMemo, useState } from 'react'
import { useParams } from 'react-router'
import { useMe } from '@/api/auth'
import { ApiError } from '@/api/client'
import { useDatasets } from '@/api/datasets'
import { useDeleteMetric, useMetrics, useSaveMetric, useSetAggregation, type MetricsResponse } from '@/api/metrics'
import { EmptyState } from '@/components/EmptyState'
import type { FormError } from '@/components/forms/formError'
import { FormMessage } from '@/components/forms/FormMessage'
import { AggregationList } from '@/components/metrics/AggregationList'
import { MetricForm } from '@/components/metrics/MetricForm'
import { MetricTable } from '@/components/metrics/MetricTable'
import type { ColumnItem, MetricDraft, MetricField, MetricItem } from '@/components/metrics/types'
import { Button } from '@/components/ui/button'
import { RouterLink } from './RouterLink'
import { workspaceHref } from './workspaceNav'

const metricFields: MetricField[] = ['name', 'description', 'expression', 'filter', 'time_column', 'unit']

function toItems(data: MetricsResponse): { metrics: MetricItem[]; columns: ColumnItem[] } {
  return {
    metrics: data.metrics.map((m) => ({
      id: m.id,
      name: m.name,
      description: m.description,
      expression: m.expression,
      filter: m.filter,
      timeColumn: m.time_column,
      unit: m.unit,
      version: m.version,
    })),
    columns: data.columns.map((c) => ({
      name: c.name,
      dataType: c.data_type,
      role: c.role,
      description: c.description,
      aggregation: c.aggregation,
    })),
  }
}

const optional = (text: string) => (text.trim() ? text.trim() : null)

export function MetricsPage() {
  const { ws = '', datasetId = '' } = useParams()
  // Đổi workspace hay dataset thì dựng lại từ đầu, để form đang soạn không đi theo sang dataset khác.
  return <DatasetMetrics key={`${ws}/${datasetId}`} ws={ws} datasetId={datasetId} />
}

function DatasetMetrics({ ws, datasetId }: { ws: string; datasetId: string }) {
  const me = useMe()
  const datasets = useDatasets(ws)
  const metrics = useMetrics(ws, datasetId)
  const save = useSaveMetric(ws, datasetId)
  const remove = useDeleteMetric(ws, datasetId)
  const setAggregation = useSetAggregation(ws, datasetId)
  // Form đang mở: editing là chỉ số đang sửa, không có là thêm mới; key đổi mỗi lần mở để form đọc lại giá trị đầu.
  const [editor, setEditor] = useState<{ key: number; editing?: MetricItem }>()
  const isAdmin = me.data?.workspaces.find((m) => m.slug === ws)?.role === 'admin'
  const dataset = datasets.data?.find((d) => d.id === datasetId)

  const formError = useMemo<FormError<MetricField> | undefined>(() => {
    const error = save.error
    if (!error) return undefined
    const field = error instanceof ApiError ? metricFields.find((f) => f === error.field) : undefined
    return { message: error.message, field }
  }, [save.error])

  const open = (editing?: MetricItem) => {
    save.reset()
    setEditor({ key: Date.now(), editing })
  }
  const submit = (draft: MetricDraft) => {
    const request = {
      name: draft.name,
      description: draft.description,
      expression: draft.expression,
      filter: optional(draft.filter),
      time_column: optional(draft.timeColumn),
      unit: optional(draft.unit),
    }
    save.mutate({ id: editor?.editing?.id, request }, { onSuccess: () => setEditor(undefined) })
  }

  const back = (
    <RouterLink href={workspaceHref(ws, 'datasets')} className="text-sm text-muted-foreground underline-offset-4 hover:underline">
      Về trang Dữ liệu
    </RouterLink>
  )
  if (metrics.error && !metrics.data) {
    return (
      <div className="flex flex-col gap-4">
        {back}
        <EmptyState title="Không tải được chỉ số" description={metrics.error.message} />
      </div>
    )
  }
  if (!metrics.data) return <p className="text-sm text-muted-foreground">Đang tải...</p>
  const { metrics: items, columns } = toItems(metrics.data)
  const measures = columns.filter((c) => c.role === 'measure')

  return (
    <div className="flex max-w-4xl flex-col gap-8">
      <div className="flex flex-col gap-1">
        {back}
        <h2 className="text-lg font-semibold tracking-tight">Chỉ số của {dataset?.name ?? 'dataset'}</h2>
        <p className="text-sm text-muted-foreground">
          Câu hỏi nhắc tới tên chỉ số thì mô hình tính đúng theo công thức ở đây, thay vì tự chọn cột.
        </p>
      </div>

      <section aria-labelledby="metrics-heading" className="flex flex-col gap-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h3 id="metrics-heading" className="text-[15px] font-semibold tracking-tight">
            Chỉ số
          </h3>
          {isAdmin && !editor && columns.length > 0 && <Button onClick={() => open()}>Thêm chỉ số</Button>}
        </div>
        {editor && (
          <MetricForm
            key={editor.key}
            columns={columns}
            editing={editor.editing}
            pending={save.isPending}
            error={formError}
            onSubmit={submit}
            onCancel={() => setEditor(undefined)}
          />
        )}
        {items.length > 0 ? (
          <MetricTable
            metrics={items}
            onEdit={isAdmin ? (m) => open(m) : undefined}
            onDelete={isAdmin ? (m) => remove.mutate(m.id) : undefined}
            deletePending={remove.isPending}
          />
        ) : (
          <EmptyState
            title="Chưa có chỉ số nào"
            description={isAdmin ? 'Thêm chỉ số để câu hỏi dùng đúng công thức của tổ chức.' : 'Admin của workspace định nghĩa chỉ số.'}
          />
        )}
        {remove.error && <FormMessage kind="error">{remove.error.message}</FormMessage>}
      </section>

      {measures.length > 0 && (
        <section aria-labelledby="aggregation-heading" className="flex flex-col gap-3">
          <h3 id="aggregation-heading" className="text-[15px] font-semibold tracking-tight">
            Cách gộp của cột số đo
          </h3>
          <p className="text-sm text-muted-foreground">
            Cột mô tả từng dòng (tuổi, điểm, số ngày) cộng lại thì vô nghĩa: đặt Trung bình để câu hỏi và câu vì sao
            dùng trung bình mỗi dòng.
          </p>
          <AggregationList
            columns={measures}
            onChange={isAdmin ? (column, aggregation) => setAggregation.mutate({ column, aggregation }) : undefined}
            pendingColumn={setAggregation.isPending ? setAggregation.variables?.column : undefined}
          />
          {setAggregation.error && <FormMessage kind="error">{setAggregation.error.message}</FormMessage>}
        </section>
      )}
    </div>
  )
}
