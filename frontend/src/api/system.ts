// SPDX-License-Identifier: Apache-2.0
/* api/system.ts - hook cho các endpoint hệ thống. Kiểu dữ liệu sẽ sinh từ openapi.json khi có (#53). */

import { useQuery } from '@tanstack/react-query'
import { apiGet } from './client'

export type VersionInfo = {
  name: string
  version: string
  llm: string
}

export function useVersion() {
  return useQuery({ queryKey: ['version'], queryFn: () => apiGet<VersionInfo>('/version') })
}
