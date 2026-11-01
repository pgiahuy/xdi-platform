// SPDX-License-Identifier: Apache-2.0
/* pages/askScope.ts - đọc dataset và bảng ghim của trang hỏi, trang Tổng quan từ đường dẫn (?dataset=...&table=...) và
dựng dữ liệu cho ô chọn (DatasetScopePicker). Một dataset là một database hay một file (#257).
Đường dẫn cũ ghi id bảng vào ?dataset= (trước #257 mỗi bảng là một dataset; link Tổng quan, cuộc trò chuyện đã lưu):
id đó là một bảng thì mở dataset chứa nó và ghim đúng bảng đó, nên link cũ vẫn mở đúng chỗ. */

import { isTableProfiling, type CatalogDataset, type CatalogTable } from '@/api/catalog'
import type { ProfileProgress, ScopeDatasetOption } from '@/components/ask/types'

export type AskScope = {
  dataset: CatalogDataset | undefined
  /** Bảng ghim; undefined khi để backend chọn (với file cũng vậy: file chỉ có một bảng). */
  table: CatalogTable | undefined
}

/** datasetParam không khớp gì (chưa ghi, đã xóa) thì lấy database đầu tiên, không có database thì dataset đầu tiên. */
export function resolveAskScope(catalog: CatalogDataset[], datasetParam: string | null, tableParam: string | null): AskScope {
  const direct = catalog.find((d) => d.id === datasetParam)
  if (direct) return { dataset: direct, table: direct.tables.find((t) => t.id === tableParam) }
  const parent = catalog.find((d) => d.tables.some((t) => t.id === datasetParam))
  if (parent) return { dataset: parent, table: parent.tables.find((t) => t.id === datasetParam) }
  return { dataset: catalog.find((d) => d.kind === 'database') ?? catalog[0], table: undefined }
}

/** Bảng mở sẵn ở trang Tổng quan khi đường dẫn chưa ghi bảng: bảng giữ nhiều khóa ngoại nhất (thường là bảng sự kiện,
 * như dòng đơn hàng), bằng nhau thì bảng nhiều dòng hơn; giống cách backend chọn bảng bắt đầu khi câu hỏi không gợi ý
 * bảng nào (service/ask/scope.rs). */
export function pickMainTable(dataset: CatalogDataset): CatalogTable | undefined {
  const keys = (t: CatalogTable) => dataset.relations.filter((r) => r.from_dataset_id === t.id).length
  return [...dataset.tables].sort((a, b) => keys(b) - keys(a) || (b.row_count ?? 0) - (a.row_count ?? 0))[0]
}

export function buildScopeOptions(catalog: CatalogDataset[]): ScopeDatasetOption[] {
  return catalog.map((d) => ({
    id: d.id,
    name: d.name,
    kind: d.kind,
    tables: d.tables.map((t) => ({ id: t.id, name: t.name, schema: t.schema })),
  }))
}

/** Hồ sơ của bảng ghim, không ghim thì của cả dataset: còn bảng đang lập thì báo đang lập; chỉ báo lỗi hay không có câu
 * gợi ý khi mọi bảng như vậy, vì một bảng lỗi thì câu hỏi vẫn bắt đầu được từ bảng khác. */
export function buildScopeProgress({ dataset, table }: AskScope): ProfileProgress {
  const tables = table ? [table] : (dataset?.tables ?? [])
  if (tables.length === 0) return 'ready'
  if (tables.some(isTableProfiling)) return 'profiling'
  if (tables.every((t) => t.current_profile_status === 'failed')) return 'failed'
  if (tables.every((t) => t.semantic_unavailable)) return 'no_suggestions'
  return 'ready'
}

const MAX_SUGGESTIONS = 6

/** Câu gợi ý của bảng ghim. Không ghim: câu chọn tay của dataset nếu có (database mẫu, DatasetSummary.suggested_questions),
 * không thì lấy lần lượt từng bảng một câu, để vài bảng đầu không chiếm hết chỗ. */
export function buildScopeSuggestions({ dataset, table }: AskScope): string[] {
  if (!table && dataset && dataset.suggested_questions.length > 0) return dataset.suggested_questions
  const lists = (table ? [table] : (dataset?.tables ?? [])).map((t) => t.suggested_questions)
  const picked: string[] = []
  for (let round = 0; picked.length < MAX_SUGGESTIONS && lists.some((l) => l.length > round); round++) {
    for (const list of lists) {
      const question = list[round]
      if (question !== undefined && !picked.includes(question) && picked.length < MAX_SUGGESTIONS) picked.push(question)
    }
  }
  return picked
}
