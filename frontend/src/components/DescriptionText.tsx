// SPDX-License-Identifier: Apache-2.0
/* components/DescriptionText.tsx - một dòng mô tả kèm nguồn của nó (#245): "từ database" khi tổ chức đã ghi mô tả trong
database nguồn (COMMENT ON), "do mô hình viết" khi lớp ngữ nghĩa viết lúc lập hồ sơ. Khung tổng quan dữ liệu
(overview/) và màn Chỉ số (metrics/AggregationList.tsx) dùng chung, để người đọc biết mô tả nào là của tổ chức, mô tả
nào là mô hình đoán. */

type Props = {
  text: string
  /** database, model; giá trị khác hay null thì không ghi nguồn. */
  source: string | null
}

const sourceLabels: Record<string, string> = {
  database: 'từ database',
  model: 'do mô hình viết',
}

export function DescriptionText({ text, source }: Props) {
  const label = source ? sourceLabels[source] : undefined
  return (
    <p className="text-xs text-muted-foreground">
      {text}
      {label && <span className="italic"> ({label})</span>}
    </p>
  )
}
