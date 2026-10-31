// SPDX-License-Identifier: Apache-2.0
/* pages/ReportPage.tsx - một báo cáo (/w/:ws/reports/:reportId): phần đầu (tiêu đề, đoạn mở đầu, Xuất PDF, Tải Excel,
Xóa) rồi các mục theo thứ tự, mỗi mục có nút đưa lên, đưa xuống, bỏ. Backend dựng lại mọi mục khi mở (chạy lại câu SQL
đã lưu với quyền của người xem), nên lần mở đầu chờ đủ các câu đó.
Xuất PDF mở hộp in của trình duyệt: kiểu in ở index.css và các lớp print: ẩn thanh bên, thanh trên và mọi nút, mỗi mục
sang trang mới, biểu đồ giữ nguyên. Biểu đồ tải riêng (lazy) như trang hỏi, trong ErrorBoundary riêng từng mục. */

import { lazy, Suspense } from 'react'
import { ArrowDown, ArrowUp, Trash2 } from 'lucide-react'
import { useNavigate, useParams } from 'react-router'
import {
  reportXlsxHref,
  useDeleteReport,
  useRemoveReportItem,
  useReorderReport,
  useReport,
  useUpdateReport,
  type ReportItem,
} from '@/api/reports'
import { ConfirmDialog } from '@/components/ConfirmDialog'
import { EmptyState } from '@/components/EmptyState'
import { ErrorBoundary } from '@/components/ErrorBoundary'
import { FormMessage } from '@/components/forms/FormMessage'
import { ReportHeader } from '@/components/report/ReportHeader'
import { ReportItemCard } from '@/components/report/ReportItemCard'
import { Button } from '@/components/ui/button'
import { buildResultView } from './resultView'
import { buildReportChart, buildReportItemView } from './reportView'
import { workspaceHref, reportsPath } from './workspaceNav'

const ResultChart = lazy(() =>
  import('@/components/ask/ResultChart').then((m) => ({
    default: m.ResultChart,
  })),
)

export function ReportPage() {
  const { ws = '', reportId = '' } = useParams()
  const navigate = useNavigate()
  const report = useReport(ws, reportId)
  const update = useUpdateReport(ws, reportId)
  const remove = useRemoveReportItem(ws, reportId)
  const reorder = useReorderReport(ws, reportId)
  const deleteReport = useDeleteReport(ws)

  if (report.error) return <FormMessage kind="error">{report.error.message}</FormMessage>
  if (report.isPending) {
    return (
      <p role="status" className="text-sm text-muted-foreground">
        Đang chạy lại các câu của báo cáo...
      </p>
    )
  }
  const { items } = report.data
  const summary = report.data.report
  const move = (index: number, offset: number) => {
    const ids = items.map((item) => item.id)
    const [moved] = ids.splice(index, 1)
    ids.splice(index + offset, 0, moved)
    reorder.mutate(ids)
  }
  const mutationError = remove.error ?? reorder.error ?? deleteReport.error

  return (
    <div className="flex max-w-5xl flex-col gap-6 print:max-w-none print:gap-4">
      <ReportHeader
        key={summary.updated_at}
        title={summary.title}
        intro={summary.intro}
        automatic={summary.kind === 'automatic'}
        modelCalls={summary.model_calls}
        renderedAt={report.data.rendered_at}
        onSave={(change) => update.mutate(change)}
        saving={update.isPending}
        saveError={update.error?.message ?? null}
        onPrint={() => window.print()}
        xlsxHref={reportXlsxHref(ws, reportId)}
        deleteButton={
          <ConfirmDialog
            trigger={
              <Button variant="outline" size="sm">
                <Trash2 aria-hidden />
                Xóa báo cáo
              </Button>
            }
            title="Xóa báo cáo này?"
            description="Các câu hỏi của báo cáo vẫn còn trong lịch sử; chỉ báo cáo mất."
            confirmLabel="Xóa"
            onConfirm={() =>
              deleteReport.mutate(reportId, { onSuccess: () => navigate(workspaceHref(ws, reportsPath)) })
            }
          />
        }
      />
      {mutationError && <FormMessage kind="error">{mutationError.message}</FormMessage>}
      {items.length === 0 ? (
        <EmptyState
          title="Báo cáo chưa có mục nào"
          description="Ở trang hỏi, bấm Thêm vào báo cáo dưới một câu trả lời để đưa nó vào đây."
        />
      ) : (
        items.map((item, index) => (
          <ReportItemCard
            key={item.id}
            item={buildReportItemView(item)}
            number={index + 1}
            chart={buildReportChart(item) ? <ItemChart item={item} /> : null}
            actions={
              <>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  aria-label="Đưa lên"
                  disabled={index === 0 || reorder.isPending}
                  onClick={() => move(index, -1)}
                >
                  <ArrowUp aria-hidden />
                </Button>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  aria-label="Đưa xuống"
                  disabled={index === items.length - 1 || reorder.isPending}
                  onClick={() => move(index, 1)}
                >
                  <ArrowDown aria-hidden />
                </Button>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  aria-label="Bỏ khỏi báo cáo"
                  disabled={remove.isPending}
                  onClick={() => remove.mutate(item.id)}
                >
                  <Trash2 aria-hidden />
                </Button>
              </>
            }
          />
        ))
      )}
    </div>
  )
}

/** Biểu đồ của một mục, hay không gì khi mục không có gợi ý biểu đồ. */
function ItemChart({ item }: { item: ReportItem }) {
  const chart = buildReportChart(item)
  if (!chart || !item.result) return null
  return (
    <ErrorBoundary fallback={<p className="text-sm text-muted-foreground">Không vẽ được biểu đồ; số liệu ở bảng bên dưới.</p>}>
      <Suspense fallback={<p className="text-sm text-muted-foreground">Đang tải biểu đồ...</p>}>
        <ResultChart result={buildResultView(item.result)} chart={chart} />
      </Suspense>
    </ErrorBoundary>
  )
}
