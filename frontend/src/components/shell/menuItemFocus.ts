// SPDX-License-Identifier: Apache-2.0
/* components/shell/menuItemFocus.ts - lớp CSS thêm vào mục menu (DropdownMenuItem) trong khung giao diện.
shadcn chỉ đổi nền mục đang chọn sang --accent, tương phản khoảng 1,1:1 với nền menu: người dùng bàn phím khó thấy
mình đang ở mục nào (WCAG 2.2 mục 1.4.11). Thêm viền trong 2px màu ring khi mục ở trạng thái focus-visible, nên viền
thường chỉ hiện khi dùng bàn phím. Thao tác gần nhất là bàn phím rồi mới mở menu bằng chuột thì mục đang rê cũng có
viền: Radix đặt focus bằng script, Chrome vẫn coi là focus-visible. Đặt ở chỗ dùng, không sửa components/ui/
(AGENTS.md mục 3). */

export const menuItemFocus = 'focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset'
