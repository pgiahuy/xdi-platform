// SPDX-License-Identifier: Apache-2.0
/* components/report/NewReportForm.tsx - hai cách tạo báo cáo trên trang Báo cáo: báo cáo trống có tiêu đề (rồi thêm câu
trả lời từ trang hỏi bằng nút "Thêm vào báo cáo"), và "Tạo báo cáo tự động" cho một dataset: hệ thống chạy một bộ phân
tích dựng sẵn (độ đo theo thời gian, theo nhóm) với đúng quyền xem dữ liệu của người bấm. Việc gửi do trang làm. */

import { useId, useState, type FormEvent } from 'react'
import { FormMessage } from '@/components/forms/FormMessage'
import { NativeSelect } from '@/components/metrics/NativeSelect'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

type Props = {
  datasets: { value: string; label: string }[]
  onCreate: (title: string) => void
  creating: boolean
  createError: string | null
  onCreateAutomatic: (datasetId: string) => void
  building: boolean
  buildError: string | null
}

export function NewReportForm(props: Props) {
  const titleId = useId()
  const datasetId = useId()
  const [title, setTitle] = useState('')
  const [dataset, setDataset] = useState('')
  const chosen = dataset || props.datasets[0]?.value || ''
  const submitTitle = (event: FormEvent) => {
    event.preventDefault()
    if (title.trim() !== '') props.onCreate(title.trim())
  }
  const submitAutomatic = (event: FormEvent) => {
    event.preventDefault()
    if (chosen) props.onCreateAutomatic(chosen)
  }
  return (
    <div className="grid gap-6 md:grid-cols-2">
      <form onSubmit={submitTitle} className="flex flex-col gap-2 rounded-xl border bg-card p-4">
        <h3 className="text-[15px] font-semibold tracking-tight">Báo cáo mới</h3>
        <p className="text-sm text-muted-foreground">
          Tạo báo cáo trống, rồi bấm "Thêm vào báo cáo" dưới các câu trả lời ở trang hỏi.
        </p>
        <Label htmlFor={titleId}>Tiêu đề</Label>
        <div className="flex gap-2">
          <Input id={titleId} value={title} onChange={(e) => setTitle(e.target.value)} maxLength={200} />
          <Button type="submit" disabled={props.creating || title.trim() === ''}>
            {props.creating ? 'Đang tạo...' : 'Tạo'}
          </Button>
        </div>
        {props.createError && <FormMessage kind="error">{props.createError}</FormMessage>}
      </form>

      <form onSubmit={submitAutomatic} className="flex flex-col gap-2 rounded-xl border bg-card p-4">
        <h3 className="text-[15px] font-semibold tracking-tight">Tạo báo cáo tự động</h3>
        <p className="text-sm text-muted-foreground">
          Hệ thống tính độ đo chính theo thời gian (kỳ đổi mạnh nhất và nhóm tạo ra phần lớn chênh lệch), theo hai chiều
          (nhóm dẫn đầu, mức tập trung, nhóm lệch mức trung bình), trên dữ liệu bạn được xem.
        </p>
        <Label htmlFor={datasetId}>Dataset</Label>
        <div className="flex gap-2">
          <NativeSelect
            id={datasetId}
            className="flex-1"
            options={props.datasets}
            value={chosen}
            onChange={(e) => setDataset(e.target.value)}
            disabled={props.datasets.length === 0}
          />
          <Button type="submit" disabled={props.building || chosen === ''}>
            {props.building ? 'Đang tạo...' : 'Tạo báo cáo tự động'}
          </Button>
        </div>
        {props.building && (
          <p role="status" className="text-xs text-muted-foreground">
            Đang chạy các phân tích, mỗi phần một câu SQL rồi một lần viết lời...
          </p>
        )}
        {props.buildError && <FormMessage kind="error">{props.buildError}</FormMessage>}
      </form>
    </div>
  )
}
