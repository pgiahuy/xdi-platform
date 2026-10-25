// SPDX-License-Identifier: Apache-2.0
/* components/ask/EvidenceSql.tsx - tab SQL của pane bằng chứng: bản mô hình viết và bản sau guard (bản đã chạy), mỗi
bản một nút chép. Dưới bản sau guard là những gì guard ghi đã thêm (giới hạn số dòng) và lời nhắc về điều kiện phân
quyền. Câu SQL xuống dòng trong khung thay vì cuộn ngang, để trang không rộng hơn màn điện thoại. */

import { useState } from 'react'
import { Copy } from 'lucide-react'
import { Button } from '@/components/ui/button'
import type { SqlView } from './evidenceTypes'

type Props = {
  sql: SqlView
}

export function EvidenceSql({ sql }: Props) {
  return (
    <div className="flex flex-col gap-4">
      <SqlBlock title={sql.repaired ? 'Mô hình viết (bản đã sửa)' : 'Mô hình viết'} text={sql.generated} />
      {sql.guarded ? (
        <div className="flex flex-col gap-2">
          <SqlBlock title="Sau guard (bản đã chạy)" text={sql.guarded} />
          <GuardNotes notes={sql.guardNotes} />
        </div>
      ) : (
        <section className="flex flex-col gap-1">
          <h3 className="text-xs font-semibold">Sau guard</h3>
          <p className="text-sm text-muted-foreground">{sql.guardedMissing}</p>
        </section>
      )}
    </div>
  )
}

function GuardNotes({ notes }: { notes: string[] }) {
  return (
    <div className="flex flex-col gap-1 text-xs text-muted-foreground">
      {notes.length > 0 && (
        <ul className="list-disc pl-4">
          {notes.map((note) => (
            <li key={note}>{note}</li>
          ))}
        </ul>
      )}
      {/* Response chưa tách riêng điều kiện phân quyền guard đã thêm (chỉ có trong câu SQL), nên chỉ nói điều
      kiện đó nằm ở đâu, không chỉ ra từng điều kiện. */}
      <p>Khi workspace có phân quyền dữ liệu, điều kiện lọc theo vai của người hỏi nằm trong bản này.</p>
    </div>
  )
}

function SqlBlock({ title, text }: { title: string; text: string | null }) {
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
        <h3 className="text-xs font-semibold">{title}</h3>
        {text && (
          <Button variant="ghost" size="icon-sm" aria-label={`Chép SQL: ${title}`} onClick={() => copy(text)}>
            <Copy aria-hidden />
          </Button>
        )}
      </div>
      {text ? (
        <pre className="rounded-md bg-muted px-3 py-2 font-mono text-xs break-all whitespace-pre-wrap">{text}</pre>
      ) : (
        <p className="text-sm text-muted-foreground">Mô hình chưa viết SQL cho câu này.</p>
      )}
      <p role="status" className="text-xs text-muted-foreground">
        {copyStatus}
      </p>
    </section>
  )
}
