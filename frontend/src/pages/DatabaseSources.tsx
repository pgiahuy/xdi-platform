// SPDX-License-Identifier: Apache-2.0
/* pages/DatabaseSources.tsx - khung "Kết nối database" của trang Dữ liệu (#165, chỉ Postgres): nút mở form thêm nguồn,
rồi với mỗi nguồn đã có: thử kết nối, chọn bảng, chụp bảng thành dataset. DatasetsPage chỉ hiện khung này cho admin;
backend vẫn trả 403 cho member. Nối hook (src/api/sources.ts) với component (ConnectForm, SourcePanel). */

import { useState } from 'react'
import {
  useCreateSource,
  useDeleteSource,
  useSnapshot,
  useSourceTables,
  useSources,
  useTestSource,
  type Source,
} from '@/api/sources'
import { ConnectForm, type ConnectValues } from '@/components/datasets/ConnectForm'
import { SourcePanel } from '@/components/datasets/SourcePanel'
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
        Chụp một bảng Postgres thành dataset; mỗi lần chụp là một phiên bản, chụp lại khi dữ liệu bên đó đổi.
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

function SourceItem({ ws, source }: { ws: string; source: Source }) {
  const tables = useSourceTables(ws, source.id)
  const snapshot = useSnapshot(ws)
  const remove = useDeleteSource(ws)
  const testSource = useTestSource(ws)
  const [table, setTable] = useState('')
  const [name, setName] = useState('')
  const [message, setMessage] = useState<{ kind: 'error' | 'success'; text: string }>()

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
      { sourceId: source.id, table, name: name.trim() || null },
      {
        onSuccess: (result) => {
          const text = result.created
            ? `Đã chụp ${table} thành ${result.dataset.name}, phiên bản ${result.version.version}, ${result.version.row_count ?? 0} dòng.`
            : `Bảng không đổi từ lần chụp trước; giữ phiên bản ${result.version.version} của ${result.dataset.name}.`
          setMessage({ kind: 'success', text })
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
