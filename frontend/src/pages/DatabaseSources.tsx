// SPDX-License-Identifier: Apache-2.0
/* pages/DatabaseSources.tsx - khung "Kết nối database" của trang Dữ liệu (#165, chỉ Postgres), luồng chính của trang
(#230, nạp file là lựa chọn phụ bên dưới): nút mở form thêm nguồn, workspace chưa có nguồn nào thì form mở sẵn,
rồi với mỗi nguồn đã có: thử kết nối, chọn bảng, chọn bảng chụp kèm, chụp vào dataset của database (#257). DatasetsPage chỉ hiện khung
này cho admin; backend vẫn trả 403 cho member. Nối hook (src/api/sources.ts) với component (ConnectForm, SourcePanel,
RelatedTablePicker). Cách chọn sẵn, mở thêm một bước và giới hạn số bảng ở relatedTablesView.ts. */

import { useState } from 'react'
import {
  useCreateSource,
  useDeleteSource,
  useSnapshot,
  useSourceRelations,
  useSourceTables,
  useSources,
  useTestSource,
  type SnapshotResponse,
  type Source,
} from '@/api/sources'
import { ConnectForm, type ConnectValues } from '@/components/datasets/ConnectForm'
import { RelatedTablePicker } from '@/components/datasets/RelatedTablePicker'
import { SourcePanel } from '@/components/datasets/SourcePanel'
import type { SnapshotTableResult } from '@/components/datasets/types'
import { FormMessage } from '@/components/forms/FormMessage'
import { Button } from '@/components/ui/button'
import { buildRelatedView, MAX_SNAPSHOT_TABLES, type RelationChoice } from './relatedTablesView'

// Khớp SSL_MODES của backend (service/sources/postgres.rs), nghĩa như sslmode của libpq.
const SSL_MODES = [
  { value: 'prefer', label: 'Dùng TLS nếu máy chủ nhận, không kiểm chứng chỉ.' },
  { value: 'require', label: 'Bắt buộc TLS, không kiểm chứng chỉ: chống nghe lén, không chống giả máy chủ.' },
  { value: 'verify-full', label: 'Bắt buộc TLS, chứng chỉ phải do CA tin cậy cấp và đúng tên máy chủ.' },
  { value: 'disable', label: 'Không dùng TLS. Chỉ dùng trong mạng tin cậy.' },
]
const EMPTY: ConnectValues = {
  host: '',
  port: '5432',
  database: '',
  user: '',
  password: '',
  sslmode: 'prefer',
  sslRootCert: '',
}

export function DatabaseSources({ ws }: { ws: string }) {
  const sources = useSources(ws)
  const create = useCreateSource(ws)
  const [open, setOpen] = useState<boolean>()
  const [values, setValues] = useState(EMPTY)
  // Chưa bấm gì thì form mở khi workspace chưa có nguồn: đó là việc đầu tiên admin cần làm ở trang này.
  const formOpen = open ?? sources.data?.length === 0

  const submit = () => {
    const port = Number(values.port)
    const { sslRootCert, ...connection } = values
    create.mutate(
      {
        ...connection,
        port: Number.isInteger(port) && port > 0 ? port : null,
        // Backend chỉ nhận chứng chỉ CA với verify-full; đổi sang mode khác thì không gửi phần đã dán.
        ssl_root_cert: values.sslmode === 'verify-full' && sslRootCert.trim() !== '' ? sslRootCert : null,
      },
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
          Kết nối database
        </h2>
        {!formOpen && (
          <Button size="sm" onClick={() => setOpen(true)}>
            Thêm database
          </Button>
        )}
      </div>
      <p className="text-sm text-muted-foreground">
        Mỗi database kết nối là một dataset; chụp bảng Postgres của tổ chức vào dataset đó, kèm các bảng liên quan theo
        khóa ngoại, để câu hỏi nối được nhiều bảng. Mỗi lần chụp là một phiên bản, chụp lại khi dữ liệu bên đó đổi.
      </p>
      {formOpen && (
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

function toResult(result: SnapshotResponse): { tables: SnapshotTableResult[]; relationCount: number } {
  return {
    tables: result.datasets.map((d) => ({
      datasetName: d.dataset.name,
      created: d.created,
      version: d.version.version,
      rowCount: d.version.row_count,
    })),
    relationCount: result.relations.length,
  }
}

function SourceItem({ ws, source }: { ws: string; source: Source }) {
  const tables = useSourceTables(ws, source.id)
  const snapshot = useSnapshot(ws)
  const remove = useDeleteSource(ws)
  const testSource = useTestSource(ws)
  const [table, setTable] = useState('')
  // Lựa chọn gắn với bảng bắt đầu: đổi bảng thì lựa chọn cũ không còn áp dụng. Suy ra lúc dựng thay vì đặt lại trong
  // effect, để không có lần vẽ nào dùng lựa chọn của bảng trước.
  const [saved, setChoice] = useState<RelationChoice>()
  const choice: RelationChoice = saved?.start === table ? saved : { start: table, picked: {}, expanded: [] }
  const relations = useSourceRelations(ws, source.id, table === '' ? [] : [table, ...choice.expanded])
  const [name, setName] = useState('')
  const [message, setMessage] = useState<{ kind: 'error' | 'success'; text: string }>()
  const [result, setResult] = useState<SnapshotResponse>()

  const startData = relations[0]?.data
  const view = startData ? buildRelatedView(startData, relations, choice) : undefined

  const toggle = (picked: string, checked: boolean) =>
    setChoice({ ...choice, picked: { ...choice.picked, [picked]: checked } })
  const expand = (opened: string, open: boolean) =>
    setChoice({
      ...choice,
      expanded: open ? [...choice.expanded, opened] : choice.expanded.filter((t) => t !== opened),
    })

  const test = () => {
    setResult(undefined)
    testSource.mutate(source.id, {
      onSuccess: (result) => {
        setMessage({ kind: 'success', text: `Kết nối được, đọc được ${result.table_count} bảng và view.` })
        void tables.refetch()
      },
      onError: (error) => setMessage({ kind: 'error', text: error.message }),
    })
  }

  const take = () => {
    setMessage(undefined)
    setResult(undefined)
    snapshot.mutate(
      { sourceId: source.id, table, name: name.trim() || null, tables: view?.selected ?? [] },
      {
        onSuccess: (done) => {
          setResult(done)
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
        table === '' ? undefined : view ? (
          <RelatedTablePicker
            start={view.start}
            nested={view.nested}
            expandable={view.expandable}
            selected={view.selected}
            max={MAX_SNAPSHOT_TABLES}
            onToggle={toggle}
            onExpand={expand}
          />
        ) : relations[0]?.error ? (
          <FormMessage kind="error">{relations[0].error.message}</FormMessage>
        ) : (
          <p className="text-sm text-muted-foreground">Đang đọc khóa ngoại của bảng...</p>
        )
      }
      name={name}
      onNameChange={setName}
      busy={snapshot.isPending || testSource.isPending || remove.isPending}
      message={message}
      result={result ? toResult(result) : undefined}
      onTest={test}
      onSnapshot={take}
      onDelete={() =>
        remove.mutate(source.id, { onError: (error) => setMessage({ kind: 'error', text: error.message }) })
      }
    />
  )
}
