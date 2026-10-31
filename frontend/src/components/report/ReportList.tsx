// SPDX-License-Identifier: Apache-2.0
/* components/report/ReportList.tsx - danh sách báo cáo của người dùng trên trang Báo cáo: tiêu đề (link mở báo cáo),
loại (tự gom hay tự động), số mục, lúc sửa gần nhất. Link do trang truyền vào (LinkComponent), component không biết
router. */

import type { LinkComponent } from '@/components/shell/types'
import type { ReportListEntry } from './types'

const dateFormat = new Intl.DateTimeFormat('vi-VN', { dateStyle: 'short', timeStyle: 'short' })

type Props = {
  reports: ReportListEntry[]
  link: LinkComponent
}

export function ReportList({ reports, link: Link }: Props) {
  return (
    <ul className="flex flex-col divide-y rounded-xl border bg-card">
      {reports.map((report) => (
        <li key={report.id} className="flex flex-col gap-0.5 px-4 py-3">
          <Link href={report.href} className="font-medium text-foreground underline-offset-4 hover:underline">
            {report.title}
          </Link>
          <p className="text-xs text-muted-foreground">
            {report.automatic ? 'Tạo tự động' : 'Tự gom'}, {report.itemCount} mục, sửa lúc{' '}
            {dateFormat.format(new Date(report.updatedAt))}
          </p>
        </li>
      ))}
    </ul>
  )
}
