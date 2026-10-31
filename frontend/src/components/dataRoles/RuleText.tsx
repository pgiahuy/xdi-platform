// SPDX-License-Identifier: Apache-2.0
/* components/dataRoles/RuleText.tsx - một luật bằng lời: "sales_demo / orders: market là Europe, LATAM". Dùng ở thẻ vai
và ở khung sửa luật. */

import type { RuleDraft } from './types'

export function RuleText({ rule, table }: { rule: RuleDraft; table: string }) {
  return (
    <>
      <span className="text-muted-foreground">{table}: </span>
      <span className="font-medium">{rule.column}</span> là{' '}
      {rule.values.length > 0 ? rule.values.join(', ') : <span className="italic">không giá trị nào (không thấy dòng nào)</span>}
    </>
  )
}
