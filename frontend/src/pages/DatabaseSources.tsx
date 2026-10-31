// SPDX-License-Identifier: Apache-2.0
/* pages/DatabaseSources.tsx - khung "Kết nối database" của trang Dữ liệu (#165, chỉ Postgres), luồng chính của trang
(#230, nạp file là lựa chọn phụ bên dưới): nút mở form thêm nguồn, workspace chưa có nguồn nào thì form mở sẵn,
rồi với mỗi nguồn đã có: thử kết nối, danh sách mọi bảng đọc được tick sẵn, admin bỏ tick, một nút chụp vào dataset
của database (#247, #257). DatasetsPage chỉ hiện khung này cho admin; backend vẫn trả 403 cho member. Nối hook
(src/api/sources.ts) với component (ConnectForm, SourcePanel, SourceTablePicker). Bảng nào tick sẵn, câu báo khi bỏ
tick bảng danh mục và thân của lần chụp ở sourceTablesView.ts. */

import { useState } from 'react'
import {
  useCreateSource,
  useDeleteSource,
  useSnapshot,
  useSourceTables,
  useSources,
  useTestSource,
  type SnapshotResponse,
  type Source,
} from '@/api/sources'
import { ConnectForm, type ConnectValues } from '@/components/datasets/ConnectForm'
import { SourcePanel } from '@/components/datasets/SourcePanel'
import { SourceTablePicker } from '@/components/datasets/SourceTablePicker'
import { FormMessage } from '@/components/forms/FormMessage'
import { Button } from '@/components/ui/button'
import {
  buildSnapshotRequest,
  buildSnapshotSummary,
  buildSourceTableGroups,
  isFirstSnapshot,
  isTicked,
  type TickChoice,
} from './sourceTablesView'

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
        Mỗi database kết nối là một dataset. Mọi bảng tài khoản đọc được tick sẵn, bỏ tick bảng không cần rồi chụp một
        lần, kèm mọi quan hệ khóa ngoại giữa các bảng đã chụp. Mỗi lần chụp là một phiên bản, chụp lại khi dữ liệu bên đó
        đổi.
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

function SourceItem({ ws, source }: { ws: string; source: Source }) {
  const tables = useSourceTables(ws, source.id)
  const snapshot = useSnapshot(ws)
  const remove = useDeleteSource(ws)
  const testSource = useTestSource(ws)
  const [choice, setChoice] = useState<TickChoice>({})
  // undefined: chưa bấm gì, theo mặc định (mở khi nguồn chưa chụp bảng nào).
  const [open, setOpen] = useState<boolean>()
  const [message, setMessage] = useState<{ kind: 'error' | 'success'; text: string }>()
  const [result, setResult] = useState<SnapshotResponse>()

  const list = tables.data
  const first = list ? isFirstSnapshot(list) : true
  const groups = list ? buildSourceTableGroups(list, choice) : undefined
  const tickedCount = list ? list.filter(isTicked(list, choice)).length : 0

  const toggle = (table: string, ticked: boolean) => setChoice({ ...choice, [table]: ticked })
  const toggleSchema = (schema: string, ticked: boolean) => {
    const next = { ...choice }
    for (const t of list ?? []) if (t.schema === schema) next[t.table] = ticked
    setChoice(next)
  }

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
    if (!list) return
    setMessage(undefined)
    setResult(undefined)
    snapshot.mutate(
      { sourceId: source.id, ...buildSnapshotRequest(list, choice) },
      {
        onSuccess: (done) => {
          setResult(done)
          // Bảng vừa chụp thành bảng nguồn đang giữ; lựa chọn tay không cần giữ nữa.
          setChoice({})
          setOpen(false)
        },
        onError: (error) => setMessage({ kind: 'error', text: error.message }),
      },
    )
  }

  return (
    <SourcePanel
      label={sourceLabel(source)}
      tablesLoading={tables.isLoading}
      tablesError={tables.error?.message}
      picker={
        groups && (
          <SourceTablePicker
            groups={groups}
            tickedCount={tickedCount}
            onToggle={toggle}
            onToggleSchema={toggleSchema}
            onSnapshot={take}
            busy={snapshot.isPending}
          />
        )
      }
      pickerOpen={first && result === undefined ? undefined : (open ?? false)}
      onPickerOpenChange={setOpen}
      busy={snapshot.isPending || testSource.isPending || remove.isPending}
      snapshotting={snapshot.isPending ? tickedCount : undefined}
      message={message}
      result={result ? buildSnapshotSummary(result) : undefined}
      onTest={test}
      onDelete={() =>
        remove.mutate(source.id, { onError: (error) => setMessage({ kind: 'error', text: error.message }) })
      }
    />
  )
}
