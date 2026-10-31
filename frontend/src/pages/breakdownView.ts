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
      baseRows: explanation.base.rows,
      compareRows: explanation.compare.rows,
      countsRows: explanation.counts_rows,
      change: explanation.change,
      percentChange: explanation.percent_change,
      sliceColumn: explanation.slice_column,
      sliceColumnLabel: explanation.slice_column_label,
      dimensions: explanation.dimensions.map((dimension) => ({
        column: dimension.column,
        label: dimension.label,
        kind: dimension.kind,
        groups: dimension.groups.map((group) => ({
          value: group.value,
          baseValue: group.base_value,
          compareValue: group.compare_value,
          contribution: group.contribution,
          mixEffect: group.mix_effect,
          rateEffect: group.rate_effect,
          presence: group.presence,
          shareOfChange: group.share_of_change,
          percentChange: group.percent_change,
          signFlip: group.sign_flip,
          outlier: group.outlier,
        })),
        mixEffect: dimension.mix_effect,
        rateEffect: dimension.rate_effect,
        hasOutliers: dimension.has_outliers,
      })),
      dropped: explanation.dropped_dimensions.map((dropped) => ({
        column: dropped.column,
        label: dropped.label,
        reason: dropped.reason,
        duplicateOf: dropped.duplicate_of,
        baseOnly: dropped.base_only,
        compareOnly: dropped.compare_only,
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
      summaryTitle: explanation.summary_title,
      summaryPoints: explanation.summary_points.map((point) => ({ lead: point.lead, text: point.text })),
      // explanation.summary là câu chính và các ý trên nối lại, không hiện lần nữa.
      byModel: explanation.summary_by_model,
      warnings: explanation.summary_warnings,
    },
  }
}

function buildBreakdownReason(response: AskResponse): string {
  if (response.status === 'answered') {
    // Câu "tại sao" không dựng được phân rã thì backend gửi cảnh báo kèm lý do; không có cảnh báo là câu thường.
    const unavailable = response.warnings.find((warning) => warning.code === 'explanation_unavailable')
    return unavailable?.message ?? 'Câu hỏi này không có phần phân rã.'
  }
  if (response.status === 'needs_choice') return 'Chưa có phân rã: cần chọn cách hiểu câu hỏi trước.'
  return 'Không có phân rã: câu SQL không chạy ra kết quả.'
}
