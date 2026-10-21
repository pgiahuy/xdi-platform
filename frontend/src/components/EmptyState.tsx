// SPDX-License-Identifier: Apache-2.0
/* components/EmptyState.tsx - khối báo trạng thái khi chưa có gì để hiện: tiêu đề, một câu giải thích, một hành động.
Dùng trong trang (chưa có dữ liệu) và trong StatusScreen (cả màn hình, khi chưa vào được khung giao diện). */

import type { ReactNode } from 'react'

type Props = {
  title: string
  description?: string
  action?: ReactNode
  /** h1 khi khối này là cả màn hình; trong trang thì h1 đã nằm trên thanh trên nên dùng h2. */
  heading?: 'h1' | 'h2'
}

export function EmptyState({ title, description, action, heading: Heading = 'h2' }: Props) {
  return (
    <div className="flex max-w-md flex-col items-start gap-2">
      <Heading className="text-base font-semibold">{title}</Heading>
      {description && <p className="text-sm text-muted-foreground">{description}</p>}
      {action && <div className="mt-2">{action}</div>}
    </div>
  )
}
