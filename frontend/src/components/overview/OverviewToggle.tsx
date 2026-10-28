// SPDX-License-Identifier: Apache-2.0
/* components/overview/OverviewToggle.tsx - nút mở, đóng khung "Tổng quan dữ liệu" cạnh ô chọn dataset của trang hỏi.
Khung đóng sẵn: người đã quen dataset vào trang để hỏi, khung chỉ mở khi cần xem dataset có gì. */

import { ChartColumn } from 'lucide-react'
import { Button } from '@/components/ui/button'

type Props = {
  open: boolean
  onToggle: () => void
  /** id của khung, cho aria-controls. */
  controls: string
  disabled: boolean
}

export function OverviewToggle({ open, onToggle, controls, disabled }: Props) {
  return (
    <Button
      variant={open ? 'secondary' : 'outline'}
      size="sm"
      aria-expanded={open}
      aria-controls={controls}
      onClick={onToggle}
      disabled={disabled}
    >
      <ChartColumn aria-hidden />
      Tổng quan dữ liệu
    </Button>
  )
}
