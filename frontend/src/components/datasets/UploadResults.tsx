// SPDX-License-Identifier: Apache-2.0
/* components/datasets/UploadResults.tsx - tiến trình và kết quả từng lần gửi file lên backend.
Hai giai đoạn hiện riêng: đang gửi (có phần trăm) và máy chủ đang xử lý (đổi sang Parquet, không đo được tiến trình),
để file lớn gửi xong mà chưa có kết quả thì người dùng biết trang vẫn chạy. Lỗi hiện nguyên lời backend trả, vì lời đó
đã nêu tên file hoặc sheet gây lỗi. Nạp trùng là kết quả bình thường, không hiện như lỗi. */

import type { ColumnInfo, UploadEntry, UploadStatus } from './types'

const numberFormat = new Intl.NumberFormat('vi-VN')

export function UploadResults({ entries }: { entries: UploadEntry[] }) {
  return (
    // aria-live để trình đọc màn hình đọc kết quả khi có, không cần rời khỏi nút vừa bấm. Vùng có sẵn từ đầu, kể cả khi
    // chưa có gì: vùng chèn vào cùng lúc với nội dung thì thường không được đọc. Chữ phần trăm để aria-hidden, nên
    // vùng chỉ đọc khi đổi giai đoạn (chờ, đang gửi, đang xử lý, kết quả).
    <div aria-live="polite">
      {entries.length > 0 && (
        <ul className="mt-3 flex flex-col gap-2">
          {entries.map((entry) => (
            <li key={entry.id} className="rounded-lg border bg-card px-3 py-2 text-sm">
              <p className="font-medium break-words">{entry.label}</p>
              <StatusLine status={entry.status} />
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}

function StatusLine({ status }: { status: UploadStatus }) {
  switch (status.kind) {
    case 'queued':
      return <p className="mt-1 text-muted-foreground">Chờ lượt, nạp sau phần phía trên.</p>
    case 'sending': {
      const percent = Math.round(status.progress * 100)
      return (
        <div className="mt-1 flex items-center gap-2 text-muted-foreground">
          <progress
            className="h-2 min-w-0 flex-1 appearance-none overflow-hidden rounded-full bg-muted [&::-moz-progress-bar]:bg-primary [&::-webkit-progress-bar]:bg-muted [&::-webkit-progress-value]:bg-primary"
            max={100}
            value={percent}
            aria-label="Đã gửi"
          />
          <span className="shrink-0 tabular-nums">
            Đang gửi<span aria-hidden="true"> {percent}%</span>
          </span>
        </div>
      )
    }
    case 'processing':
      return <p className="mt-1 text-muted-foreground">Đã gửi xong, máy chủ đang xử lý...</p>
    case 'created':
      return (
        <div className="mt-1">
          <p>
            Đã nạp vào {status.datasetName}, phiên bản {status.version}
            {status.rowCount !== null && `, ${numberFormat.format(status.rowCount)} dòng`}.
          </p>
          <ColumnList columns={status.columns} />
        </div>
      )
    case 'duplicate':
      return (
        <p className="mt-1">
          Đã có: dữ liệu này giống hệt phiên bản {status.version} của {status.datasetName}, không nạp lại.
        </p>
      )
    case 'error':
      return <p className="mt-1 break-words text-destructive">{status.message}</p>
  }
}

function ColumnList({ columns }: { columns: ColumnInfo[] }) {
  if (columns.length === 0) return null
  return (
    <details className="mt-1">
      <summary className="cursor-pointer text-muted-foreground">{columns.length} cột</summary>
      <dl className="mt-1 grid grid-cols-[minmax(0,1fr)_auto] gap-x-4 text-xs">
        {columns.map((column) => (
          <div key={column.name} className="contents">
            <dt className="break-all">{column.name}</dt>
            <dd className="text-muted-foreground">{column.dataType}</dd>
          </div>
        ))}
      </dl>
    </details>
  )
}
