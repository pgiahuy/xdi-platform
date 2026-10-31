// SPDX-License-Identifier: Apache-2.0
/* pages/AddToReportButton.tsx - nối nút "Thêm vào báo cáo" (components/report/AddToReportMenu.tsx) với API báo cáo cho
một câu trả lời có query_id, ở cả lượt vừa hỏi lẫn lượt đã lưu của cuộc trò chuyện. "Báo cáo mới" đặt tiêu đề theo
câu hỏi (người dùng sửa sau ở trang báo cáo). */

import { useState } from 'react'
import { useAddReportItem, useCreateReport, useReports } from '@/api/reports'
import { AddToReportMenu } from '@/components/report/AddToReportMenu'
import { RouterLink } from './RouterLink'
import { reportHref } from './workspaceNav'

/** Tiêu đề tối đa của báo cáo backend nhận là 200 ký tự; câu hỏi dài hơn thì cắt. */
const TITLE_MAX = 200

type Props = {
  ws: string
  queryId: string
  question: string
}

export function AddToReportButton({ ws, queryId, question }: Props) {
  const reports = useReports(ws)
  const add = useAddReportItem(ws)
  const create = useCreateReport(ws)
  const [added, setAdded] = useState<{ title: string; href: string } | null>(null)
  const addTo = (reportId: string, title: string) =>
    add.mutate({ reportId, queryId }, { onSuccess: () => setAdded({ title, href: reportHref(ws, reportId) }) })
  const addToNew = () => {
    const title = question.trim().slice(0, TITLE_MAX) || 'Báo cáo mới'
    create.mutate({ title, intro: '' }, { onSuccess: (report) => addTo(report.id, report.title) })
  }
  const error = add.error ?? create.error
  return (
    <AddToReportMenu
      reports={reports.data ? reports.data.map((r) => ({ id: r.id, title: r.title })) : null}
      onAdd={(id) => addTo(id, reports.data?.find((r) => r.id === id)?.title ?? '')}
      onAddToNew={addToNew}
      pending={add.isPending || create.isPending}
      added={added}
      error={error ? error.message : null}
      link={RouterLink}
    />
  )
}
