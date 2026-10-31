// SPDX-License-Identifier: Apache-2.0
/* pages/ReportsPage.tsx - trang Báo cáo của workspace (/w/:ws/reports): báo cáo của người dùng (chỉ người tạo thấy báo
cáo của mình, như cuộc trò chuyện), form tạo báo cáo trống và form "Tạo báo cáo tự động" cho một dataset. Tạo xong thì
mở ngay báo cáo vừa tạo. Báo cáo dựng lại số liệu mỗi lần mở (backend chạy lại câu SQL đã lưu). */

import { useNavigate, useParams } from 'react-router'
import { useCatalog } from '@/api/catalog'
import { useCreateAutoReport, useCreateReport, useReports } from '@/api/reports'
import { EmptyState } from '@/components/EmptyState'
import { FormMessage } from '@/components/forms/FormMessage'
import { NewReportForm } from '@/components/report/NewReportForm'
import { ReportList } from '@/components/report/ReportList'
import { buildReportEntry } from './reportView'
import { RouterLink } from './RouterLink'
import { reportHref } from './workspaceNav'

export function ReportsPage() {
  const { ws = '' } = useParams()
  const navigate = useNavigate()
  const reports = useReports(ws)
  const catalog = useCatalog(ws)
  const create = useCreateReport(ws)
  const automatic = useCreateAutoReport(ws)
  const datasets = (catalog.data ?? []).map((d) => ({ value: d.id, label: d.name }))
  const open = (id: string) => navigate(reportHref(ws, id))

  return (
    <div className="flex max-w-5xl flex-col gap-8">
      <div className="flex flex-col gap-1">
        <h2 className="text-lg font-semibold tracking-tight">Báo cáo</h2>
        <p className="text-sm text-muted-foreground">
          Gom các câu trả lời đã chọn thành một báo cáo có tiêu đề và đoạn mở đầu, hay để hệ thống tạo báo cáo tự động cho
          một dataset. Mỗi mục giữ câu hỏi, cách tính, câu SQL, phiên bản dữ liệu và kết quả kiểm; xuất được PDF và Excel.
        </p>
      </div>

      <NewReportForm
        datasets={datasets}
        onCreate={(title) => create.mutate({ title, intro: '' }, { onSuccess: (r) => open(r.id) })}
        creating={create.isPending}
        createError={create.error?.message ?? null}
        onCreateAutomatic={(datasetId) => automatic.mutate(datasetId, { onSuccess: (r) => open(r.report.id) })}
        building={automatic.isPending}
        buildError={automatic.error?.message ?? null}
      />

      <section aria-labelledby="reports-heading" className="flex flex-col gap-3">
        <h3 id="reports-heading" className="text-[15px] font-semibold tracking-tight">
          Báo cáo của bạn
        </h3>
        {reports.error ? (
          <FormMessage kind="error">{reports.error.message}</FormMessage>
        ) : reports.isPending ? (
          <p className="text-sm text-muted-foreground">Đang tải...</p>
        ) : reports.data.length === 0 ? (
          <EmptyState
            title="Chưa có báo cáo nào"
            description="Tạo một báo cáo ở trên, hay bấm Thêm vào báo cáo dưới một câu trả lời."
          />
        ) : (
          <ReportList
            reports={reports.data.map((r) => buildReportEntry(r, reportHref(ws, r.id)))}
            link={RouterLink}
          />
        )}
      </section>
    </div>
  )
}
