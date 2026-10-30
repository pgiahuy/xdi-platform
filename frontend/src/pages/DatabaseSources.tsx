// SPDX-License-Identifier: Apache-2.0
/* pages/DatabaseSources.tsx - khung "Kết nối database" của trang Dữ liệu (#165, chỉ Postgres): nút mở form thêm nguồn,
rồi với mỗi nguồn đã có: thử kết nối, chọn bảng, chụp bảng thành dataset. DatasetsPage chỉ hiện khung này cho admin;
backend vẫn trả 403 cho member. Nối hook (src/api/sources.ts) với component (ConnectForm, SourcePanel, RelationPicker).
Chọn bảng xong thì mọi bảng mà nó trỏ tới (nhiều-một) chụp kèm được đều chọn sẵn; bảng trỏ vào nó để người dùng tự chọn. */

import { useState } from 'react'
import {
  useCreateSource,
  useDeleteSource,
  useSnapshot,
  useSourceRelations,
  useSourceTables,
  useSources,
  useTestSource,
  type RelationsResponse,
  type SnapshotResponse,
  type Source,
} from '@/api/sources'
import { ConnectForm, type ConnectValues } from '@/components/datasets/ConnectForm'
import { RelationPicker } from '@/components/datasets/RelationPicker'
import { SourcePanel } from '@/components/datasets/SourcePanel'
import type { RelationOption } from '@/components/datasets/types'
import { FormMessage } from '@/components/forms/FormMessage'
import { Button } from '@/components/ui/button'

// Khớp SSL_MODES của backend (service/sources/postgres.rs): bản này chưa hỗ trợ máy chủ chỉ nhận TLS.
const SSL_MODES = ['prefer', 'disable']
const EMPTY: ConnectValues = { host: '', port: '5432', database: '', user: '', password: '', sslmode: 'prefer' }

export function DatabaseSources({ ws }: { ws: string }) {
  const sources = useSources(ws)
  const create = useCreateSource(ws)
  const [open, setOpen] = useState(false)
  const [values, setValues] = useState(EMPTY)

  const submit = () => {
    const port = Number(values.port)
    create.mutate(
      { ...values, port: Number.isInteger(port) && port > 0 ? port : null },
      {
        onSuccess: () => {
          setValues(EMPTY)
          setOpen(false)
        },
      },
    )
  }

  return (
    <section
      aria-labelledby="database-sources-heading"
      className="flex flex-col gap-4 rounded-xl border bg-card p-4 shadow-xs sm:px-6 sm:py-5"
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 id="database-sources-heading" className="text-[15px] font-semibold tracking-tight">
          Database
        </h2>
        {!open && (
          <Button variant="outline" size="sm" onClick={() => setOpen(true)}>
            Kết nối database
          </Button>
        )}
      </div>
      <p className="text-sm text-muted-foreground">
        Chụp bảng Postgres thành dataset, có thể kèm các bảng liên quan theo khóa ngoại (mỗi bảng một dataset); mỗi lần
        chụp là một phiên bản, chụp lại khi dữ liệu bên đó đổi.
      </p>
      {open && (
        <ConnectForm
          values={values}
          onChange={setValues}
          sslModes={SSL_MODES}
          busy={create.isPending}
          error={create.error?.message}
          onSubmit={submit}
          onCancel={() => {
            setOpen(false)
            create.reset()
          }}
        />
      )}
      {sources.error && <FormMessage kind="error">{sources.error.message}</FormMessage>}
      {sources.data?.map((source) => <SourceItem key={source.id} ws={ws} source={source} />)}
    </section>
  )
}

function sourceLabel(source: Source): string {
  return `${source.host}:${source.port}/${source.database} (${source.user})`
}

/** Nhiều-một trước, rồi một-nhiều; bảng trùng (hai khóa cùng tới một bảng) vẫn hiện từng khóa. */
function toOptions(relations: RelationsResponse): RelationOption[] {
  return [...relations.many_to_one, ...relations.one_to_many].map((r) => ({
    table: r.table,
    fromTable: r.from_table,
    fromColumns: r.from_columns,
    addable: r.addable,
    reason: r.reason,
    columnCount: r.column_count,
  }))
}

/** Mặc định: mọi bảng mà bảng đang chọn trỏ tới, nếu chụp kèm được. */
function defaultSelection(relations: RelationsResponse | undefined): string[] {
  const tables = (relations?.many_to_one ?? []).filter((r) => r.addable).map((r) => r.table)
  return [...new Set(tables)]
}

/** "Đã chụp 3 bảng: orders phiên bản 2 (120 dòng), customers không đổi, ..." */
function resultText(result: SnapshotResponse): string {
  const parts = result.datasets.map((d) =>
    d.created
      ? `${d.dataset.name} phiên bản ${d.version.version} (${d.version.row_count ?? 0} dòng)`
      : `${d.dataset.name} không đổi`,
  )
  const relations = result.relations.length > 0 ? `, ghi ${result.relations.length} quan hệ` : ''
  return `Đã chụp ${result.datasets.length} bảng: ${parts.join(', ')}${relations}.`
}

function SourceItem({ ws, source }: { ws: string; source: Source }) {
  const tables = useSourceTables(ws, source.id)
  const snapshot = useSnapshot(ws)
  const remove = useDeleteSource(ws)
  const testSource = useTestSource(ws)
  const [table, setTable] = useState('')
  const relations = useSourceRelations(ws, source.id, table)
  // Lựa chọn gắn với bảng đã chọn: đổi bảng thì lựa chọn cũ không còn áp dụng, dùng mặc định của bảng mới. Suy ra lúc
  // dựng thay vì đặt lại trong effect, để không có lần vẽ nào dùng lựa chọn của bảng trước.
  const [choice, setChoice] = useState<{ table: string; tables: string[] }>()
  const [name, setName] = useState('')
  const [message, setMessage] = useState<{ kind: 'error' | 'success'; text: string }>()

  const selected = choice?.table === table ? choice.tables : defaultSelection(relations.data)

  const toggle = (option: RelationOption, checked: boolean) => {
    const rest = selected.filter((t) => t !== option.table)
    setChoice({ table, tables: checked ? [...rest, option.table] : rest })
  }

  const test = () => {
    testSource.mutate(source.id, {
      onSuccess: (result) => {
        setMessage({ kind: 'success', text: `Kết nối được, đọc được ${result.table_count} bảng và view.` })
        void tables.refetch()
      },
      onError: (error) => setMessage({ kind: 'error', text: error.message }),
    })
  }

  const take = () => {
    snapshot.mutate(
      { sourceId: source.id, table, name: name.trim() || null, tables: selected },
      {
        onSuccess: (result) => {
          setMessage({ kind: 'success', text: resultText(result) })
          setName('')
        },
        onError: (error) => setMessage({ kind: 'error', text: error.message }),
      },
    )
  }

  return (
    <SourcePanel
      label={sourceLabel(source)}
      tables={tables.data?.map((t) => ({ fullName: `${t.schema}.${t.name}`, kind: t.kind }))}
      tablesLoading={tables.isLoading}
      tablesError={tables.error?.message}
      table={table}
      onTableChange={setTable}
      relations={
        table === '' ? undefined : relations.isLoading ? (
          <p className="text-sm text-muted-foreground">Đang đọc khóa ngoại của bảng...</p>
        ) : relations.error ? (
          <FormMessage kind="error">{relations.error.message}</FormMessage>
        ) : relations.data ? (
          <RelationPicker table={table} options={toOptions(relations.data)} selected={selected} onToggle={toggle} />
        ) : undefined
      }
      name={name}
      onNameChange={setName}
      busy={snapshot.isPending || testSource.isPending || remove.isPending}
      message={message}
      onTest={test}
      onSnapshot={take}
      onDelete={() =>
        remove.mutate(source.id, { onError: (error) => setMessage({ kind: 'error', text: error.message }) })
      }
    />
  )
}
