// SPDX-License-Identifier: Apache-2.0
/* components/overview/DataOverview.tsx - nội dung khung "Tổng quan dữ liệu" (#198): dataset có gì trước khi hỏi. Thứ
tự đọc: tóm tắt (số dòng, khoảng thời gian), mô tả bảng ghi trong database nguồn nếu có, phần chưa có (hồ sơ đang lập, lọc theo vai), biểu đồ dựng sẵn
(OverviewCharts.tsx, độ đo chính theo tháng đứng đầu), các cột theo nhóm (ColumnGroups.tsx), chỉ số đã định nghĩa.
Trang hỏi và trang Tổng quan dữ liệu cùng dùng; mỗi trang tự quyết bấm một mục thì làm gì (onPick): trang hỏi điền câu
vào ô hỏi, trang Tổng quan dữ liệu mở trang hỏi với câu đó. Không mục nào gửi câu hỏi đi. */

import type { ReactNode } from 'react'
import { DescriptionText } from '../DescriptionText'
import { ColumnGroups } from './ColumnGroups'
import { OverviewCharts } from './OverviewCharts'
import { buildMetricQuestion } from './questions'
import type { OverviewView } from './types'
import { buildUnitText } from './units'

type Props = {
  overview: OverviewView
  onPick: (question: string) => void
}

const numberFormat = new Intl.NumberFormat('vi-VN')
// Ngày dạng 01/01/2015 như bảng kết quả; dateStyle "medium" của vi-VN ra "1 thg 1, 2015", khó đọc.
const dateFormat = new Intl.DateTimeFormat('vi-VN', { day: '2-digit', month: '2-digit', year: 'numeric' })

export function DataOverview({ overview, onPick }: Props) {
  return (
    <div className="flex flex-col gap-4">
      <p className="text-sm">{buildSummary(overview)}</p>
      {overview.description && <DescriptionText text={overview.description} source="database" />}
      {overview.missing.length > 0 && (
        <ul role="status" className="flex flex-col gap-1 text-xs text-muted-foreground">
          {overview.missing.map((message) => (
            <li key={message}>{message}</li>
          ))}
        </ul>
      )}
      {overview.charts.length > 0 && (
        <Section title="Biểu đồ dựng sẵn">
          <OverviewCharts overview={overview} onPick={onPick} />
        </Section>
      )}
      {overview.columns.length > 0 && (
        <Section title="Các cột">
          <ColumnGroups overview={overview} onPick={onPick} />
        </Section>
      )}
      {overview.metrics.length > 0 && (
        <Section title="Chỉ số đã định nghĩa">
          <ul className="flex flex-col gap-2">
            {overview.metrics.map((metric) => (
              <li key={metric.name} className="flex flex-col gap-0.5">
                <button
                  type="button"
                  onClick={() => onPick(buildMetricQuestion(metric, overview))}
                  className="self-start text-left text-sm font-medium text-label underline-offset-2 hover:underline"
                >
                  {metric.name}
                </button>
                <p className="text-xs text-muted-foreground">
                  {[metric.description, metric.unit && buildUnitText(metric.unit), !metric.confirmed && 'chưa xác nhận']
                    .filter(Boolean)
                    .join('; ')}
                </p>
              </li>
            ))}
          </ul>
        </Section>
      )}
    </div>
  )
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-2">
      <h3 className="text-xs font-semibold tracking-wider text-label uppercase">{title}</h3>
      {children}
    </section>
  )
}

/** "1.500 dòng, dữ liệu từ 01/01/2015 tới 31/01/2018 (cột order date), phiên bản 1." */
function buildSummary(overview: OverviewView): string {
  const parts: string[] = []
  if (overview.rowCount !== null) {
    const seen = overview.restricted ? ' bạn được xem' : ''
    parts.push(`${numberFormat.format(overview.rowCount)} dòng${seen}`)
  }
  if (overview.timeStart && overview.timeEnd) {
    parts.push(
      `dữ liệu từ ${formatDate(overview.timeStart)} tới ${formatDate(overview.timeEnd)} (cột ${overview.timeColumn})`,
    )
  }
  if (overview.version !== null) parts.push(`phiên bản ${overview.version}`)
  if (parts.length === 0) return overview.name
  const text = parts.join(', ')
  return `${overview.name}: ${text}.`
}

function formatDate(day: string): string {
  // Ngày dạng YYYY-MM-DD: đọc theo giờ địa phương, không theo UTC, để không lùi một ngày ở múi giờ âm.
  const [year, month, date] = day.split('-').map(Number)
  return dateFormat.format(new Date(year, month - 1, date))
}
