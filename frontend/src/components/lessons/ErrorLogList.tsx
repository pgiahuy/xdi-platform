// SPDX-License-Identifier: Apache-2.0
/* components/lessons/ErrorLogList.tsx - nhật ký lỗi (#228): mỗi dòng là một lần mô hình sai (guard từ chối, SQL lỗi,
cộng lặp, form sai, lời bị bộ kiểm chặn) hay một câu bị đánh giá Chưa đạt. Thẻ ghi tên từng phần: câu hỏi, lỗi, câu
mô hình đã trả.

Nhãn loại lỗi có màu và icon theo nhóm (bị chặn, chạy lỗi, đánh giá Chưa đạt) để lướt nhanh; hàng nút lọc theo loại
hiện khi nhật ký có từ hai loại trở lên, mỗi loại đang có một nút. Câu mô hình trả gập lại, mở khi cần: form JSON hay SQL có khi dài hàng chục dòng, để mở sẵn thì vài
dòng nhật ký đã kín màn hình. Nút "Viết bài học" mở hộp viết bài học gắn với dòng đó. Chỉ nhận dữ liệu qua props. */

import { useState } from 'react'
import { CircleCheck, ShieldAlert, ThumbsDown, TriangleAlert, type LucideIcon } from 'lucide-react'
import { SqlCode } from '@/components/ask/SqlCode'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { CardField } from './CardField'
import type { ErrorItem, ErrorTone } from './types'

type Props = {
  items: ErrorItem[]
  onWriteLesson: (item: ErrorItem) => void
}

const dateFormat = new Intl.DateTimeFormat('vi-VN', { dateStyle: 'short', timeStyle: 'short' })

const tones: Record<ErrorTone, { icon: LucideIcon; badge: string }> = {
  blocked: { icon: ShieldAlert, badge: 'border-highlight-border bg-highlight-soft text-highlight-text' },
  failed: { icon: TriangleAlert, badge: 'border-destructive/30 bg-destructive/10 text-destructive' },
  feedback: { icon: ThumbsDown, badge: 'border-info-border bg-info-soft text-info-text' },
}

const badgeClass = 'inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-medium whitespace-nowrap'

// Câu mô hình trả là SQL thì hiện như khung SQL; form JSON hay lời viết thì để chữ thường.
const looksLikeSql = (text: string) => /^\s*(select|with)\b/i.test(text)

// Form JSON mô hình trả thường nằm trên một dòng; xuống dòng theo cấp để đọc được. Không phải JSON thì giữ nguyên.
function readableOutput(text: string): string {
  try {
    return JSON.stringify(JSON.parse(text), null, 2)
  } catch {
    return text
  }
}

/** Các loại lỗi có trong nhật ký, theo thứ tự gặp lần đầu, kèm số dòng. */
function buildStepFilters(items: ErrorItem[]): { step: string; label: string; count: number }[] {
  const filters = new Map<string, { step: string; label: string; count: number }>()
  for (const item of items) {
    const found = filters.get(item.step)
    if (found) found.count += 1
    else filters.set(item.step, { step: item.step, label: item.stepLabel, count: 1 })
  }
  return [...filters.values()]
}

export function ErrorLogList({ items, onWriteLesson }: Props) {
  // null: mọi loại. Loại đã chọn biến khỏi nhật ký (dòng cũ trôi ra) thì coi như chưa chọn.
  const [chosen, setChosen] = useState<string | null>(null)
  const filters = buildStepFilters(items)
  const step = filters.some((f) => f.step === chosen) ? chosen : null
  const shown = step === null ? items : items.filter((item) => item.step === step)

  return (
    <div className="flex flex-col gap-3">
      {filters.length > 1 && (
        <div className="flex flex-wrap gap-2" role="group" aria-label="Lọc theo loại lỗi">
          <FilterChip label="Tất cả" count={items.length} active={step === null} onClick={() => setChosen(null)} />
          {filters.map((f) => (
            <FilterChip key={f.step} label={f.label} count={f.count} active={step === f.step} onClick={() => setChosen(f.step)} />
          ))}
        </div>
      )}
      <ul className="flex flex-col gap-3">
        {shown.map((item) => (
          <ErrorCard key={item.id} item={item} onWriteLesson={onWriteLesson} />
        ))}
      </ul>
    </div>
  )
}

function FilterChip({ label, count, active, onClick }: { label: string; count: number; active: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      aria-pressed={active}
      onClick={onClick}
      className={cn(
        'inline-flex items-center gap-1.5 rounded-full border px-3 py-1 text-xs font-medium transition-colors',
        active ? 'border-label bg-label-soft text-label' : 'bg-card text-muted-foreground hover:border-label hover:text-foreground',
      )}
    >
      {label}
      <span className={cn('tabular-nums', active ? 'text-label' : 'text-muted-foreground')}>{count}</span>
    </button>
  )
}

function ErrorCard({ item, onWriteLesson }: { item: ErrorItem; onWriteLesson: (item: ErrorItem) => void }) {
  const tone = tones[item.tone]
  const Icon = tone.icon
  return (
    <li className="flex flex-col gap-3 rounded-xl border bg-card p-4">
      <div className="flex flex-wrap items-center gap-2">
        <span className={cn(badgeClass, tone.badge)}>
          <Icon aria-hidden className="size-3.5" />
          {item.stepLabel}
        </span>
        {item.hasLesson && (
          <span className={cn(badgeClass, 'border-label-border bg-label-soft text-label')}>
            <CircleCheck aria-hidden className="size-3.5" />
            Đã có bài học
          </span>
        )}
        <span className="text-xs text-muted-foreground">
          {[item.datasetName ?? 'Dataset đã xóa', dateFormat.format(new Date(item.createdAt))].join(', ')}
        </span>
      </div>
      <dl className="flex flex-col gap-3">
        <CardField label="Câu hỏi">
          <span className="text-[15px] font-semibold">{item.question}</span>
        </CardField>
        <CardField label="Lỗi">
          <span className="text-sm">{item.error}</span>
        </CardField>
      </dl>
      <div className="flex flex-wrap items-start justify-between gap-3">
        {item.modelOutput ? (
          <details className="min-w-0 flex-1 text-xs">
            <summary className="cursor-pointer text-muted-foreground">Câu mô hình đã trả</summary>
            {looksLikeSql(item.modelOutput) ? (
              <SqlCode sql={item.modelOutput} className="mt-2" />
            ) : (
              <pre className="mt-2 max-h-72 overflow-auto rounded-md bg-muted px-3 py-2 font-mono text-xs whitespace-pre-wrap wrap-break-word">
                {readableOutput(item.modelOutput)}
              </pre>
            )}
          </details>
        ) : (
          <span className="flex-1" />
        )}
        <Button size="sm" variant={item.hasLesson ? 'outline' : 'default'} onClick={() => onWriteLesson(item)}>
          Viết bài học
        </Button>
      </div>
    </li>
  )
}
