// SPDX-License-Identifier: Apache-2.0
/* components/report/ReportItemCard.tsx - một mục của báo cáo, đọc từ trên xuống như một câu trả lời: câu hỏi và nguồn
(bảng, phiên bản, lúc hỏi), ghi chú khác lúc hỏi, lời trả lời kèm kiểm số và kiểm nghĩa, các ý "Số liệu chính" do hệ thống tính
lại, biểu đồ (trang truyền vào, vì biểu đồ tải riêng), bảng kết quả, phân rã, phần "Cách tính", rồi câu SQL đã chạy.
SQL thu gọn trên màn hình, luôn hiện trong bản in (hai khối, một khối chỉ cho màn hình, một khối chỉ cho bản in). Mỗi
mục bắt đầu một trang in mới, trừ mục đầu. Nút đổi thứ tự và bỏ mục do trang truyền vào và không in. */

import { useId, type ReactNode } from 'react'
import { SqlCode } from '@/components/ask/SqlCode'
import { UnclearPoints } from '@/components/ask/UnclearPoints'
import { cn } from '@/lib/utils'
import { ReportBreakdownTable } from './ReportBreakdownTable'
import { ReportChecks } from './ReportChecks'
import { ReportMethod } from './ReportMethod'
import { ReportTable } from './ReportTable'
import type { ReportItemView } from './types'

const dateFormat = new Intl.DateTimeFormat('vi-VN', { dateStyle: 'short', timeStyle: 'short' })

type Props = {
  item: ReportItemView
  /** Số thứ tự hiện trong báo cáo, từ 1. */
  number: number
  /** Biểu đồ của mục; null khi không có. */
  chart: ReactNode
  /** Nút sửa mục (lên, xuống, bỏ); không in. */
  actions?: ReactNode
}

export function ReportItemCard({ item, number, chart, actions }: Props) {
  const headingId = useId()
  return (
    <article
      aria-labelledby={headingId}
      className={cn(
        'flex flex-col gap-4 rounded-xl border bg-card p-4 text-card-foreground shadow-xs sm:px-6 sm:py-5',
        'print:rounded-none print:border-0 print:p-0 print:shadow-none',
        number > 1 && 'print:break-before-page',
      )}
    >
      <header className="flex flex-col gap-1">
        <div className="flex items-start justify-between gap-3">
          <p className="text-xs font-semibold tracking-wider text-label uppercase">
            Mục {number}, {item.causal ? 'câu hỏi nguyên nhân' : 'câu hỏi số liệu'}
          </p>
          {actions && <div className="flex shrink-0 gap-1 print:hidden">{actions}</div>}
        </div>
        <h3 id={headingId} className="text-base font-semibold tracking-tight">
          {item.question}
        </h3>
        <p className="text-xs text-muted-foreground">{describeSource(item)}</p>
      </header>

      {item.notes.length > 0 && (
        <ul role="note" className="flex list-disc flex-col gap-1 rounded-lg border border-highlight-border bg-highlight-soft py-2 pr-3 pl-7 text-xs">
          {item.notes.map((note) => (
            <li key={note}>{note}</li>
          ))}
        </ul>
      )}
      {item.error && (
        <p role="alert" className="rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive">
          {item.error}
        </p>
      )}

      {item.answer && (
        <div className="flex flex-col gap-2">
          <p className="text-[15px] leading-relaxed tabular-nums">{item.answer.text}</p>
          {item.answer.byModel && item.answer.warnings.length > 0 && (
            <UnclearPoints warnings={item.answer.warnings} checkAgainst="bảng và mục Số liệu chính bên dưới" />
          )}
          <ReportChecks byModel={item.answer.byModel} numberCheck={item.numberCheck} meaningCheck={item.meaningCheck} />
        </div>
      )}

      {item.facts && (
        <section className="flex flex-col gap-1 text-sm">
          <h4 className="font-semibold">Số liệu chính do hệ thống tính</h4>
          <p>{item.facts.title}</p>
          {item.facts.points.length > 0 && (
            <ul className="list-disc pl-5 text-muted-foreground">
              {item.facts.points.map((point) => (
                <li key={point}>{point}</li>
              ))}
            </ul>
          )}
        </section>
      )}

      {chart && <div className="w-full max-w-[680px] break-inside-avoid">{chart}</div>}
      {item.result && <ReportTable result={item.result} />}
      {item.breakdown && <ReportBreakdownTable breakdown={item.breakdown} />}
      {item.method && <ReportMethod method={item.method} />}

      {item.sql && (
        <>
          <details className="text-sm print:hidden">
            <summary className="cursor-pointer font-semibold">SQL đã chạy</summary>
            <SqlCode sql={item.sql} className="mt-2" />
          </details>
          <section className="hidden text-sm print:block">
            <h4 className="font-semibold">SQL đã chạy</h4>
            <pre className="mt-1 text-xs whitespace-pre-wrap">{item.sql}</pre>
          </section>
        </>
      )}
    </article>
  )
}

/** "Hỏi lúc 01/11/2026 10:20, bảng order_items phiên bản 1 (chụp 31/10/2026 09:12)". */
function describeSource(item: ReportItemView): string {
  const tables = item.tables.map((t) => {
    const newer = t.isCurrent ? '' : ', đã có phiên bản mới hơn'
    return `${t.name} phiên bản ${t.version} (chụp ${dateFormat.format(new Date(t.capturedAt))}${newer})`
  })
  const asked = `Hỏi lúc ${dateFormat.format(new Date(item.askedAt))}`
  return tables.length > 0 ? `${asked}, bảng ${tables.join('; ')}` : asked
}
