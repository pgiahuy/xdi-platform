// SPDX-License-Identifier: Apache-2.0
/* components/metrics/metricFormula.ts - đổi qua lại giữa "phép tính + cột" của form chỉ số và biểu thức SQL gửi
backend. Người không biết SQL chọn phép tính và cột; công thức dạng khác (thương, biểu thức lồng) thì form mở ô SQL.
Backend kiểm lại mọi công thức (backend/src/service/metrics/check.rs), ở đây chỉ dựng cho đúng cú pháp. */

import type { ColumnItem } from './types'

export type FormulaOp = 'sum' | 'avg' | 'count' | 'count_distinct'

export const formulaOps: { op: FormulaOp; label: string }[] = [
  { op: 'sum', label: 'Tổng' },
  { op: 'avg', label: 'Trung bình' },
  { op: 'count', label: 'Đếm dòng' },
  { op: 'count_distinct', label: 'Đếm giá trị khác nhau' },
]

/** Phép tính có cần chọn cột không: đếm dòng là COUNT(*). */
export function needsColumn(op: FormulaOp): boolean {
  return op !== 'count'
}

/** Tên cột trong ngoặc kép như SQL viết, ngoặc kép bên trong nhân đôi. */
export function quoteColumn(name: string): string {
  return `"${name.replaceAll('"', '""')}"`
}

export function buildExpression(op: FormulaOp, column: string): string {
  switch (op) {
    case 'sum':
      return `SUM(${quoteColumn(column)})`
    case 'avg':
      return `AVG(${quoteColumn(column)})`
    case 'count':
      return 'COUNT(*)'
    case 'count_distinct':
      return `COUNT(DISTINCT ${quoteColumn(column)})`
  }
}

const patterns: { op: FormulaOp; pattern: RegExp }[] = [
  { op: 'count', pattern: /^count\(\s*\*\s*\)$/i },
  { op: 'count_distinct', pattern: /^count\(\s*distinct\s+"((?:[^"]|"")+)"\s*\)$/i },
  { op: 'sum', pattern: /^sum\(\s*"((?:[^"]|"")+)"\s*\)$/i },
  { op: 'avg', pattern: /^avg\(\s*"((?:[^"]|"")+)"\s*\)$/i },
]

/** Phép tính và cột của một công thức viết theo đúng các dạng ở trên; undefined với công thức khác. */
export function readExpression(expression: string): { op: FormulaOp; column: string } | undefined {
  const text = expression.trim()
  for (const { op, pattern } of patterns) {
    const match = pattern.exec(text)
    if (match) return { op, column: (match[1] ?? '').replaceAll('""', '"') }
  }
  return undefined
}

const numericType = /^(TINYINT|SMALLINT|INTEGER|BIGINT|HUGEINT|UTINYINT|USMALLINT|UINTEGER|UBIGINT|FLOAT|DOUBLE|DECIMAL)/

/** Cột chọn được cho một phép tính: tổng và trung bình cần cột số đo kiểu số; đếm khác nhau nhận mọi cột trừ cột bị
 * bỏ qua. */
export function columnsFor(op: FormulaOp, columns: ColumnItem[]): ColumnItem[] {
  if (op === 'count') return []
  if (op === 'count_distinct') return columns.filter((c) => c.role !== 'ignore')
  return columns.filter((c) => c.role === 'measure' && numericType.test(c.dataType))
}
