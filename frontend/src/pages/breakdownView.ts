// SPDX-License-Identifier: Apache-2.0
/* pages/breakdownView.ts - đổi explanation của AskResponse sang BreakdownView của components/ask. Tab Phân rã của pane
bằng chứng (AskEvidence.tsx) và phần đầu câu trả lời của câu "tại sao" (ba số lớn, biểu đồ đóng góp, AskOutcome.tsx)
cùng dùng; để ở file .ts riêng vì file .tsx chỉ xuất component (luật only-export-components của lint). */

import type { AskResponse } from '@/api/ask'
import type { BreakdownView, TabData } from '@/components/ask/evidenceTypes'

export function buildBreakdown(response: AskResponse): TabData<BreakdownView> {
  const explanation = response.explanation
  if (!explanation) return { data: null, reason: buildBreakdownReason(response) }
  const vp = explanation.volume_price
  return {
    data: {
      measure: explanation.measure,
      comparison: explanation.comparison,
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
          mixEffect: group.mix_effect,
          rateEffect: group.rate_effect,
        })),
        mixEffect: dimension.mix_effect,
        rateEffect: dimension.rate_effect,
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
