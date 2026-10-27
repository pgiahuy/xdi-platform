// SPDX-License-Identifier: Apache-2.0
/* pages/evidenceView.ts - trạng thái pane bằng chứng của trang hỏi: tab đang mở, chỗ vừa bấm để xem dòng nguồn
(DrillTarget) và trang dòng nguồn đang xem. Giữ ở AskPage chứ không trong AskEvidence vì có hai chỗ bấm: dòng phân rã
nằm trong pane, ô số đo nằm trong bảng kết quả ở cột chính (AskOutcome); cả hai cùng mở tab Dòng nguồn qua drill().
Trạng thái gắn với khóa của câu trả lời: khóa đổi (câu trả lời mới) thì về tab mặc định và bỏ chỗ đã bấm, như khi pane
còn gắn key theo câu trả lời. */

import { useState } from 'react'
import type { DrillTarget, EvidenceTab } from '@/components/ask/evidenceTypes'

type ViewState = {
  key: string
  /** null: tab mặc định theo câu trả lời (AskEvidence chọn). */
  tab: EvidenceTab | null
  target?: DrillTarget
  offset: number
}

export type EvidenceView = {
  tab: EvidenceTab | null
  target?: DrillTarget
  /** Vị trí dòng đầu của trang dòng nguồn đang xem. */
  offset: number
  setTab: (tab: EvidenceTab) => void
  /** Mở tab Dòng nguồn của chỗ vừa bấm, từ trang đầu. key: khóa của câu trả lời chứa chỗ bấm khi nó khác câu đang
   * hiện, như khi bấm ô của một lượt khác trong cuộc trò chuyện (AskPage chọn lượt đó cùng lúc). */
  drill: (target: DrillTarget, key?: string) => void
  setOffset: (offset: number) => void
}

export function useEvidenceView(answerKey: string): EvidenceView {
  const [state, setState] = useState<ViewState>()
  const current: ViewState = state?.key === answerKey ? state : { key: answerKey, tab: null, offset: 0 }
  return {
    tab: current.tab,
    target: current.target,
    offset: current.offset,
    setTab: (tab) => setState({ ...current, tab }),
    drill: (target, key = answerKey) => setState({ key, tab: 'rows', target, offset: 0 }),
    setOffset: (offset) => setState({ ...current, offset }),
  }
}
