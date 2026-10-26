// SPDX-License-Identifier: Apache-2.0
/* components/ask/SampleModeSwitch.tsx - chỉ hiện ở chế độ dev: bật để trang hỏi trả lời bằng response mẫu thay vì gọi
backend, để làm giao diện khi không có mô hình ngôn ngữ, hay xem phân rã của câu "tại sao" mà backend chưa trả (#38).
Trang chỉ dựng component này sau điều kiện import.meta.env.DEV. */

type Props = {
  checked: boolean
  onChange: (checked: boolean) => void
}

export function SampleModeSwitch({ checked, onChange }: Props) {
  return (
    <label className="flex w-fit items-center gap-2 rounded-lg border border-dashed px-3 py-1.5 text-xs">
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
        className="size-4 accent-primary"
      />
      Dữ liệu mẫu (chỉ có ở chế độ dev): trả lời bằng response mẫu, không gọi backend
    </label>
  )
}

/** Nhãn trên câu trả lời lấy từ response mẫu, để không ai nhầm số minh họa với số thật. */
export function SampleBadge() {
  return (
    <p className="w-fit rounded-md border border-dashed px-2 py-0.5 text-xs font-medium">
      Dữ liệu mẫu: số minh họa, không tính từ dataset
    </p>
  )
}
