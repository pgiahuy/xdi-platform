// SPDX-License-Identifier: Apache-2.0
/* components/ask/EvidenceSql.tsx - tab SQL của pane bằng chứng: bản mô hình viết và bản sau guard (bản đã chạy), mỗi
bản một nút chép. Dòng guard thêm chỉ được đánh dấu ngay trong khung (SqlCode.tsx), không kèm câu chú thích bên dưới:
chi tiết bước guard (như giới hạn số dòng) đã có ở tab Các bước, nhắc lại ở đây chỉ làm tab dài thêm với người đọc
không cần nó. Câu SQL hiện đã xuống dòng theo mệnh đề (SqlCode.tsx); nút chép chép nguyên văn câu backend gửi, không
chép bản đã định dạng, để câu dán sang công cụ khác đúng là câu đã chạy. */

import { useState } from 'react'
import { Copy } from 'lucide-react'
import { Button } from '@/components/ui/button'
import type { SqlView } from './evidenceTypes'
import { SqlCode } from './SqlCode'

type Props = {
  sql: SqlView
}

export function EvidenceSql({ sql }: Props) {
  return (
    <div className="flex flex-col gap-4">
      <SqlBlock title={sql.repaired ? 'Mô hình viết (bản đã sửa)' : 'Mô hình viết'} text={sql.generated} />
      {sql.guarded ? (
        <SqlBlock title="Sau guard (bản đã chạy)" text={sql.guarded} compareTo={sql.generated} />
      ) : (
        <section className="flex flex-col gap-1">
          <h3 className="text-xs font-semibold tracking-wider text-label uppercase">Sau guard</h3>
          <p className="text-sm text-muted-foreground">{sql.guardedMissing}</p>
        </section>
      )}
    </div>
  )
}

type SqlBlockProps = { title: string; text: string | null; compareTo?: string | null }

function SqlBlock({ title, text, compareTo }: SqlBlockProps) {
  const [copyStatus, setCopyStatus] = useState('')

  // Clipboard có thể bị chặn (trang không chạy trên https, trình duyệt từ chối quyền): báo để người dùng chọn chữ
  // và chép tay.
  const copy = (value: string) => {
    navigator.clipboard.writeText(value).then(
      () => setCopyStatus('Đã chép.'),
      () => setCopyStatus('Không chép được; chọn chữ rồi chép tay.'),
    )
  }

  return (
    <section className="flex flex-col gap-1">
      <div className="flex items-center justify-between gap-2">
        <h3 className="text-xs font-semibold tracking-wider text-label uppercase">{title}</h3>
        {text && (
          <Button variant="outline" size="icon-sm" aria-label={`Chép SQL: ${title}`} onClick={() => copy(text)}>
            <Copy aria-hidden />
          </Button>
        )}
      </div>
      {text ? (
        <SqlCode sql={text} compareTo={compareTo} />
      ) : (
        <p className="text-sm text-muted-foreground">Mô hình chưa viết SQL cho câu này.</p>
      )}
      <p role="status" className="text-xs text-muted-foreground">
        {copyStatus}
      </p>
    </section>
  )
}
