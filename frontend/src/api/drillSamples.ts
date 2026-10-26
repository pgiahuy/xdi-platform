// SPDX-License-Identifier: Apache-2.0
/* api/drillSamples.ts - chỉ dùng ở chế độ dev: vài dòng nguồn mẫu cho một dòng phân rã của mẫu why hoặc một ô số đo
của bảng kết quả trong examples/ask.json, để thấy bố cục tab Dòng nguồn khi không có backend hay dataset để hỏi. Chỉ
được gọi sau điều kiện import.meta.env.DEV, nên bản build production không chứa file này.
Dòng là số minh họa: chỉ giá trị nhóm, lát cắt hay khóa của ô khớp với chỗ được bấm, tổng các dòng không bằng số đã
bấm. */

import type { DrillKey, DrillRequest, DrillResponse } from './drill'
import examples from './examples/ask.json'

const delayMs = 300

// Ngày đặt hàng mẫu trong từng lát cắt của mẫu why (quý 2 và quý 3 năm 2017), theo định dạng chữ của cột DataCo.
const sampleDates = {
  base: ['4/3/2017 10:15', '5/18/2017 14:02', '6/27/2017 08:41'],
  compare: ['7/5/2017 09:40', '8/14/2017 16:25', '9/29/2017 11:03'],
}
const sampleTotals = [327.75, 199.99, 1250.4]

export async function drillFromSamples(request: DrillRequest): Promise<DrillResponse> {
  await new Promise((resolve) => setTimeout(resolve, delayMs))
  if (request.cell) return buildCellSample(request, request.cell)
  if (!request.breakdown) throw new Error('Request truy dòng nguồn thiếu cả cell lẫn breakdown.')
  const { slice, column, value } = request.breakdown
  const explanation = examples.why.explanation
  const filter = slice === 'base' ? explanation.base.filter : explanation.compare.filter
  const literal = value === null ? 'IS NULL' : `= '${String(value).replaceAll("'", "''")}'`
  const page = `LIMIT ${request.limit} OFFSET ${request.offset}`
  const sql = `SELECT * FROM dataco WHERE ${filter} AND "${column}" ${literal} ${page}`
  return {
    sql,
    result: {
      columns: [
        { name: 'Order Id', kind: 'number', role: 'other' },
        { name: 'order date (DateOrders)', kind: 'text', role: 'other' },
        { name: column, kind: 'text', role: 'other' },
        { name: 'Order Item Total', kind: 'number', role: 'other' },
        { name: '_source_file', kind: 'text', role: 'other' },
      ],
      rows: sampleDates[slice].map((date, index) => [
        41000 + index,
        date,
        value === null ? null : String(value),
        sampleTotals[index],
        'DataCoSupplyChainDataset.csv',
      ]),
      total_rows: sampleDates[slice].length,
      truncated: false,
      limit_reached: false,
    },
    total_rows: sampleDates[slice].length,
  }
}

// Ô của mẫu simple (doanh thu theo thị trường) hay mẫu why (theo quý): mỗi khóa thành một cột mang đúng giá trị đã
// bấm, cạnh vài cột của dataset DataCo.
function buildCellSample(request: DrillRequest, cell: DrillKey[]): DrillResponse {
  const where = cell.map(({ column, value }) =>
    value === null ? `"${column}" IS NULL` : `"${column}" = '${String(value).replaceAll("'", "''")}'`,
  )
  const filter = where.length > 0 ? ` WHERE ${where.join(' AND ')}` : ''
  const sql = `SELECT * FROM dataco${filter} LIMIT ${request.limit} OFFSET ${request.offset}`
  return {
    sql,
    result: {
      columns: [
        { name: 'Order Id', kind: 'number', role: 'other' },
        ...cell.map(({ column }) => ({ name: column, kind: 'text' as const, role: 'other' as const })),
        { name: 'Order Item Total', kind: 'number', role: 'other' },
        { name: '_source_file', kind: 'text', role: 'other' },
      ],
      rows: sampleTotals.map((total, index) => [
        42000 + index,
        ...cell.map(({ value }) => (value === null ? null : String(value))),
        total,
        'DataCoSupplyChainDataset.csv',
      ]),
      total_rows: sampleTotals.length,
      truncated: false,
      limit_reached: false,
    },
    total_rows: sampleTotals.length,
  }
}
