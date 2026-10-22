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

export function apiPatch<T>(path: string, body?: unknown): Promise<T> {
  return request<T>('PATCH', path, body)
}

export function apiDelete<T>(path: string): Promise<T> {
  return request<T>('DELETE', path)
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
    throw networkError()
  }
  if (!res.ok) {
    throw errorFromBody(res.status, await res.json().catch(() => null))
  }
  // Một số endpoint trả thành công mà không có body: 204 (đăng xuất) và 201 (thêm thành viên). Gọi res.json() với body
  // rỗng sẽ ném lỗi, nên đọc chữ trước. Hook gọi endpoint kiểu này khai báo T là void.
  const text = await res.text()
  return (text ? JSON.parse(text) : undefined) as T
}

export type UploadOptions = {
  /** Nhận tỉ lệ đã gửi, từ 0 tới 1. */
  onProgress?: (sent: number) => void
  /** Hủy thì ngừng gửi, đóng kết nối, và lời hứa trả về bị từ chối với ApiError có code là 'aborted'. */
  signal?: AbortSignal
}

/** Gửi form multipart (nạp file).
Dùng XMLHttpRequest thay cho fetch: fetch không báo đã gửi được bao nhiêu byte, mà file vài chục MB gửi mất một lúc;
không có tiến trình thì người dùng không biết trang còn chạy hay đã treo. */
export function apiUpload<T>(path: string, form: FormData, { onProgress, signal }: UploadOptions = {}): Promise<T> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) {
      reject(abortedError())
      return
    }
    const xhr = new XMLHttpRequest()
    const abort = () => xhr.abort()
    signal?.addEventListener('abort', abort, { once: true })
    const finish = () => signal?.removeEventListener('abort', abort)

    xhr.open('POST', `/api${path}`)
    xhr.setRequestHeader('Accept', 'application/json')
    xhr.upload.onprogress = (event) => {
      if (event.lengthComputable) onProgress?.(event.loaded / event.total)
    }
    xhr.onabort = () => {
      finish()
      reject(abortedError())
    }
    xhr.onerror = () => {
      finish()
      reject(networkError())
    }
    xhr.onload = () => {
      finish()
      const body = parseJson(xhr.responseText)
      if (xhr.status >= 200 && xhr.status < 300) resolve(body as T)
      else reject(errorFromBody(xhr.status, body))
    }
    // Không đặt Content-Type: trình duyệt tự ghi multipart kèm ranh giới giữa các trường.
    xhr.send(form)
  })
}

export function isAborted(error: unknown): boolean {
  return error instanceof ApiError && error.code === 'aborted'
}

function abortedError(): ApiError {
  return new ApiError(0, 'aborted', 'Đã hủy gửi file.')
}

function networkError(): ApiError {
  return new ApiError(0, 'network', 'Không kết nối được tới máy chủ. Kiểm tra backend đã chạy chưa.')
}

function parseJson(text: string): unknown {
  try {
    return JSON.parse(text)
  } catch {
    return null
  }
}

function errorFromBody(status: number, body: unknown): ApiError {
  // Body đọc ra có kiểu unknown; ghi rõ dạng lỗi của backend để TypeScript kiểm phần dùng bên dưới.
  const err = body as { code?: string; message?: string } | null
  if (err?.message) {
    return new ApiError(status, err.code ?? 'unknown', err.message)
  }
  // 502, 503, 504 không kèm thông báo từ backend: lỗi đến từ proxy đứng trước (Vite khi dev, Caddy khi chạy thật),
  // tức là backend không trả lời được.
  if (status >= 502 && status <= 504) {
    return new ApiError(status, 'backend_unavailable', 'Máy chủ không phản hồi. Kiểm tra backend đã chạy chưa.')
  }
  // Backend trả 413 kèm thông báo (MAX_UPLOAD_MB). 413 không kèm thông báo đến từ proxy đứng trước, ví dụ nginx mặc
  // định chỉ nhận 1 MB và trả trang HTML.
  if (status === 413) {
    return new ApiError(
      status,
      'too_large',
      'File vượt giới hạn dung lượng máy chủ nhận. Nếu có proxy đứng trước backend, kiểm cả giới hạn của proxy.',
    )
  }
  return new ApiError(status, 'unknown', `Lỗi ${status}`)
}
