// SPDX-License-Identifier: Apache-2.0
/* components/report/AddToReportMenu.tsx - nút "Thêm vào báo cáo" dưới một câu trả lời đã ghi (có query_id): mở menu các
báo cáo của người dùng, chọn một báo cáo thì câu trả lời vào cuối báo cáo đó; "Báo cáo mới" tạo báo cáo đặt tên theo câu
hỏi rồi thêm câu vào. Thêm xong hiện tên báo cáo kèm link mở nó. Việc gọi API và link do trang truyền vào. */

import { FilePlus2 } from 'lucide-react'
import { menuItemFocus } from '@/components/shell/menuItemFocus'
import type { LinkComponent } from '@/components/shell/types'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'

type Props = {
  /** Báo cáo của người dùng, sửa gần nhất trước; null khi đang tải. */
  reports: { id: string; title: string }[] | null
  onAdd: (reportId: string) => void
  onAddToNew: () => void
  pending: boolean
  /** Báo cáo vừa thêm vào thành công, kèm đường dẫn mở nó. */
  added: { title: string; href: string } | null
  error: string | null
  link: LinkComponent
}

export function AddToReportMenu({ reports, onAdd, onAddToNew, pending, added, error, link: Link }: Props) {
  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="outline" size="sm" disabled={pending}>
            <FilePlus2 aria-hidden />
            {pending ? 'Đang thêm...' : 'Thêm vào báo cáo'}
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" className="w-72">
          <DropdownMenuLabel className="text-xs font-normal text-muted-foreground">Thêm câu trả lời này vào</DropdownMenuLabel>
          {reports === null ? (
            <DropdownMenuItem disabled>Đang tải danh sách báo cáo...</DropdownMenuItem>
          ) : (
            reports.map((report) => (
              <DropdownMenuItem key={report.id} className={menuItemFocus} onSelect={() => onAdd(report.id)}>
                <span className="truncate">{report.title}</span>
              </DropdownMenuItem>
            ))
          )}
          {reports !== null && reports.length > 0 && <DropdownMenuSeparator />}
          <DropdownMenuItem className={menuItemFocus} onSelect={onAddToNew}>
            Báo cáo mới với câu này
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      {added && !error && (
        <p role="status" className="text-xs text-muted-foreground">
          Đã thêm vào{' '}
          <Link href={added.href} className="font-medium text-foreground underline underline-offset-4">
            {added.title}
          </Link>
        </p>
      )}
      {error && (
        <p role="alert" className="text-xs text-destructive">
          {error}
        </p>
      )}
    </div>
  )
}
