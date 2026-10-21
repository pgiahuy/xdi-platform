// SPDX-License-Identifier: Apache-2.0
/* components/shell/labels.ts - chữ hiện cho vai và chữ viết tắt của tên.
Backend trả mã vai tiếng Anh (`admin`, `member`, xem docs/ARCHITECTURE.md mục 4b); người dùng đọc tên tiếng Việt.
Vai mới thêm ở backend mà chưa có tên ở đây thì hiện nguyên mã, không hiện trống. */

const roleLabels: Record<string, string> = {
  admin: 'Quản lý',
  member: 'Thành viên',
}

export function roleLabel(role: string): string {
  return roleLabels[role] ?? role
}

/** Hai chữ đầu của tên (chữ đầu của từ đầu và từ cuối), hoặc chữ đầu của email khi không có tên. */
export function initials(displayName: string, email: string): string {
  const words = displayName.trim().split(/\s+/).filter(Boolean)
  if (words.length === 0) return email.charAt(0).toUpperCase()
  const first = words[0].charAt(0)
  const last = words.length > 1 ? words[words.length - 1].charAt(0) : ''
  return (first + last).toUpperCase()
}
