// SPDX-License-Identifier: Apache-2.0
/* pages/AskDatasetStatus.tsx - chỗ của ô hỏi khi chưa hỏi được: danh sách dataset đang tải, tải lỗi, hoặc workspace
chưa có dataset nào. Admin thấy nút tới trang Dữ liệu; thành viên khác chỉ thấy lời nhắn, vì họ không nạp được file.
Tách khỏi AskPage.tsx cho file trang ngắn; dùng router (Link) nên ở lớp pages, không ở components. */

import { Link } from 'react-router'
import { EmptyState } from '@/components/EmptyState'
import { Button } from '@/components/ui/button'

type DatasetStatusProps = {
  error: Error | null
  loading: boolean
  onRetry: () => void
  isAdmin: boolean
  datasetsHref: string
}

/** Chưa hỏi được vì danh sách dataset đang tải, tải lỗi, hoặc workspace chưa có dataset nào. */
export function AskDatasetStatus({ error, loading, onRetry, isAdmin, datasetsHref }: DatasetStatusProps) {
  if (error) {
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
  if (loading) return <p className="text-sm text-muted-foreground">Đang tải...</p>
  return (
    <EmptyState
      title="Chưa có dataset nào để hỏi"
      description={
        isAdmin
          ? 'Nạp file CSV, Parquet hoặc Excel ở trang Dữ liệu.'
          : 'Admin của workspace nạp dữ liệu; khi có, hỏi ở đây.'
      }
      action={
        isAdmin && (
          <Button variant="outline" asChild>
            <Link to={datasetsHref}>Tới trang Dữ liệu</Link>
          </Button>
        )
      }
    />
  )
}
