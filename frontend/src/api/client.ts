// SPDX-License-Identifier: Apache-2.0
/* api/client.ts - một chỗ duy nhất gọi backend. Mọi hook trong src/api dùng hàm này.
Đường dẫn luôn bắt đầu bằng /api, cùng địa chỉ với trang, nên không cần cấu hình địa chỉ backend hay CORS.
Lỗi từ backend có dạng {code, message}; message là tiếng Việt, hiện được thẳng cho người dùng. */

export class ApiError extends Error {
  readonly status: number
  readonly code: string

  constructor(status: number, code: string, message: string) {
    super(message)
    this.status = status
    this.code = code
  }
}

export async function apiGet<T>(path: string): Promise<T> {
  let res: Response
  try {
    res = await fetch(`/api${path}`, { headers: { Accept: 'application/json' } })
  } catch {
    throw new ApiError(0, 'network', 'Không kết nối được tới máy chủ. Kiểm tra backend đã chạy chưa.')
  }
  if (!res.ok) {
    const body = await res.json().catch(() => null)
    if (body?.message) {
      throw new ApiError(res.status, body.code ?? 'unknown', body.message)
    }
    // 502, 503, 504 không kèm thông báo từ backend: lỗi đến từ proxy đứng trước (Vite khi dev, Caddy khi chạy thật),
    // tức là backend không trả lời được.
    if (res.status >= 502 && res.status <= 504) {
      throw new ApiError(res.status, 'backend_unavailable', 'Máy chủ không phản hồi. Kiểm tra backend đã chạy chưa.')
    }
    throw new ApiError(res.status, 'unknown', `Lỗi ${res.status}`)
  }
  return (await res.json()) as T
}
