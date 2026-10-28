// SPDX-License-Identifier: Apache-2.0
/* components/ask/columnText.ts - tên cột kèm nghĩa tiếng Việt trong ngoặc, cho thẻ trả lời của câu "tại sao"
(WhyOverview.tsx, ContributionBars.tsx). Nghĩa lấy từ hồ sơ cột (label của explanation); một chỗ viết để mọi nơi gọi
tên cột giống nhau và giống lời tóm tắt backend viết. */

/** "Type (hình thức thanh toán)"; hồ sơ cột không có nghĩa tiếng Việt thì chỉ tên cột. */
export function columnText(column: string, label: string | null): string {
  return label ? `${column} (${label})` : column
}
