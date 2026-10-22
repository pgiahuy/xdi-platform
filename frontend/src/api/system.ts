// SPDX-License-Identifier: Apache-2.0
/* api/system.ts - hook cho các endpoint hệ thống. Kiểu dữ liệu lấy từ schema.ts, sinh từ openapi.json. */

import { useQuery } from '@tanstack/react-query'
import { apiGet } from './client'
import type { components } from './schema'

export type VersionInfo = components['schemas']['VersionResponse']

export function useVersion() {
  // Phiên bản chỉ đổi khi cập nhật máy chủ; đọc lại vài phút một lần là đủ.
  return useQuery({ queryKey: ['version'], queryFn: () => apiGet<VersionInfo>('/version'), staleTime: 5 * 60_000 })
}
