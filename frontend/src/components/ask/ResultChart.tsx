// SPDX-License-Identifier: Apache-2.0
/* components/ask/ResultChart.tsx - biểu đồ theo gợi ý của backend (chart trong AskResponse), bộ luật ở ARCHITECTURE
mục 6b theo IBCS: thanh ngang so sánh nhóm, cột đứng hay đường theo thời gian (thời gian luôn trên trục ngang), cột chồng
cho cơ cấu theo thời gian, nhiều số đo cùng đơn vị cạnh nhau, khác đơn vị thì mỗi số đo một biểu đồ nhỏ chung trục. Dữ
liệu vẽ dựng ở chartData.ts từ chính các dòng của bảng. Không có gợi ý thì trang chỉ hiện bảng.
Vẽ bằng Recharts (MIT, các gói kéo theo MIT, ISC, BSD-3-Clause). Gói nặng: lúc viết, chunk của file này khoảng 110
KB sau gzip, còn JS lần tải đầu của cả giao diện khoảng 166 KB, nên trang tải file này riêng (React.lazy) khi có biểu
đồ để vẽ: mở trang hỏi không phải chờ nó. Đo bằng gzip -c | wc -c trên các file JS mà dist/index.html nạp (script
và modulepreload) và trên chunk ResultChart, sau npm run build.
Trang bọc file này trong ErrorBoundary: chunk tải lỗi hay Recharts ném lỗi thì chỉ mất biểu đồ, bảng vẫn còn.
Màu là thang xám của theme, như IBCS dùng màu trung tính và chỉ làm nổi số cần nhấn: chuỗi đầu đậm nhất, phần "Khác"
nhạt nhất; nhiều đường còn phân biệt bằng nét đứt và chú giải, và bảng bên dưới có đủ số. */

import {
  Bar,
  BarChart,
  CartesianGrid,
  Legend,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts'
import { buildChartData, type ChartData } from './chartData'
import type { ChartView, ResultView } from './types'

type Props = {
  result: ResultView
  chart: ChartView
}

// Thứ tự cố định, chuỗi thứ i luôn cùng màu và cùng nét. --chart-1 gần màu nền sáng nên chỉ dành cho phần "Khác".
const seriesColors = ['var(--chart-5)', 'var(--chart-3)', 'var(--chart-2)', 'var(--chart-4)', 'var(--muted-foreground)']
const otherColor = 'var(--chart-1)'
const seriesDashes = [undefined, '6 3', '2 3', '8 3 2 3', '4 4']
const axisTick = { fill: 'var(--muted-foreground)', fontSize: 12 }
const compactFormat = new Intl.NumberFormat('vi-VN', { notation: 'compact', maximumFractionDigits: 1 })
const tooltipStyle = {
  background: 'var(--popover)',
  color: 'var(--popover-foreground)',
  borderColor: 'var(--border)',
  borderRadius: 8,
  fontSize: 12,
}

// IBCS vẽ số gốc rộng 2/3 khoảng của một mốc và tỷ lệ rộng 1/3, để người đọc không lấy chiều cao cột tỷ lệ cộng dồn.
const baseGap = '33%'
const ratioGap = '66%'

const kindLabels: Record<ChartView['kind'], string> = {
  bar: 'Biểu đồ thanh ngang',
  column: 'Biểu đồ cột',
  line: 'Biểu đồ đường',
  stacked_column: 'Biểu đồ cột chồng',
}

export function ResultChart({ result, chart }: Props) {
  // Khác đơn vị thì mỗi số đo một biểu đồ nhỏ, không dùng hai trục: hai thang đo chọn tùy ý làm số trông như bất kỳ.
  const groups = chart.layout === 'panels' ? chart.y.map((name) => [name]) : [chart.y]
  const panels = groups.map((measures) => ({ measures, data: buildChartData(result, chart, measures) }))
  const notes = buildNotes(panels[0].data)

  return (
    <figure className="flex flex-col gap-3">
      <figcaption className="text-xs text-muted-foreground">
        {buildCaption(chart)}
        {notes && ` (${notes})`}
      </figcaption>
      {panels.map(({ measures, data }) => (
        <ChartPanel
          key={measures.join('|')}
          title={panels.length > 1 ? measures[0] : null}
          chart={chart}
          data={data}
          small={panels.length > 1}
        />
      ))}
    </figure>
  )
}

type PanelProps = {
  /** Tên số đo khi mỗi số đo một biểu đồ nhỏ; null khi chỉ có một biểu đồ. */
  title: string | null
  chart: ChartView
  data: ChartData
  small: boolean
}

function ChartPanel({ title, chart, data, small }: PanelProps) {
  const showLegend = data.series.length > 1
  const allRatio = data.series.every((s) => s.ratio)
  const tooltip = (
    <Tooltip
      formatter={(_value, name, item) => [item.payload[`${String(item.dataKey)}_text`], name]}
      contentStyle={tooltipStyle}
      cursor={{ fill: 'var(--muted)', stroke: 'var(--border)' }}
    />
  )
  const numberAxisProps = {
    tick: axisTick,
    tickLine: false,
    tickFormatter: (value: number) => compactFormat.format(value),
  }

  if (chart.kind === 'bar') {
    // Mỗi thanh một hàng: chiều cao theo số nhóm để nhãn nhóm không đè nhau.
    const height = Math.max(small ? 120 : 160, data.points.length * 32 + 48)
    return (
      <div className="flex flex-col gap-1">
        {title && <p className="text-xs font-medium">{title}</p>}
        <div style={{ height }} className="w-full">
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={data.points} layout="vertical" barCategoryGap={allRatio ? ratioGap : baseGap}>
              <CartesianGrid horizontal={false} stroke="var(--border)" />
              <XAxis type="number" axisLine={false} {...numberAxisProps} />
              <YAxis
                type="category"
                dataKey="x"
                width={120}
                tick={axisTick}
                tickLine={false}
                axisLine={{ stroke: 'var(--border)' }}
              />
              {tooltip}
              {showLegend && <Legend wrapperStyle={{ fontSize: 12 }} />}
              {/* Tắt hiệu ứng chạy: số hiện ngay, không gây chóng mặt cho người nhạy chuyển động. */}
              {data.series.map((s, i) => (
                <Bar
                  key={s.key}
                  dataKey={s.key}
                  name={s.name}
                  fill={colorOf(s.key, i)}
                  radius={[0, 4, 4, 0]}
                  isAnimationActive={false}
                />
              ))}
            </BarChart>
          </ResponsiveContainer>
        </div>
      </div>
    )
  }

  const isLine = chart.kind === 'line'
  const Chart = isLine ? LineChart : BarChart
  const stacked = chart.kind === 'stacked_column'
  return (
    <div className="flex flex-col gap-1">
      {title && <p className="text-xs font-medium">{title}</p>}
      <div className={small ? 'h-48 w-full' : 'h-72 w-full'}>
        <ResponsiveContainer width="100%" height="100%">
          <Chart
            data={data.points}
            margin={{ top: 8, right: 8, bottom: 0, left: 0 }}
            barCategoryGap={allRatio ? ratioGap : baseGap}
          >
            <CartesianGrid vertical={false} stroke="var(--border)" />
            <XAxis
              dataKey="x"
              tick={axisTick}
              tickLine={false}
              axisLine={{ stroke: 'var(--border)' }}
              // Đường cần lề hai đầu để điểm và nhãn đầu, cuối không dính mép; cột đã có khoảng của riêng nó.
              padding={isLine ? { left: 24, right: 24 } : undefined}
              interval="preserveStartEnd"
            />
            <YAxis axisLine={false} width={56} {...numberAxisProps} />
            {tooltip}
            {showLegend && <Legend wrapperStyle={{ fontSize: 12 }} />}
            {data.series.map((s, i) =>
              isLine ? (
                // Điểm đánh dấu ở từng kỳ (IBCS): số liệu chỉ có tại các điểm, đoạn nối không phải số liệu. Kỳ trống
                // để đứt đường, không nối qua.
                <Line
                  key={s.key}
                  type="linear"
                  dataKey={s.key}
                  name={s.name}
                  stroke={colorOf(s.key, i)}
                  strokeWidth={2}
                  strokeDasharray={seriesDashes[i % seriesDashes.length]}
                  dot={{ r: 3, fill: colorOf(s.key, i) }}
                  connectNulls={false}
                  isAnimationActive={false}
                />
              ) : (
                // Các phần của cột chồng ngăn nhau bằng viền màu nền, để thang xám gần nhau vẫn tách được.
                <Bar
                  key={s.key}
                  dataKey={s.key}
                  name={s.name}
                  fill={colorOf(s.key, i)}
                  stackId={stacked ? 'parts' : undefined}
                  stroke={stacked ? 'var(--background)' : undefined}
                  radius={stacked ? undefined : [4, 4, 0, 0]}
                  isAnimationActive={false}
                />
              ),
            )}
          </Chart>
        </ResponsiveContainer>
      </div>
    </div>
  )
}

function colorOf(key: string, index: number): string {
  return key === 'other' ? otherColor : seriesColors[index % seriesColors.length]
}

function buildCaption(chart: ChartView): string {
  const measures = chart.y.join(', ')
  const split = chart.series ? `, chia theo ${chart.series}` : ''
  const layout = chart.layout === 'panels' ? '; mỗi số đo một biểu đồ vì khác đơn vị' : ''
  return `${kindLabels[chart.kind]}: ${measures} theo ${chart.x}${split}${layout}`
}

function buildNotes(data: ChartData): string {
  if (data.merged > 0) return `${data.merged} nhóm nhỏ hơn gộp vào "Khác"`
  if (data.leftOut > 0) return `không vẽ ${data.leftOut} nhóm nhỏ hơn, xem ở bảng`
  return ''
}
