// SPDX-License-Identifier: Apache-2.0
/* pages/authNav.ts - đường dẫn của màn đăng nhập, thiết lập, và cách nhớ trang đang mở để quay lại sau khi đăng nhập.
Trang đang mở đi qua tham số next trong đường dẫn, không lưu ở trình duyệt, nên mở lại link vẫn quay đúng chỗ. */

export const loginRoute = '/login'
export const setupRoute = '/setup'

/** Sau khi đăng xuất: tải lại cả trang tới màn đăng nhập, không kèm next (người đăng nhập sau có thể là người khác).
 * Không chuyển trong ứng dụng: trang đang mở vẫn gắn vào bộ đệm, tự đọc lại /auth/me, nhận 401 rồi tự chuyển tới
 * /login?next=<trang cũ>; lần chuyển đó có thể đến sau và thắng. Tải lại cả trang cũng xóa mọi dữ liệu còn trong bộ nhớ
 * của trình duyệt. */
export function signOutTo() {
  window.location.replace(loginRoute)
}

/** Đường dẫn tới màn đăng nhập, nhớ trang đang mở. Trang gốc thì không cần nhớ. */
export function loginHref(returnTo: string): string {
  const next = safeNext(returnTo)
  return next === '/' ? loginRoute : `${loginRoute}?next=${encodeURIComponent(next)}`
}

/** Chỉ nhận đường dẫn trong chính trang này. "//ví-dụ.com" hay "https://..." trong next sẽ đưa người vừa đăng nhập
 * sang trang khác (lỗi chuyển hướng mở), nên bỏ, về trang gốc. Khoảng trắng (gồm tab, xuống dòng) và dấu \ cũng bỏ:
 * trình duyệt xóa tab và xuống dòng khỏi URL, nên "/<tab>/ví-dụ.com" thành "//ví-dụ.com". React Router cũng chặn
 * chuyển ra ngoài, nhưng bằng cách ném lỗi, trang sập. Màn đăng nhập và thiết lập cũng bỏ, tránh vòng lặp. */
export function safeNext(raw: string | null): string {
  if (!raw || !raw.startsWith('/') || raw.startsWith('//') || /[\s\\]/.test(raw)) return '/'
  // Router không phân biệt hoa thường và tự bỏ "." và "..": so trên đường dẫn đã chuẩn hóa, để /LOGIN hay /w/../login
  // cũng không đưa người vừa đăng nhập quay lại màn đăng nhập.
  const path = new URL(raw, 'http://localhost').pathname.toLowerCase()
  if (path.startsWith(loginRoute) || path.startsWith(setupRoute)) return '/'
  return raw
}
