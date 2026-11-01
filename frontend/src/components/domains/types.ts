// SPDX-License-Identifier: Apache-2.0
/* components/domains/types.ts - kiểu dữ liệu các component của màn Khái niệm ngành nhận từ trang
(pages/DomainsPage.tsx). Trang dựng các kiểu này từ dữ liệu backend (src/api/domains.ts), để component không phụ thuộc
dạng JSON của API. */

export type CalculationItem = { name: string; description: string }

/** Luật kiểm hợp lý; màn này chưa sửa luật, chỉ giữ nguyên và gửi lại khi lưu. */
export type SanityCheckItem = { kind: 'value_range' | 'non_negative'; min?: number | null; max?: number | null }

export type ConceptItem = {
  id: string
  /** Khóa trong file gói; null với khái niệm admin thêm. */
  key: string | null
  name: string
  description: string
  synonyms: string[]
  calculations: CalculationItem[]
  dimensions: string[]
  sanityChecks: SanityCheckItem[]
  enabled: boolean
  /** Lần sửa gần nhất; null khi chưa ai sửa từ lúc chép từ file gói. */
  updatedAt: string | null
}

export type PackItem = {
  key: string
  name: string
  description: string
  concepts: ConceptItem[]
}

/** Khái niệm admin đang soạn: từ đồng nghĩa và chiều phân rã là chữ ngăn bằng dấu phẩy như trong ô nhập. */
export type ConceptDraft = {
  name: string
  description: string
  synonyms: string
  dimensions: string
  calculations: CalculationItem[]
}

/** Tên trường backend trả trong lỗi (ApiError.field), trùng tên trường JSON của request. */
export type ConceptField = 'name' | 'description' | 'synonyms' | 'calculations' | 'dimensions' | 'sanity_checks'

/** Một dataset trên màn Khái niệm ngành: pack là khóa gói đang gắn, null khi không dùng gói. */
export type DatasetPackItem = {
  id: string
  name: string
  pack: string | null
}
