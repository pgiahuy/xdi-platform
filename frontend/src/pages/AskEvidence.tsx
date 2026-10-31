// SPDX-License-Identifier: Apache-2.0
/* pages/AskEvidence.tsx - pane bằng chứng của trang hỏi: dựng dữ liệu bốn tab từ một AskResponse (hợp đồng ở
backend/src/service/ask/contract.rs), gọi useDrill để lấy dòng nguồn của chỗ vừa bấm. Phần hiện ở
components/ask/EvidencePane.tsx. Tab đang mở, chỗ vừa bấm và trang dòng nguồn do AskPage giữ (evidenceView.ts), vì ô
của bảng kết quả nằm ngoài pane cũng mở tab Dòng nguồn.
Tab Dòng nguồn gọi route drill (backend/src/service/drill/mod.rs) với khóa của chỗ vừa bấm: breakdown cho dòng phân rã,
cell cho ô số đo; lỗi thì hiện lời báo của backend. */

import type { AskResponse } from '@/api/ask'
import { drillPageSize, useDrill, type DrillRequest, type DrillVariables } from '@/api/drill'
import { EvidencePane } from '@/components/ask/EvidencePane'
import type {
  DrillTarget,
  EvidenceTab,
  RowsView,
  SqlView,
  StepView,
  TabData,
} from '@/components/ask/evidenceTypes'
import { cn } from '@/lib/utils'
import type { Answer } from './AskOutcome'
import { buildBreakdown } from './breakdownView'
import { buildLeadFirst, buildLeadNames } from './drillColumns'
import type { EvidenceView } from './evidenceView'
import { buildCellIndexes, buildResultView } from './resultView'

type Props = {
  ws: string
  answer: Answer
  /** Đang hỏi lại: bằng chứng cũ vẫn hiện, mờ đi, không bấm được, như phần câu trả lời. */
  pending: boolean
  view: EvidenceView
}

type StepName = AskResponse['steps'][number]['name']

// Nhãn tiếng Việt của tên bước do giao diện đặt (hợp đồng chỉ gửi tên); classify, llm, repair, narrate là bốn bước gọi
// mô hình.
const stepLabels: Record<StepName, string> = {
  context: 'Đọc hồ sơ dữ liệu',
  classify: 'Phân loại câu hỏi',
  llm: 'Mô hình viết SQL',
  guard: 'Kiểm SQL (guard)',
  run: 'Chạy SQL',
  repair: 'Mô hình sửa SQL',
  explain: 'Tính phân rã',
  narrate: 'Mô hình viết lời, qua hai cổng kiểm',
  record: 'Lưu vào lịch sử',
}
const modelSteps: StepName[] = ['classify', 'llm', 'repair', 'narrate']

export function AskEvidence({ ws, answer, pending, view }: Props) {
  const { response, variables } = answer
  const { target, offset } = view

  const drillVariables: DrillVariables | null =
    target && response.query_id
      ? {
          datasetId: variables.datasetId,
          request: { query_id: response.query_id, ...buildDrillKey(target), offset, limit: drillPageSize },
        }
      : null
  const drill = useDrill(ws, drillVariables)

  const rows = ((): RowsView | null => {
    if (!target) {
      const hint = buildRowsHint(Boolean(response.explanation), buildCellIndexes(response) !== null)
      return hint ? { state: 'idle', hint } : null
    }
    if (drill.error) return { state: 'error', target, message: drill.error.message }
    if (!drill.data) return { state: 'loading', target }
    // result.total_rows của drill chỉ đếm dòng của trang này; tổng thật ở total_rows ngoài, nên bảng ghi "N trên M".
    const total = drill.data.total_rows
    const page = buildLeadFirst(buildResultView(drill.data.result), buildLeadNames(target, response))
    const result = { ...page, totalRows: total, truncated: page.rows.length < total }
    return {
      state: 'ready',
      target,
      sql: drill.data.sql,
      result,
      page: { offset, limit: drillPageSize, total },
    }
  })()

  return (
    <div aria-busy={pending} className={cn(pending && 'pointer-events-none opacity-60')}>
      <EvidencePane
        tab={view.tab ?? buildDefaultTab(response)}
        onTabChange={view.setTab}
        sql={buildSql(response)}
        breakdown={buildBreakdown(response)}
        rows={rows ? { data: rows } : { data: null, reason: 'Câu này không có ô số đo hay phân rã để xem dòng nguồn.' }}
        steps={buildSteps(response)}
        onDrill={view.drill}
        onRowsPageChange={view.setOffset}
      />
    </div>
  )
}

// Giá trị gửi lại đúng như đã nhận (kể cả chuỗi decimal, null): backend so bằng giá trị đó trên biểu thức nhóm.
function buildDrillKey(target: DrillTarget): Pick<DrillRequest, 'cell' | 'breakdown'> {
  if (target.from === 'breakdown') {
    return { breakdown: { slice: target.slice, column: target.column, value: target.value } }
  }
  return { cell: target.keys.map((key) => ({ column: key.column, value: key.value })) }
}

/** Lời nhắc khi chưa bấm gì, theo những chỗ bấm được của câu trả lời; null khi không có chỗ nào. */
function buildRowsHint(hasBreakdown: boolean, hasCells: boolean): string | null {
  if (hasBreakdown && hasCells) {
    return (
      'Bấm vào một con số trong bảng kết quả, hoặc con số của một kỳ ở tab Phân rã, để xem những dòng dữ liệu tạo ' +
      'ra con số đó.'
    )
  }
  if (hasBreakdown) return 'Bấm vào con số của một kỳ ở tab Phân rã để xem những dòng dữ liệu tạo ra con số đó.'
  if (hasCells) return 'Bấm vào một con số trong bảng kết quả để xem những dòng dữ liệu tạo ra con số đó.'
  return null
}

/** Tab mở sẵn: chỗ trả lời câu "vì sao câu trả lời là thế này" nhanh nhất cho từng status. */
function buildDefaultTab(response: AskResponse): EvidenceTab {
  switch (response.status) {
    case 'answered':
      return response.explanation ? 'breakdown' : 'sql'
    case 'rejected':
      return 'sql'
    case 'needs_choice':
    case 'failed':
      return 'steps'
  }
}

function buildSql(response: AskResponse): TabData<SqlView> {
  const { steps } = response
  if (!response.sql_generated && !response.sql_guarded) {
    const reason =
      response.status === 'needs_choice'
        ? 'Chưa có SQL: cần chọn cách hiểu câu hỏi trước khi mô hình viết SQL.'
        : 'Backend không gửi câu SQL cho câu này.'
    return { data: null, reason }
  }
  // Bước guard cuối cùng là bước kiểm đúng câu đang hiện: sau một vòng sửa có hai bước guard.
  const lastGuard = steps.filter((step) => step.name === 'guard').at(-1)
  return {
    data: {
      generated: response.sql_generated,
      repaired: steps.some((step) => step.name === 'repair'),
      guarded: response.sql_guarded,
      guardedMissing: response.sql_guarded ? null : buildGuardedMissing(response, lastGuard),
      tablesRead: response.tables_read.map((table) => table.logical_table),
    },
  }
}

function buildGuardedMissing(response: AskResponse, lastGuard: AskResponse['steps'][number] | undefined): string {
  if (lastGuard && !lastGuard.ok) return `Guard từ chối câu này: ${lastGuard.detail ?? response.message ?? ''}`
  if (response.status === 'needs_choice') return 'Chưa qua guard: cần chọn cách hiểu câu hỏi trước.'
  return 'Câu SQL chưa qua guard.'
}

function buildSteps(response: AskResponse): TabData<StepView[]> {
  if (response.steps.length === 0) return { data: null, reason: 'Backend không gửi các bước cho câu này.' }
  // Loại câu thuộc bước phân loại; kết luận hai cổng kiểm thuộc bước viết lời: của câu "tại sao" hay của phần giải thích
  // câu thường (phân rã tự động của câu thường không qua bước viết lời của câu "tại sao", nên summary_verdict null).
  // Backend cũ không có các trường này.
  const questionType = response.question_type ?? null
  const verdict = response.explanation?.summary_verdict ?? response.insight?.summary_verdict ?? null
  return {
    data: response.steps.map((step) => ({
      label: stepLabels[step.name],
      durationMs: step.duration_ms,
      ok: step.ok,
      // Bước phân loại đạt thì cờ đã nói loại câu, lý do của mô hình chỉ lặp lại điều đó; bước lỗi giữ chi tiết để thấy vì
      // sao phải dùng luật từ khóa.
      detail: step.name === 'classify' && step.ok && questionType ? null : step.detail,
      byModel: modelSteps.includes(step.name),
      flag: step.name === 'classify' ? questionType : step.name === 'narrate' ? verdict : null,
    })),
  }
}
