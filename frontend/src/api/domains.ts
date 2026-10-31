// SPDX-License-Identifier: Apache-2.0
/* api/domains.ts - hook cho gói khái niệm ngành của workspace (backend/src/api/domains.rs). Mọi thành viên đọc được;
thêm, sửa, tắt khái niệm chỉ admin (backend kiểm). Lỗi của một trường có ApiError.field. Ghi xong thì tải lại danh
sách, vì thứ tự và thời điểm sửa do backend đặt. */

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { apiGet, apiPost, apiPut } from './client'
import type { components } from './schema'

type Schemas = components['schemas']
export type Concept = Schemas['Concept']
export type ConceptPack = Schemas['ConceptPack']
export type ConceptRequest = Schemas['ConceptRequest']
export type ConceptsResponse = Schemas['ConceptsResponse']

function domainsPath(ws: string): string {
  return `/workspaces/${encodeURIComponent(ws)}/domains`
}

function conceptsKey(ws: string) {
  return ['workspaces', ws, 'domains']
}

export function useConcepts(ws: string) {
  return useQuery({
    queryKey: conceptsKey(ws),
    queryFn: () => apiGet<ConceptsResponse>(domainsPath(ws)),
  })
}

/** Thêm khái niệm vào gói (id không có) hoặc ghi đè khái niệm có id này; tắt là gửi enabled false. */
export function useSaveConcept(ws: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ pack, id, request }: { pack: string; id?: string; request: ConceptRequest }) => {
      const base = `${domainsPath(ws)}/${encodeURIComponent(pack)}/concepts`
      return id ? apiPut<Concept>(`${base}/${encodeURIComponent(id)}`, request) : apiPost<Concept>(base, request)
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: conceptsKey(ws) }),
  })
}
