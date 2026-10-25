// SPDX-License-Identifier: Apache-2.0
/* pages/AskEvidence.tsx - pane bằng chứng của trang hỏi: dựng dữ liệu bốn tab từ một AskResponse (hợp đồng ở
backend/src/service/ask_contract.rs), giữ tab đang mở và dòng phân rã vừa bấm, gọi useDrill để lấy dòng nguồn. Phần
hiện ở components/ask/EvidencePane.tsx. AskPage gắn key theo câu trả lời, nên câu trả lời mới thì về tab mặc định và
bỏ lần truy cũ.
Route drill còn trả 501 (#34): tab Dòng nguồn hiện lời báo; ở chế độ dữ liệu mẫu thì hiện vài dòng mẫu. */

import { useState } from 'react'
import type { AskResponse } from '@/api/ask'
import { ApiError } from '@/api/client'
import { drillPageSize, useDrill, type DrillVariables } from '@/api/drill'
import { EvidencePane } from '@/components/ask/EvidencePane'
import type {
  BreakdownView,
  DrillTarget,
  EvidenceTab,
  RowsView,
  SqlView,
  StepView,
  TabData,
} from '@/components/ask/evidenceTypes'
import { cn } from '@/lib/utils'
import type { Answer } from './AskOutcome'
import { buildResultView } from './resultView'

type Props = {
  ws: string
  answer: Answer
  /** Đang hỏi lại: bằng chứng cũ vẫn hiện, mờ đi, không bấm được, như phần câu trả lời. */
  pending: boolean
}

type StepName = AskResponse['steps'][number]['name']

// Nhãn tiếng Việt của tên bước do giao diện đặt (hợp đồng chỉ gửi tên); llm, repair, narrate là ba bước gọi mô hình.
const stepLabels: Record<StepName, string> = {
  context: 'Đọc hồ sơ dữ liệu',
  llm: 'Mô hình viết SQL',
  guard: 'Kiểm SQL (guard)',
  run: 'Chạy SQL',
  repair: 'Mô hình sửa SQL',
  explain: 'Tính phân rã',
  narrate: 'Mô hình viết lời giải thích',
  record: 'Lưu vào lịch sử',
}
const modelSteps: StepName[] = ['llm', 'repair', 'narrate']

export function AskEvidence({ ws, answer, pending }: Props) {
  const { response, variables } = answer
  const [tab, setTab] = useState<EvidenceTab>(() => buildDefaultTab(response))
  const [target, setTarget] = useState<DrillTarget>()

  const drillVariables: DrillVariables | null =
    target && response.query_id
      ? {
          datasetId: variables.datasetId,
          request: {
            query_id: response.query_id,
            breakdown: { slice: target.slice, column: target.column, value: target.value },
            offset: 0,
            limit: drillPageSize,
          },
          fromSamples: variables.fromSamples,
        }
      : null
  const drill = useDrill(ws, drillVariables)

  const rows = ((): RowsView => {
    if (!target) return { state: 'idle' }
    if (drill.error) return { state: 'error', target, message: drillErrorMessage(drill.error) }
    if (!drill.data) return { state: 'loading', target }
    const sample = import.meta.env.DEV && variables.fromSamples === true
    return { state: 'ready', target, sql: drill.data.sql, result: buildResultView(drill.data.result), sample }
  })()

  const breakdown = buildBreakdown(response)
  return (
    <div aria-busy={pending} className={cn(pending && 'pointer-events-none opacity-60')}>
      <EvidencePane
        tab={tab}
        onTabChange={setTab}
        sql={buildSql(response)}
        breakdown={breakdown}
        rows={
          breakdown.data
            ? { data: rows }
            : { data: null, reason: 'Dòng nguồn mở từ bảng phân rã; câu này không có phân rã.' }
        }
        steps={buildSteps(response)}
        onDrill={(next) => {
          setTarget(next)
          setTab('rows')
        }}
      />
    </div>
  )
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

function drillErrorMessage(error: Error): string {
  if (error instanceof ApiError && error.status === 501) {
    return 'Bản này chưa truy được dòng nguồn: backend chưa cài đặt phần này.'
  }
  return error.message
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
      guardNotes: lastGuard?.ok && lastGuard.detail ? [lastGuard.detail] : [],
    },
  }
}

function buildGuardedMissing(response: AskResponse, lastGuard: AskResponse['steps'][number] | undefined): string {
  if (lastGuard && !lastGuard.ok) return `Guard từ chối câu này: ${lastGuard.detail ?? response.message ?? ''}`
  if (response.status === 'needs_choice') return 'Chưa qua guard: cần chọn cách hiểu câu hỏi trước.'
  return 'Câu SQL chưa qua guard.'
}

function buildBreakdown(response: AskResponse): TabData<BreakdownView> {
  const explanation = response.explanation
  if (!explanation) return { data: null, reason: buildBreakdownReason(response) }
  const vp = explanation.volume_price
  return {
    data: {
      measure: explanation.measure,
      baseLabel: explanation.base.label,
      compareLabel: explanation.compare.label,
      baseValue: explanation.base.value,
      compareValue: explanation.compare.value,
      change: explanation.change,
      dimensions: explanation.dimensions.map((dimension) => ({
        column: dimension.column,
        kind: dimension.kind,
        groups: dimension.groups.map((group) => ({
          value: group.value,
          baseValue: group.base_value,
          compareValue: group.compare_value,
          contribution: group.contribution,
        })),
      })),
      volumePrice: vp && {
        volumeLabel: vp.volume_label,
        priceLabel: vp.price_label,
        baseVolume: vp.base_volume,
        compareVolume: vp.compare_volume,
        basePrice: vp.base_price,
        comparePrice: vp.compare_price,
        volumeEffect: vp.volume_effect,
        priceEffect: vp.price_effect,
        ifVolumeUnchanged: vp.if_volume_unchanged,
      },
      summary: explanation.summary,
      summaryByModel: explanation.summary_by_model,
    },
  }
}

function buildBreakdownReason(response: AskResponse): string {
  if (response.status === 'answered') {
    // Câu "tại sao" không dựng được phân rã thì backend gửi cảnh báo kèm lý do; không có cảnh báo là câu thường.
    const unavailable = response.warnings.find((warning) => warning.code === 'explanation_unavailable')
    return unavailable?.message ?? 'Câu này không phải câu hỏi "tại sao", nên không có phân rã.'
  }
  if (response.status === 'needs_choice') return 'Chưa có phân rã: cần chọn cách hiểu câu hỏi trước.'
  return 'Không có phân rã: câu SQL không chạy ra kết quả.'
}

function buildSteps(response: AskResponse): TabData<StepView[]> {
  if (response.steps.length === 0) return { data: null, reason: 'Backend không gửi các bước cho câu này.' }
  return {
    data: response.steps.map((step) => ({
      label: stepLabels[step.name],
      durationMs: step.duration_ms,
      ok: step.ok,
      detail: step.detail,
      byModel: modelSteps.includes(step.name),
    })),
  }
}
