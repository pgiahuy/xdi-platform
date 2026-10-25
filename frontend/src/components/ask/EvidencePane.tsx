// SPDX-License-Identifier: Apache-2.0
/* components/ask/EvidencePane.tsx - pane bằng chứng của trang hỏi, bốn tab: SQL, Phân rã, Dòng nguồn, Các bước. Chỉ
nhận props (AGENTS.md mục 3): trang giữ tab đang mở và lần truy dòng nguồn, dựng dữ liệu từ response
(pages/AskEvidence.tsx). Bốn tab luôn có mặt để phím mũi tên đi qua cùng một thứ tự với mọi câu trả lời; tab không có
dữ liệu ghi lý do thay vì biến mất. Tab dùng Tabs của shadcn (Radix): mũi tên trái, phải đổi tab, Tab vào nội dung. */

import { useRef } from 'react'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { EvidenceBreakdown } from './EvidenceBreakdown'
import { EvidenceRows } from './EvidenceRows'
import { EvidenceSql } from './EvidenceSql'
import { EvidenceSteps } from './EvidenceSteps'
import type { BreakdownView, DrillTarget, EvidenceTab, RowsView, SqlView, StepView, TabData } from './evidenceTypes'

type Props = {
  tab: EvidenceTab
  onTabChange: (tab: EvidenceTab) => void
  sql: TabData<SqlView>
  breakdown: TabData<BreakdownView>
  /** Dòng nguồn chỉ mở từ bảng phân rã, nên không có phân rã thì tab này có lý do riêng. */
  rows: TabData<RowsView>
  steps: TabData<StepView[]>
  onDrill: (target: DrillTarget) => void
}

const tabLabels: Record<EvidenceTab, string> = {
  sql: 'SQL',
  breakdown: 'Phân rã',
  rows: 'Dòng nguồn',
  steps: 'Các bước',
}

export function EvidencePane({ tab, onTabChange, sql, breakdown, rows, steps, onDrill }: Props) {
  // Bấm số trong bảng phân rã thì tab Phân rã đóng lại cùng nút vừa bấm, focus rơi về đầu trang. Chuyển focus sang
  // nội dung tab Dòng nguồn để người dùng bàn phím và trình đọc màn hình ở đúng chỗ kết quả hiện ra. Khung nội dung
  // của Radix luôn có trong DOM nhưng còn hidden cho tới khi Tabs render xong tab mới, focus vào phần tử hidden thì
  // trình duyệt bỏ qua; nên focus ở khung hình sau.
  const rowsPanel = useRef<HTMLDivElement>(null)
  const drill = (target: DrillTarget) => {
    onDrill(target)
    requestAnimationFrame(() => rowsPanel.current?.focus())
  }

  const sliceLabels = breakdown.data
    ? { base: breakdown.data.baseLabel, compare: breakdown.data.compareLabel }
    : { base: '', compare: '' }

  return (
    <Tabs value={tab} onValueChange={(value) => onTabChange(value as EvidenceTab)} className="min-w-0">
      <TabsList className="w-full" aria-label="Bằng chứng">
        {(Object.keys(tabLabels) as EvidenceTab[]).map((key) => (
          <TabsTrigger key={key} value={key} className="px-1 text-xs sm:text-sm">
            {tabLabels[key]}
          </TabsTrigger>
        ))}
      </TabsList>
      <TabsContent value="sql" className="pt-2">
        {sql.data ? <EvidenceSql sql={sql.data} /> : <Reason text={sql.reason} />}
      </TabsContent>
      <TabsContent value="breakdown" className="pt-2">
        {breakdown.data ? (
          <EvidenceBreakdown breakdown={breakdown.data} onDrill={drill} />
        ) : (
          <Reason text={breakdown.reason} />
        )}
      </TabsContent>
      <TabsContent value="rows" ref={rowsPanel} className="pt-2">
        {rows.data ? <EvidenceRows rows={rows.data} sliceLabels={sliceLabels} /> : <Reason text={rows.reason} />}
      </TabsContent>
      <TabsContent value="steps" className="pt-2">
        {steps.data ? <EvidenceSteps steps={steps.data} /> : <Reason text={steps.reason} />}
      </TabsContent>
    </Tabs>
  )
}

function Reason({ text }: { text: string }) {
  return <p className="text-sm text-muted-foreground">{text}</p>
}
