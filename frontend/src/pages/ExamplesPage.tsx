// SPDX-License-Identifier: Apache-2.0
/* pages/ExamplesPage.tsx - màn Câu mẫu của workspace (/w/:ws/examples, #212), chỉ cho admin: các câu trả lời thành
viên đánh giá "Tốt", nút duyệt hay bỏ duyệt làm câu mẫu, và nút xuất câu mẫu đã duyệt của một dataset thành file JSON
theo khuôn ca của bộ đánh giá (eval/), để người viết bộ câu xem lại trước khi nhận. Member mở đường dẫn này thì thấy
lời báo; quyền thật do backend kiểm (403). */

import { useState } from 'react'
import { useParams } from 'react-router'
import { useMe } from '@/api/auth'
import { useDatasets } from '@/api/datasets'
import { useExamples, useExportExamples, useSetExampleApproval, type ExampleCandidate } from '@/api/feedback'
import { EmptyState } from '@/components/EmptyState'
import { ExampleList, type ExampleItem } from '@/components/feedback/ExampleList'
import { FormMessage } from '@/components/forms/FormMessage'
import { NativeSelect } from '@/components/metrics/NativeSelect'
import { Button } from '@/components/ui/button'

function toItem(example: ExampleCandidate): ExampleItem {
  return {
    queryId: example.query_id,
    question: example.question,
    datasetName: example.dataset_name,
    sql: example.sql,
    understood: example.understood,
    rowCount: example.row_count,
    why: example.why,
    ratedBy: example.rated_by,
    ratedAt: example.rated_at,
    approvedBy: example.approved_by,
    approvedAt: example.approved_at,
  }
}

function downloadJson(content: unknown, fileName: string): void {
  const blob = new Blob([`${JSON.stringify(content, null, 2)}\n`], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = fileName
  link.click()
  // Như downloadCsv (components/ask/resultCsv.ts): thu hồi ngay thì Safari hủy lượt tải.
  setTimeout(() => URL.revokeObjectURL(url), 10_000)
}

export function ExamplesPage() {
  const { ws = '' } = useParams()
  const me = useMe()
  const isAdmin = me.data?.workspaces.find((m) => m.slug === ws)?.role === 'admin'
  if (me.data && !isAdmin) {
    return <EmptyState title="Chỉ admin xem được màn này" description="Admin của workspace duyệt câu mẫu từ các câu được đánh giá Tốt." />
  }
  return <AdminExamples key={ws} ws={ws} enabled={isAdmin} />
}

function AdminExamples({ ws, enabled }: { ws: string; enabled: boolean }) {
  const examples = useExamples(ws, enabled)
  const approval = useSetExampleApproval(ws)
  const exporter = useExportExamples(ws)
  const datasets = useDatasets(ws)
  const [datasetId, setDatasetId] = useState('')
  const options = (datasets.data ?? []).map((d) => ({ value: d.id, label: d.name }))
  const chosen = datasetId || options[0]?.value || ''
  const exportChosen = () => {
    const name = options.find((o) => o.value === chosen)?.label ?? 'dataset'
    exporter.mutate(chosen, { onSuccess: (cases) => downloadJson(cases, `cau-mau-${name}.json`) })
  }

  return (
    <div className="flex flex-col gap-8">
      <div className="flex flex-col gap-1">
        <h2 className="text-lg font-semibold tracking-tight">Câu mẫu</h2>
        <p className="text-sm text-muted-foreground">
          Câu trả lời thành viên đánh giá Tốt. Câu được duyệt vào prompt làm ví dụ cho các câu hỏi sau trên cùng dataset có
          từ chung; câu chưa duyệt thì không.
        </p>
      </div>

      <section aria-labelledby="export-heading" className="flex flex-col gap-3">
        <h3 id="export-heading" className="text-[15px] font-semibold tracking-tight">
          Xuất cho bộ đánh giá
        </h3>
        <p className="text-sm text-muted-foreground">
          File JSON cùng khuôn với các file ca trong eval/: câu hỏi và SQL mô hình đã viết. Xem lại từng ca trước khi đưa
          vào bộ câu.
        </p>
        <div className="flex flex-wrap items-center gap-2">
          <NativeSelect aria-label="Dataset" options={options} value={chosen} onChange={(e) => setDatasetId(e.target.value)} />
          <Button variant="outline" onClick={exportChosen} disabled={!chosen || exporter.isPending}>
            Xuất câu mẫu đã duyệt
          </Button>
        </div>
        {exporter.error && <FormMessage kind="error">{exporter.error.message}</FormMessage>}
        {exporter.data?.length === 0 && <FormMessage kind="success">Dataset này chưa có câu mẫu nào đã duyệt.</FormMessage>}
      </section>

      <section aria-labelledby="examples-heading" className="flex flex-col gap-3">
        <h3 id="examples-heading" className="text-[15px] font-semibold tracking-tight">
          Câu được đánh giá Tốt
        </h3>
        {examples.error && <FormMessage kind="error">{examples.error.message}</FormMessage>}
        {approval.error && <FormMessage kind="error">{approval.error.message}</FormMessage>}
        {examples.data === undefined && !examples.error && <p className="text-sm text-muted-foreground">Đang tải...</p>}
        {examples.data?.length === 0 && (
          <EmptyState title="Chưa có câu nào được đánh giá Tốt" description="Thành viên bấm Tốt dưới câu trả lời của mình thì câu hiện ở đây." />
        )}
        {examples.data && examples.data.length > 0 && (
          <ExampleList
            items={examples.data.map(toItem)}
            pendingId={approval.isPending ? (approval.variables?.queryId ?? null) : null}
            onApprove={(queryId) => approval.mutate({ queryId, approve: true })}
            onWithdraw={(queryId) => approval.mutate({ queryId, approve: false })}
          />
        )}
      </section>
    </div>
  )
}
