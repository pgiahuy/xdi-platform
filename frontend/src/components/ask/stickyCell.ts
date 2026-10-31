// SPDX-License-Identifier: Apache-2.0
/* components/ask/stickyCell.ts - lớp CSS cho ô đầu dòng của bảng phân rã (BreakdownDimension.tsx, BreakdownRows.tsx) đứng
yên khi bảng cuộn ngang trong pane hẹp, để dòng số nào cũng còn thấy tên nhóm. Để ở file .ts riêng vì file .tsx chỉ xuất
component (luật only-export-components của lint). */

import { cn } from '@/lib/utils'

/** Ô tên nhóm đứng yên khi bảng cuộn ngang. Nền phải đặc, không thì số cuộn qua hiện xuyên dưới chữ; bg là nền của dòng
 * (thẻ trắng ở thân bảng, nền xám nhạt ở dòng tiêu đề). */
export function stickyCell(bg: 'bg-card' | 'bg-background'): string {
  return cn('sticky left-0 z-[1]', bg)
}
