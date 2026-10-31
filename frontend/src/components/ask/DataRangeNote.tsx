// SPDX-License-Identifier: Apache-2.0
/* components/ask/DataRangeNote.tsx - một dòng nhỏ dưới lời trả lời: dữ liệu có từ ngày nào đến ngày nào
(AskResponse.data_range, backend lấy từ hồ sơ cột thời gian lúc nạp). Câu hỏi về kỳ quá ngày cuối của dữ liệu
(partial) thì nói rõ dữ liệu mới có đến đâu, để người hỏi biết vì sao số nhỏ hay không có số. AskOutcome.tsx đặt dòng
này dưới lời trả lời của cả câu thường lẫn câu "tại sao". */

import { formatCell } from './cellFormat'

type Props = {
  /** Ngày dạng YYYY-MM-DD. */
  from: string
  to: string
  /** Tên tiếng Việt của cột thời gian, hay tên cột khi hồ sơ không có bí danh. */
  label: string
  partial: boolean
}

export function DataRangeNote({ from, to, label, partial }: Props) {
  const last = formatCell(to, 'date')
  const text = partial
    ? `Dữ liệu mới có đến ${last} (theo ${label}), kỳ được hỏi chưa đủ dữ liệu.`
    : `Dữ liệu có từ ${formatCell(from, 'date')} đến ${last} (theo ${label}).`
  return <p className="text-xs text-muted-foreground">{text}</p>
}
