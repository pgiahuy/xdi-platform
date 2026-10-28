// SPDX-License-Identifier: Apache-2.0
/* components/ask/DatasetPicker.tsx - ô chọn dataset ở đầu trang hỏi, kèm dòng báo hồ sơ chưa xong. Đặt trên cùng,
tách khỏi ô hỏi ở đáy: dataset quyết định câu gợi ý và danh sách cuộc trò chuyện bên dưới, nên người dùng chọn nó
trước khi đọc phần còn lại.
Dùng Select của shadcn (Radix) thay cho select của trình duyệt, để giống các ô khác trên trang; Radix vẫn cho chọn
bằng bàn phím và trình đọc màn hình đọc được như một combobox. */

import type { ReactNode } from 'react'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { ProfileNote } from './ProfileNote'
import type { DatasetOption, ProfileProgress } from './types'

type Props = {
  datasets: DatasetOption[]
  datasetId: string
  onDatasetChange: (id: string) => void
  profile: ProfileProgress
  disabled: boolean
  /** Nút đứng cạnh ô chọn, ví dụ mở khung tổng quan dữ liệu (#198). */
  action?: ReactNode
}

export function DatasetPicker({ datasets, datasetId, onDatasetChange, profile, disabled, action }: Props) {
  if (datasets.length === 0) return null
  return (
    <div className="flex flex-col gap-1">
      <div className="flex flex-wrap items-center gap-2">
        <Label htmlFor="ask-dataset" className="text-xs text-muted-foreground">
          Dataset
        </Label>
        <Select value={datasetId} onValueChange={onDatasetChange} disabled={disabled}>
          <SelectTrigger id="ask-dataset" size="sm" className="max-w-full min-w-40 flex-1 sm:flex-none">
            <SelectValue placeholder="Chọn dataset" />
          </SelectTrigger>
          <SelectContent>
            {datasets.map((dataset) => (
              <SelectItem key={dataset.id} value={dataset.id}>
                {dataset.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        {action}
      </div>
      <ProfileNote progress={profile} />
    </div>
  )
}
