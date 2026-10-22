// SPDX-License-Identifier: Apache-2.0
/* components/ask/ResultChart.tsx - biểu đồ theo gợi ý của backend (chart trong AskResponse): cột khi trục x là một
chiều, đường khi trục x là thời gian (ARCHITECTURE mục 6). Không có gợi ý thì trang chỉ hiện bảng.
Vẽ bằng Recharts (MIT, các gói kéo theo MIT, ISC, BSD-3-Clause). Gói nặng: lúc viết, chunk của file này khoảng 110
KB sau gzip, còn JS lần tải đầu của cả giao diện khoảng 166 KB, nên trang tải file này riêng (React.lazy) khi có biểu
đồ để vẽ: mở trang hỏi không phải chờ nó. Đo bằng gzip -c | wc -c trên các file JS mà dist/index.html nạp (script
và modulepreload) và trên chunk ResultChart, sau npm run build.
Trang bọc file này trong ErrorBoundary: chunk tải lỗi hay Recharts ném lỗi thì chỉ mất biểu đồ, bảng vẫn còn.
Màu lấy từ biến --chart-* của theme; theme hiện là thang xám, nên nhiều chuỗi còn phân biệt bằng nét đứt và chú giải,
và bảng bên dưới có đủ số. Một chuỗi thì không cần chú giải, chú thích của hình đã gọi tên nó. */

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
import { formatCell, toChartNumber } from './cellFormat'
import type { ChartView, ResultView } from './types'

type Props = {
  result: ResultView
  chart: ChartView
}

// Thứ tự cố định, chuỗi thứ i luôn cùng màu và cùng nét. Bỏ --chart-1 vì gần màu nền sáng.
const seriesColors = ['var(--chart-2)', 'var(--chart-5)', 'var(--chart-3)', 'var(--chart-4)']
const seriesDashes = [undefined, '6 3', '2 3', '8 3 2 3']
const axisTick = { fill: 'var(--muted-foreground)', fontSize: 12 }
const compactFormat = new Intl.NumberFormat('vi-VN', { notation: 'compact', maximumFractionDigits: 1 })

export function ResultChart({ result, chart }: Props) {
  const xIndex = result.columns.findIndex((column) => column.name === chart.x)
  // Quá bốn chuỗi thì các màu xám không còn phân biệt được; chỉ vẽ bốn chuỗi đầu, bảng vẫn có đủ.
  const series = chart.y
    .map((name) => result.columns.findIndex((column) => column.name === name))
    .filter((index) => index >= 0)
    .slice(0, seriesColors.length)
  if (xIndex < 0 || series.length === 0) return null

  // Mỗi điểm giữ cả số để vẽ (y0) lẫn chữ đã định dạng của ô gốc (y0_text) cho tooltip: decimal đổi sang số có thể
  // làm tròn, tooltip phải hiện đúng con số trong bảng.
  const data = result.rows.map((row) => {
    const x = formatCell(row[xIndex], result.columns[xIndex].kind)
    const point: Record<string, string | number | null> = { x: axisLabel(x) }
    series.forEach((columnIndex, i) => {
      point[`y${i}`] = toChartNumber(row[columnIndex])
      point[`y${i}_text`] = formatCell(row[columnIndex], result.columns[columnIndex].kind)
    })
    return point
  })
  const names = series.map((columnIndex) => result.columns[columnIndex].name)
  const dropped = chart.y.length - names.length
  const Chart = chart.kind === 'bar' ? BarChart : LineChart

  return (
    <figure className="flex flex-col gap-2">
      <figcaption className="text-xs text-muted-foreground">
        {chart.kind === 'bar' ? 'Biểu đồ cột' : 'Biểu đồ đường'}: {names.join(', ')} theo {chart.x}
        {dropped > 0 && ` (không vẽ thêm ${dropped} cột, xem ở bảng)`}
      </figcaption>
      <div className="h-72 w-full">
        <ResponsiveContainer width="100%" height="100%">
          <Chart data={data} margin={{ top: 8, right: 8, bottom: 0, left: 0 }}>
            <CartesianGrid vertical={false} stroke="var(--border)" />
            <XAxis
              dataKey="x"
              tick={axisTick}
              tickLine={false}
              axisLine={{ stroke: 'var(--border)' }}
              // Đường cần lề hai đầu để điểm và nhãn đầu, cuối không dính mép; cột đã có khoảng của riêng nó.
              padding={chart.kind === 'line' ? { left: 24, right: 24 } : undefined}
              interval="preserveStartEnd"
            />
            <YAxis
              tick={axisTick}
              tickLine={false}
              axisLine={false}
              width={56}
              tickFormatter={(value: number) => compactFormat.format(value)}
            />
            <Tooltip
              formatter={(_value, name, item) => [item.payload[`${String(item.dataKey)}_text`], name]}
              contentStyle={{
                background: 'var(--popover)',
                color: 'var(--popover-foreground)',
                borderColor: 'var(--border)',
                borderRadius: 8,
                fontSize: 12,
              }}
              cursor={{ fill: 'var(--muted)', stroke: 'var(--border)' }}
            />
            {series.length > 1 && <Legend wrapperStyle={{ fontSize: 12 }} />}
            {/* Tắt hiệu ứng chạy: số hiện ngay, không gây chóng mặt cho người nhạy chuyển động. */}
            {names.map((name, i) =>
              chart.kind === 'bar' ? (
                <Bar
                  key={name}
                  dataKey={`y${i}`}
                  name={name}
                  fill={seriesColors[i]}
                  radius={[4, 4, 0, 0]}
                  maxBarSize={48}
                  isAnimationActive={false}
                />
              ) : (
                <Line
                  key={name}
                  type="linear"
                  dataKey={`y${i}`}
                  name={name}
                  stroke={seriesColors[i]}
                  strokeWidth={2}
                  strokeDasharray={seriesDashes[i]}
                  dot={{ r: 4, fill: seriesColors[i] }}
                  isAnimationActive={false}
                />
              ),
            )}
          </Chart>
        </ResponsiveContainer>
      </div>
    </figure>
  )
}

// Mốc thời gian gộp theo ngày, tháng, quý đều rơi vào nửa đêm; bỏ " 00:00:00" trên trục cho nhãn đủ chỗ ở màn hẹp.
// Bảng vẫn hiện đủ giờ.
function axisLabel(text: string): string {
  return text.replace(/ 00:00:00$/, '')
}
