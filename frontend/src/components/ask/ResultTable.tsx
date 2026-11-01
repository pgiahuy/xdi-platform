// SPDX-License-Identifier: Apache-2.0
/* components/ask/ResultTable.tsx - bảng kết quả: phân trang trong các dòng backend đã gửi (tối đa 1000), dòng ghi số
dòng, nút tải CSV. Số dòng ghi theo hợp đồng (ARCHITECTURE mục 8): bị cắt thì "N trên M dòng", chạm trần của guard thì
"ít nhất M dòng", để không nói sai tổng. Trang gắn key theo response để về trang 1 khi có kết quả mới.
Tab Dòng nguồn của pane bằng chứng dùng lại bảng này với tiêu đề riêng, không có nút tải CSV (ngoài phạm vi #52).
Có cellDrill thì ô của cột số đo là nút: bấm thì gửi DrillTarget dựng từ giá trị gốc các cột khóa của dòng đó (#34), để
backend so đúng giá trị ô đã gửi, kể cả chuỗi decimal và null. */

import { useId, useState } from 'react'
import { ArrowUpRight, ChevronLeft, ChevronRight, Download } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { buildDisplayKind, formatCell } from './cellFormat'
import { describeDrillTarget } from './drillTarget'
import type { CellDrill, DrillTarget } from './evidenceTypes'
import { buildResultCsv, downloadCsv } from './resultCsv'
import { SectionHeading } from './SectionHeading'
import { useSectionOpen } from './sectionOpen'
import type { Cell, CellKind, ResultView } from './types'

type Props = {
  result: ResultView
  /** Tên file CSV khi tải; không có thì không có nút tải. */
  fileName?: string
  heading?: string
  /** Ô số đo bấm được để xem dòng nguồn; không có thì bảng chỉ để đọc. */
  cellDrill?: CellDrill
  /** Ô không xuống dòng, chữ dài cắt bớt (đủ chữ ở title), bảng cuộn ngang: cho bảng nhiều cột trong khung hẹp. */
  nowrap?: boolean
  /** Tiêu đề có nút thu gọn bảng (bảng kết quả trong khung trò chuyện); mặc định mở. */
  collapsible?: boolean
}

// 50 dòng vừa một hai lần cuộn trên màn laptop; 1000 dòng là 20 trang.
const pageSize = 50
const countFormat = new Intl.NumberFormat('vi-VN')

export function ResultTable(props: Props) {
  const { result, fileName, heading = 'Kết quả', cellDrill, nowrap = false, collapsible = false } = props
  // id theo từng bảng: trang có thể có hai bảng cùng lúc (kết quả và dòng nguồn), id trùng thì nhãn trỏ sai bảng.
  const headingId = useId()
  const bodyId = useId()
  const [open, toggle] = useSectionOpen(true)
  const [page, setPage] = useState(0)
  const pageCount = Math.max(1, Math.ceil(result.rows.length / pageSize))
  const rows = result.rows.slice(page * pageSize, (page + 1) * pageSize)
  // Xét mọi dòng, không chỉ trang đang xem, để cột không đổi cách hiện khi chuyển trang.
  const kinds = result.columns.map((column, c) => buildDisplayKind(column.kind, result.rows.map((row) => row[c])))
  // Cột số đo duy nhất của bảng, nếu có: cả dòng bấm được để xem dòng nguồn của ô đó.
  const rowDrill = cellDrill?.measures.length === 1 ? cellDrill.measures[0] : undefined
  const partial = result.truncated || result.limitReached

  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          {collapsible ? (
            <SectionHeading
              id={headingId}
              open={open}
              onToggle={toggle}
              controls={bodyId}
              className="text-sm font-semibold"
            >
              {heading}
            </SectionHeading>
          ) : (
            <h3 id={headingId} className="text-sm font-semibold">
              {heading}
            </h3>
          )}
          <p className="text-xs text-muted-foreground">{describeCount(result)}</p>
        </div>
        {fileName && (
          <Button variant="outline" size="sm" onClick={() => downloadCsv(buildResultCsv(result), fileName)}>
            <Download aria-hidden />
            Tải CSV
          </Button>
        )}
      </div>
      <div id={bodyId} hidden={collapsible && !open} className="flex flex-col gap-3">
        {fileName && partial && (
          <p className="text-xs text-muted-foreground">
            Tệp CSV chỉ gồm {countFormat.format(result.rows.length)} dòng đã nhận, không phải toàn bộ kết quả.
          </p>
        )}
        {/* Bảng rộng hơn màn thì cuộn ngang trong khung; khung nhận focus để người dùng bàn phím cuộn được. */}
        <div tabIndex={0} role="region" aria-labelledby={headingId} className="overflow-x-auto rounded-[10px] border">
          <table className="w-full text-sm">
            <thead className="bg-background text-xs text-muted-foreground">
              <tr>
                {result.columns.map((column) => (
                  <th
                    key={column.name}
                    scope="col"
                    className={cn('px-3.5 py-2.5 font-semibold whitespace-nowrap', alignClass(column.kind))}
                  >
                    {column.name}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row, r) => (
                <tr
                  key={page * pageSize + r}
                  className={cn('border-t hover:bg-background/60', rowDrill !== undefined && 'cursor-pointer')}
                  // Bảng chỉ có một cột số đo: bấm chỗ nào trên dòng cũng mở dòng nguồn của ô đó, không phải nhắm đúng con
                  // số. Bàn phím vẫn đi qua nút trong ô, nên dòng không cần tabIndex hay role riêng.
                  onClick={
                    rowDrill === undefined || cellDrill === undefined
                      ? undefined
                      : () => cellDrill.onDrill(buildTarget(result, kinds, cellDrill, row, rowDrill))
                  }
                >
                  {row.map((cell, c) => (
                    <td
                      key={c}
                      className={cn(
                        cellDrill?.measures.includes(c) ? 'p-0' : 'px-3.5 py-2',
                        alignClass(result.columns[c].kind),
                        nowrap && 'whitespace-nowrap',
                      )}
                    >
                      {cellDrill?.measures.includes(c) ? (
                        <DrillCell
                          cell={cell}
                          kind={kinds[c]}
                          target={buildTarget(result, kinds, cellDrill, row, c)}
                          onDrill={cellDrill.onDrill}
                        />
                      ) : nowrap ? (
                        <TruncatedText cell={cell} kind={kinds[c]} />
                      ) : (
                        <CellText cell={cell} kind={kinds[c]} />
                      )}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {pageCount > 1 && (
          <nav
            aria-label={`Trang của bảng ${heading.toLowerCase()}`}
            className="flex items-center justify-end gap-2 text-sm"
          >
            <Button
              variant="outline"
              size="icon-sm"
              aria-label="Trang trước"
              disabled={page === 0}
              onClick={() => setPage(page - 1)}
            >
              <ChevronLeft aria-hidden />
            </Button>
            <span aria-live="polite">
              Trang {page + 1}/{pageCount}
            </span>
            <Button
              variant="outline"
              size="icon-sm"
              aria-label="Trang sau"
              disabled={page === pageCount - 1}
              onClick={() => setPage(page + 1)}
            >
              <ChevronRight aria-hidden />
            </Button>
          </nav>
        )}
      </div>
    </section>
  )
}

function CellText({ cell, kind }: { cell: Cell; kind: CellKind }) {
  if (cell === null) return <span className="text-muted-foreground">trống</span>
  return formatCell(cell, kind)
}

// Dòng nguồn có cột chữ dài (địa chỉ, mô tả sản phẩm): xuống dòng thì mỗi dòng cao gấp mấy lần, nên cắt ở 16rem và
// để đủ chữ trong title.
function TruncatedText({ cell, kind }: { cell: Cell; kind: CellKind }) {
  const text = cell === null ? undefined : formatCell(cell, kind)
  return (
    <span className="block max-w-64 truncate" title={text}>
      <CellText cell={cell} kind={kind} />
    </span>
  )
}

type DrillCellProps = {
  cell: Cell
  kind: CellKind
  target: DrillTarget
  onDrill: (target: DrillTarget) => void
}

// Button của shadcn là thẻ button thật: Tab đi tới được, Enter và Space bấm được, không cần tự bắt phím. Chữ xanh ngọc
// và gạch chân chấm cho biết ô bấm được trước khi rê chuột; mũi tên hiện khi rê chuột hoặc focus bằng bàn phím.
function DrillCell({ cell, kind, target, onDrill }: DrillCellProps) {
  return (
    <Button
      variant="link"
      size="xs"
      // Nút phủ cả ô (đệm của ô chuyển vào nút): bấm vào khoảng trống trong ô cũng mở dòng nguồn. select-text thay
      // select-none của Button: chép bảng ra vẫn có số đo.
      className="group/drill flex h-auto w-full justify-end gap-1 rounded-none px-3.5 py-2 tabular-nums underline decoration-label/60 decoration-dotted underline-offset-4 select-text"
      aria-label={`Xem dòng nguồn: ${describeDrillTarget(target)}`}
      onClick={(event) => {
        // Dòng cũng bắt click khi bảng có một cột số đo; chặn để không mở hai lần.
        event.stopPropagation()
        onDrill(target)
      }}
    >
      <CellText cell={cell} kind={kind} />
      <ArrowUpRight
        aria-hidden
        className="opacity-0 transition-opacity group-hover/drill:opacity-100 group-focus-visible/drill:opacity-100"
      />
    </Button>
  )
}

// kinds là kind dùng để hiện từng cột (buildDisplayKind): dòng "Đang xem" hiện khóa giống ô trong bảng, ví dụ
// "01/07/2017" thay cho "01/07/2017 00:00:00". Giá trị gửi drill vẫn là giá trị gốc của ô.
function buildTarget(
  result: ResultView,
  kinds: CellKind[],
  cellDrill: CellDrill,
  row: Cell[],
  column: number,
): DrillTarget {
  const keys = cellDrill.keys.map((k) => ({
    column: result.columns[k].name,
    value: row[k],
    kind: kinds[k],
  }))
  return { from: 'cell', keys, measure: result.columns[column].name, value: row[column], kind: kinds[column] }
}

function describeCount({ rows, totalRows, truncated, limitReached }: ResultView): string {
  const received = countFormat.format(rows.length)
  const total = countFormat.format(totalRows)
  if (limitReached) return `Ít nhất ${total} dòng (chạm giới hạn số dòng của truy vấn), hiện ${received} dòng đầu.`
  if (truncated) return `Hiện ${received} trên ${total} dòng.`
  return `${total} dòng.`
}

// Số canh phải với chữ số đều bề ngang, để cột số so được bằng mắt.
function alignClass(kind: CellKind): string {
  return kind === 'number' || kind === 'decimal' ? 'text-right tabular-nums' : 'text-left'
}
