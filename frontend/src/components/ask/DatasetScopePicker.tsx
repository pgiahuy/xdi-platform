// SPDX-License-Identifier: Apache-2.0
/* components/ask/DatasetScopePicker.tsx - ô chọn dataset ở đầu trang hỏi (#257): một dataset là một database đã kết
nối hay một file. Danh sách chia hai nhóm "Database" và "File"; mở một database ra thì chọn được một bảng để ghim làm
bảng bắt đầu, không ghim thì backend tự chọn bảng theo câu hỏi. Kèm dòng báo hồ sơ chưa xong (ProfileNote).
Dùng Popover của shadcn chứ không dùng Select: Select chỉ chọn một giá trị phẳng, còn ở đây một dòng database vừa chọn
được cả dataset vừa mở ra danh sách bảng. Mỗi dòng là một nút, đi bằng phím Tab; nút đang chọn có aria-pressed.
pinTables false thì không có phần mở bảng (trang Tổng quan chọn bảng ở danh sách riêng). */

import { useId, useState, type ReactNode } from 'react'
import { CheckIcon, ChevronDownIcon, ChevronRightIcon, ChevronsUpDownIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { cn } from '@/lib/utils'
import { buildSchemaGroups } from '../tableGroups'
import { ProfileNote } from './ProfileNote'
import type { ProfileProgress, ScopeDatasetOption, ScopeTableOption } from './types'

type Props = {
  datasets: ScopeDatasetOption[]
  datasetId: string
  /** Bảng đang ghim; undefined khi để backend tự chọn. */
  tableId?: string
  /** tableId undefined: bỏ ghim, hỏi trên cả dataset. */
  onChange: (datasetId: string, tableId: string | undefined) => void
  pinTables?: boolean
  profile: ProfileProgress
  disabled: boolean
  /** Nút đứng cạnh ô chọn. */
  action?: ReactNode
}

export function DatasetScopePicker(props: Props) {
  const { datasets, datasetId, tableId, onChange, pinTables = true, profile, disabled, action } = props
  const [open, setOpen] = useState(false)
  const id = useId()
  const current = datasets.find((d) => d.id === datasetId)
  const pinned = current?.tables.find((t) => t.id === tableId)
  if (datasets.length === 0) return null

  const pick = (id: string, table: string | undefined) => {
    onChange(id, table)
    setOpen(false)
  }
  const databases = datasets.filter((d) => d.kind === 'database')
  const files = datasets.filter((d) => d.kind === 'file')
  const label = current ? (pinned ? `${current.name}, bảng ${pinned.name}` : current.name) : 'Chọn dataset'
  return (
    <div className="flex flex-col gap-1">
      <div className="flex flex-wrap items-center gap-2">
        <span id={`${id}-label`} className="text-xs text-muted-foreground">
          Dataset
        </span>
        <Popover open={open} onOpenChange={setOpen}>
          <PopoverTrigger asChild disabled={disabled}>
            <Button
              variant="outline"
              size="sm"
              aria-labelledby={`${id}-label ${id}-value`}
              className="max-w-full min-w-40 justify-between font-normal"
            >
              <span id={`${id}-value`} className="truncate">
                {label}
              </span>
              <ChevronsUpDownIcon aria-hidden="true" className="text-muted-foreground" />
            </Button>
          </PopoverTrigger>
          <PopoverContent align="start" className="max-h-[min(28rem,70vh)] w-80 max-w-[calc(100vw-2rem)] overflow-y-auto">
            {databases.length > 0 && (
              <Group title="Database">
                {databases.map((dataset) => (
                  <DatabaseItem
                    key={dataset.id}
                    dataset={dataset}
                    selected={dataset.id === datasetId}
                    tableId={dataset.id === datasetId ? tableId : undefined}
                    pinTables={pinTables}
                    onPick={(table) => pick(dataset.id, table)}
                  />
                ))}
              </Group>
            )}
            {files.length > 0 && (
              <Group title="File">
                {files.map((dataset) => (
                  <li key={dataset.id}>
                    <ItemButton selected={dataset.id === datasetId} onClick={() => pick(dataset.id, undefined)}>
                      {dataset.name}
                    </ItemButton>
                  </li>
                ))}
              </Group>
            )}
          </PopoverContent>
        </Popover>
        {action}
      </div>
      <ProfileNote progress={profile} />
    </div>
  )
}

function Group({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-1">
      <h3 className="px-2 text-xs font-semibold tracking-wider text-label uppercase">{title}</h3>
      <ul className="flex flex-col">{children}</ul>
    </section>
  )
}

type DatabaseItemProps = {
  dataset: ScopeDatasetOption
  selected: boolean
  tableId: string | undefined
  pinTables: boolean
  onPick: (tableId: string | undefined) => void
}

function DatabaseItem({ dataset, selected, tableId, pinTables, onPick }: DatabaseItemProps) {
  // Database đang chọn mở sẵn: người mở ô chọn thường để đổi bảng ghim của chính nó.
  const [expanded, setExpanded] = useState(selected && pinTables)
  const canExpand = pinTables && dataset.tables.length > 0
  const tablesId = `scope-tables-${dataset.id}`
  return (
    <li className="flex flex-col">
      <div className="flex items-center gap-1">
        <ItemButton selected={selected && tableId === undefined} onClick={() => onPick(undefined)}>
          <span className="truncate">{dataset.name}</span>
          <span className="ml-auto shrink-0 text-xs text-muted-foreground">{dataset.tables.length} bảng</span>
        </ItemButton>
        {canExpand && (
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={`Chọn bảng của ${dataset.name}`}
            aria-expanded={expanded}
            aria-controls={tablesId}
            onClick={() => setExpanded(!expanded)}
          >
            {expanded ? <ChevronDownIcon aria-hidden="true" /> : <ChevronRightIcon aria-hidden="true" />}
          </Button>
        )}
      </div>
      {canExpand && expanded && (
        <div id={tablesId} className="ml-3 flex flex-col gap-1 border-l pl-2">
          <p className="px-2 pt-1 text-xs text-muted-foreground">Ghim một bảng làm bảng bắt đầu (không bắt buộc).</p>
          <ul className="flex flex-col">
            <li>
              <ItemButton selected={selected && tableId === undefined} onClick={() => onPick(undefined)}>
                Tự chọn bảng theo câu hỏi
              </ItemButton>
            </li>
          </ul>
          <TableList tables={dataset.tables} selectedId={selected ? tableId : undefined} onPick={onPick} />
        </div>
      )}
    </li>
  )
}

type TableListProps = {
  tables: ScopeTableOption[]
  selectedId: string | undefined
  onPick: (tableId: string) => void
}

function TableList({ tables, selectedId, onPick }: TableListProps) {
  const groups = buildSchemaGroups(tables)
  return (
    <>
      {groups.map((group) => (
        <div key={group.schema ?? ''} className="flex flex-col">
          {group.schema && <p className="px-2 pt-1 text-xs text-muted-foreground">Schema {group.schema}</p>}
          <ul className="flex flex-col">
            {group.tables.map((table) => (
              <li key={table.id}>
                <ItemButton selected={table.id === selectedId} onClick={() => onPick(table.id)}>
                  <code className="truncate text-[0.8rem]">{table.name}</code>
                </ItemButton>
              </li>
            ))}
          </ul>
        </div>
      ))}
    </>
  )
}

function ItemButton({ selected, onClick, children }: { selected: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      aria-pressed={selected}
      onClick={onClick}
      className={cn(
        'flex w-full min-w-0 flex-1 items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm',
        'hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none',
        selected && 'bg-accent font-medium',
      )}
    >
      <CheckIcon aria-hidden="true" className={cn('size-4 shrink-0', !selected && 'invisible')} />
      {children}
    </button>
  )
}
