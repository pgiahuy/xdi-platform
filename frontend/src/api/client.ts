// SPDX-License-Identifier: Apache-2.0
/* api/client.ts - một chỗ duy nhất gọi backend. Mọi hook trong src/api dùng các hàm này.
Đường dẫn luôn bắt đầu bằng /api, cùng địa chỉ với trang, nên không cần cấu hình địa chỉ backend hay CORS.
Cookie phiên đăng nhập là httpOnly, cùng địa chỉ nên fetch tự gửi kèm; JavaScript không đọc được nó.
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

export function apiGet<T>(path: string): Promise<T> {
  return request<T>('GET', path)
}

export function apiPost<T>(path: string, body?: unknown): Promise<T> {
  return request<T>('POST', path, body)
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const headers: Record<string, string> = { Accept: 'application/json' }
  if (body !== undefined) headers['Content-Type'] = 'application/json'
  let res: Response
  try {
    res = await fetch(`/api${path}`, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
    })
  } catch {
    throw new ApiError(0, 'network', 'Không kết nối được tới máy chủ. Kiểm tra backend đã chạy chưa.')
  }
  if (!res.ok) {
    // res.json() trả kiểu any; ghi rõ dạng lỗi của backend để TypeScript kiểm phần dùng bên dưới.
    const err = (await res.json().catch(() => null)) as { code?: string; message?: string } | null
    if (err?.message) {
      throw new ApiError(res.status, err.code ?? 'unknown', err.message)
    }
    // 502, 503, 504 không kèm thông báo từ backend: lỗi đến từ proxy đứng trước (Vite khi dev, Caddy khi chạy thật),
    // tức là backend không trả lời được.
    if (res.status >= 502 && res.status <= 504) {
      throw new ApiError(res.status, 'backend_unavailable', 'Máy chủ không phản hồi. Kiểm tra backend đã chạy chưa.')
    }
    throw new ApiError(res.status, 'unknown', `Lỗi ${res.status}`)
  }
  // 204 không có body (ví dụ đăng xuất); gọi res.json() sẽ ném lỗi. Hook gọi endpoint kiểu này khai báo T là void.
  if (res.status === 204) return undefined as T
  return (await res.json()) as T
}
