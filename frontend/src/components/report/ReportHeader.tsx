// SPDX-License-Identifier: Apache-2.0
/* components/report/ReportHeader.tsx - phần đầu của một báo cáo: tiêu đề, đoạn mở đầu (sửa tại chỗ), dòng nguồn gốc
(tự gom hay tự động, số lần gọi mô hình khi tạo tự động, lúc số liệu được tính lại), và hàng nút Xuất PDF, Tải Excel,
Xóa. Hàng nút và ô sửa không in; bản in chỉ có tiêu đề, đoạn mở đầu và dòng nguồn gốc. */

import { useState, type ReactNode } from 'react'
import { FileDown, FileSpreadsheet, Pencil } from 'lucide-react'
import { FormMessage } from '@/components/forms/FormMessage'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'

const dateFormat = new Intl.DateTimeFormat('vi-VN', { dateStyle: 'short', timeStyle: 'short' })

type Props = {
  title: string
  intro: string
  automatic: boolean
  /** Số lần gọi mô hình khi tạo báo cáo tự động; null với báo cáo tự gom. */
  modelCalls: number | null
  /** ISO 8601: lúc các mục được chạy lại. */
  renderedAt: string
  onSave: (change: { title: string; intro: string }) => void
  saving: boolean
  saveError: string | null
  onPrint: () => void
  xlsxHref: string
  /** Nút xóa (kèm hộp hỏi lại) do trang dựng. */
  deleteButton: ReactNode
}

export function ReportHeader(props: Props) {
  const [editing, setEditing] = useState(false)
  const [title, setTitle] = useState(props.title)
  const [intro, setIntro] = useState(props.intro)
  const startEditing = () => {
    setTitle(props.title)
    setIntro(props.intro)
    setEditing(true)
  }
  const save = () => {
    props.onSave({ title: title.trim(), intro: intro.trim() })
    setEditing(false)
  }
  const origin = props.automatic
    ? `Tạo tự động${props.modelCalls === null ? '' : `, ${props.modelCalls} lần gọi mô hình`}`
    : 'Tự gom từ các câu trả lời'
  return (
    <header className="flex flex-col gap-3">
      <div className="flex flex-wrap gap-2 print:hidden">
        <Button variant="outline" size="sm" onClick={props.onPrint}>
          <FileDown aria-hidden />
          Xuất PDF
        </Button>
        <Button variant="outline" size="sm" asChild>
          <a href={props.xlsxHref} download>
            <FileSpreadsheet aria-hidden />
            Tải Excel
          </a>
        </Button>
        {!editing && (
          <Button variant="outline" size="sm" onClick={startEditing}>
            <Pencil aria-hidden />
            Sửa tiêu đề và mở đầu
          </Button>
        )}
        {props.deleteButton}
      </div>

      {editing ? (
        <div className="flex flex-col gap-2 print:hidden">
          <Input aria-label="Tiêu đề báo cáo" value={title} onChange={(e) => setTitle(e.target.value)} maxLength={200} />
          <Textarea
            aria-label="Đoạn mở đầu"
            value={intro}
            onChange={(e) => setIntro(e.target.value)}
            maxLength={5000}
            placeholder="Viết vài câu giới thiệu báo cáo: mục đích, kỳ dữ liệu, người đọc."
          />
          <div className="flex gap-2">
            <Button size="sm" onClick={save} disabled={props.saving || title.trim() === ''}>
              Lưu
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setEditing(false)}>
              Thôi
            </Button>
          </div>
        </div>
      ) : (
        <>
          <h2 className="text-xl font-semibold tracking-tight">{props.title}</h2>
          {props.intro ? (
            <p className="max-w-3xl text-[15px] leading-relaxed whitespace-pre-line">{props.intro}</p>
          ) : (
            <p className="text-sm text-muted-foreground print:hidden">Chưa có đoạn mở đầu.</p>
          )}
        </>
      )}
      {props.saveError && <FormMessage kind="error">{props.saveError}</FormMessage>}
      <p className="text-xs text-muted-foreground">
        {origin}. Số liệu tính lại lúc {dateFormat.format(new Date(props.renderedAt))}, trên đúng phiên bản dữ liệu của
        từng câu hỏi và theo quyền xem dữ liệu của bạn.
      </p>
    </header>
  )
}
