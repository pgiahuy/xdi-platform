// SPDX-License-Identifier: Apache-2.0
/* pages/DatasetUpload.tsx - luồng nạp file của trang Dữ liệu: chọn file, với file Excel thì hỏi backend danh sách sheet
rồi cho chọn, gửi từng lần nạp và theo dõi kết quả. DatasetsPage chỉ hiện phần này cho admin. Các hàm thuần (dựng
lần gửi, lý do chưa nạp được, tên dataset) ở datasetUploadPlan.ts.

Một file Excel có nhiều nhóm sheet khác cột thành nhiều lần gửi, lần lượt từng lần: backend đọc mỗi lúc một file Excel,
gửi cùng lúc cũng phải chờ nhau, mà tiến trình gửi của các lần sau lại chạy trước khi tới lượt xử lý.
Rời trang (đổi workspace, sang trang khác) thì hủy lần gửi đang chạy và không gửi các nhóm còn lại: không còn chỗ nào
báo tiến trình hay kết quả cho người dùng. */

import { useEffect, useRef, useState, type Ref } from 'react'
import { isAborted } from '@/api/client'
import { useExcelSheets, useUploadDataset, type Dataset } from '@/api/datasets'
import { groupSheets, defaultSelection, type SheetGroup } from '@/components/datasets/sheetGroups'
import { SheetPicker } from '@/components/datasets/SheetPicker'
import type { UploadEntry, UploadStatus } from '@/components/datasets/types'
import { UploadForm } from '@/components/datasets/UploadForm'
import { UploadResults } from '@/components/datasets/UploadResults'
import {
  ACCEPT,
  blocker,
  buildRequests,
  MAX_DATASET_NAME,
  resultStatus,
  singleExcel,
  type PlannedRequest,
  type SheetsState,
} from './datasetUploadPlan'

type Props = {
  ws: string
  target?: Dataset
  /** Bỏ dataset đang nhận phiên bản mới. Có id thì chỉ bỏ khi dataset đang chọn vẫn là dataset đó. */
  onClearTarget: (id?: string) => void
  headingRef?: Ref<HTMLHeadingElement>
}

export function DatasetUpload({ ws, target, onClearTarget, headingRef }: Props) {
  const upload = useUploadDataset(ws)
  const listSheets = useExcelSheets(ws)
  const [files, setFiles] = useState<File[]>([])
  const [name, setName] = useState('')
  const [sheets, setSheets] = useState<SheetsState>({ status: 'idle' })
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [entries, setEntries] = useState<UploadEntry[]>([])
  const [busy, setBusy] = useState(false)
  const nextEntryId = useRef(0)
  // Lần gửi đã xong (tạo mới hoặc đã có) của lựa chọn hiện tại. Nạp lại sau lỗi thì chỉ gửi các lần chưa xong, không
  // gửi lại cả file cho các nhóm đã nạp.
  const completed = useRef(new Map<string, UploadEntry>())
  // Hủy lần đọc sheet đang chạy khi chọn file khác, và mọi lần gửi khi rời trang. Lần đọc sheet bị bỏ mà không hủy thì
  // vẫn gửi hết file và giữ lượt đọc Excel duy nhất của backend, lần đọc mới phải chờ.
  const sheetAbort = useRef<AbortController | null>(null)
  const uploadAbort = useRef<AbortController | null>(null)

  useEffect(
    () => () => {
      sheetAbort.current?.abort()
      uploadAbort.current?.abort()
    },
    [],
  )

  // Đóng tab hay tải lại trang giữa lúc nạp thì lần gửi bị cắt; trình duyệt hỏi lại trước khi rời.
  useEffect(() => {
    if (!busy) return
    const warn = (event: BeforeUnloadEvent) => event.preventDefault()
    window.addEventListener('beforeunload', warn)
    return () => window.removeEventListener('beforeunload', warn)
  }, [busy])

  const groups = sheets.status === 'ready' ? groupSheets(sheets.sheets, selected) : []

  const chooseFiles = (chosen: File[]) => {
    setFiles(chosen)
    setSelected(new Set())
    completed.current.clear()
    sheetAbort.current?.abort()
    sheetAbort.current = null
    const excel = singleExcel(chosen)
    if (!excel) {
      setSheets({ status: 'idle' })
      return
    }
    const controller = new AbortController()
    sheetAbort.current = controller
    setSheets({ status: 'loading', progress: 0 })
    listSheets
      .mutateAsync({
        file: excel,
        signal: controller.signal,
        onProgress: (sent) => setSheets({ status: 'loading', progress: sent }),
      })
      .then((result) => {
        if (controller.signal.aborted) return
        const options = result.map((s) => ({
          name: s.name,
          hidden: s.hidden,
          columns: s.columns,
          approxRows: s.approx_rows,
        }))
        setSheets({ status: 'ready', sheets: options })
        setSelected(defaultSelection(options))
      })
      .catch((error: Error) => {
        if (!controller.signal.aborted) setSheets({ status: 'error', message: error.message })
      })
  }

  const toggleSheet = (sheet: string) => {
    const next = new Set(selected)
    if (!next.delete(sheet)) next.add(sheet)
    setSelected(next)
  }

  const updateEntry = (id: number, status: UploadStatus) =>
    setEntries((current) => current.map((e) => (e.id === id ? { ...e, status } : e)))

  const submit = async () => {
    if (busy || blocker(files, sheets, groups, target) !== undefined) return
    const startTarget = target
    const controller = new AbortController()
    uploadAbort.current = controller
    const planned = buildRequests(files, groups, name.trim(), target).map((plan) => ({
      ...plan,
      key: requestKey(plan),
      id: nextEntryId.current++,
    }))
    setEntries(
      planned.map(({ key, id, label }) => completed.current.get(key) ?? { id, label, status: { kind: 'queued' } }),
    )
    setBusy(true)
    let failed = false
    for (const { key, id, label, request } of planned) {
      if (completed.current.has(key)) continue
      const onProgress = (sent: number) =>
        updateEntry(id, sent < 1 ? { kind: 'sending', progress: sent } : { kind: 'processing' })
      try {
        onProgress(0)
        const status = resultStatus(await upload.mutateAsync({ ...request, onProgress, signal: controller.signal }))
        completed.current.set(key, { id, label, status })
        updateEntry(id, status)
      } catch (error) {
        // Bị hủy vì rời trang: không còn giao diện để cập nhật, dừng luôn các nhóm còn lại.
        if (isAborted(error)) return
        failed = true
        updateEntry(id, { kind: 'error', message: error instanceof Error ? error.message : String(error) })
      }
    }
    setBusy(false)
    // Có lần lỗi thì giữ nguyên lựa chọn để người dùng sửa rồi nạp lại; xong hết thì dọn để nạp tiếp file khác.
    // Trong lúc nạp, admin có thể đã bấm nạp phiên bản cho dataset khác; chỉ bỏ dataset vừa nạp xong, không bỏ lựa
    // chọn mới.
    if (!failed) {
      chooseFiles([])
      setName('')
      if (startTarget) onClearTarget(startTarget.id)
    }
  }

  const excelFile = singleExcel(files)
  return (
    <div>
      <UploadForm
        target={target}
        onClearTarget={() => onClearTarget()}
        name={name}
        onNameChange={setName}
        maxNameLength={MAX_DATASET_NAME}
        files={files}
        onFilesChange={chooseFiles}
        accept={ACCEPT}
        status={excelFile && sheets.status === 'loading' ? sheetsLoadingText(sheets.progress) : undefined}
        sheetSection={
          excelFile && (
            <SheetSection state={sheets} selected={selected} groups={groups} onToggle={toggleSheet} disabled={busy} />
          )
        }
        blocker={blocker(files, sheets, groups, target)}
        busy={busy}
        onSubmit={submit}
        headingRef={headingRef}
      />
      <UploadResults entries={entries} />
    </div>
  )
}

/** Cùng khóa nghĩa là cùng một lần gửi: cùng file hay sheet, cùng tên, cùng dataset nhận phiên bản. */
function requestKey({ label, request }: PlannedRequest): string {
  return JSON.stringify([label, request.name, request.datasetId, request.sheets])
}

/** Dòng trạng thái lúc đọc danh sách sheet. Phần trăm để riêng (aria-hidden): vùng báo cho trình đọc màn hình chỉ đọc
khi đổi giai đoạn, không đọc từng phần trăm. */
function sheetsLoadingText(progress: number) {
  if (progress >= 1) return 'Đang đọc danh sách sheet...'
  return (
    <>
      Đang gửi file để đọc danh sách sheet
      <span aria-hidden="true">, {Math.round(progress * 100)}%</span>
    </>
  )
}

type SheetSectionProps = {
  state: SheetsState
  selected: Set<string>
  groups: SheetGroup[]
  onToggle: (sheet: string) => void
  disabled: boolean
}

function SheetSection({ state, selected, groups, onToggle, disabled }: SheetSectionProps) {
  switch (state.status) {
    case 'idle':
    case 'loading':
      return null
    case 'error':
      return (
        <p role="alert" className="text-sm break-words text-destructive">
          Không đọc được danh sách sheet: {state.message}
        </p>
      )
    case 'ready':
      return (
        <SheetPicker
          sheets={state.sheets}
          selected={selected}
          groups={groups}
          onToggle={onToggle}
          disabled={disabled}
        />
      )
  }
}
