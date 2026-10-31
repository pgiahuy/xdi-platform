// SPDX-License-Identifier: Apache-2.0
/* components/DescriptionText.tsx - một dòng mô tả kèm nguồn của nó (#245): "từ database" khi tổ chức đã ghi mô tả trong
database nguồn (COMMENT ON), "do mô hình viết" khi lớp ngữ nghĩa viết lúc lập hồ sơ. Khung tổng quan dữ liệu
(overview/) và màn Chỉ số (metrics/AggregationList.tsx) dùng chung, để người đọc biết mô tả nào là của tổ chức, mô tả
nào là mô hình đoán. */

type Props = {
  text: string
  /** database, model; giá trị khác hay null thì không ghi nguồn. */
  source: string | null
  /** span khi đặt trong nút (nút chỉ chứa được phần tử nội dòng), mặc định p. */
  as?: 'p' | 'span'
}

const sourceLabels: Record<string, string> = {
  database: 'từ database',
  model: 'do mô hình viết',
}

export function DescriptionText({ text, source, as: Tag = 'p' }: Props) {
  const label = source ? sourceLabels[source] : undefined
  return (
    <Tag className="block text-xs text-muted-foreground">
      {text}
      {label && <span className="italic"> ({label})</span>}
    </Tag>
  )
}
