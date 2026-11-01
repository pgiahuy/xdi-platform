// SPDX-License-Identifier: Apache-2.0
/* pages/DomainsPage.tsx - màn Khái niệm ngành của workspace (/w/:ws/domains): mỗi gói ngành một mục, bảng khái niệm,
form thêm và sửa, nút tắt và bật; đầu màn là gói của từng dataset. Mọi thành viên xem; nút thêm, sửa, tắt và ô chọn gói
của dataset chỉ hiện với admin, quyền thật do backend kiểm. Khái niệm lưu xong dùng ngay ở câu hỏi kế tiếp trên dataset
gắn gói đó. */

import { useMemo, useState } from 'react'
import { useParams } from 'react-router'
import { useMe } from '@/api/auth'
import { type CatalogDataset, useCatalog, useSetDatasetDomain } from '@/api/catalog'
import { ApiError } from '@/api/client'
import { type Concept, type ConceptRequest, type ConceptsResponse, useConcepts, useSaveConcept } from '@/api/domains'
import { EmptyState } from '@/components/EmptyState'
import { ConceptForm } from '@/components/domains/ConceptForm'
import { ConceptTable } from '@/components/domains/ConceptTable'
import { DatasetPacks } from '@/components/domains/DatasetPacks'
import type { ConceptDraft, ConceptField, ConceptItem, DatasetPackItem, PackItem } from '@/components/domains/types'
import type { FormError } from '@/components/forms/formError'
import { FormMessage } from '@/components/forms/FormMessage'
import { Button } from '@/components/ui/button'

const conceptFields: ConceptField[] = ['name', 'description', 'synonyms', 'calculations', 'dimensions', 'sanity_checks']

function toItem(c: Concept): ConceptItem {
  return {
    id: c.id,
    key: c.key,
    name: c.name,
    description: c.description,
    synonyms: c.synonyms,
    calculations: c.calculations,
    dimensions: c.dimensions,
    sanityChecks: c.sanity_checks,
    enabled: c.enabled,
    updatedAt: c.updated_at,
  }
}

function toPacks(data: ConceptsResponse): PackItem[] {
  return data.packs.map((p) => ({ key: p.key, name: p.name, description: p.description, concepts: p.concepts.map(toItem) }))
}

function toDatasetPacks(datasets: CatalogDataset[]): DatasetPackItem[] {
  return datasets.map((d) => ({ id: d.id, name: d.name, pack: d.domain }))
}

/** "a, b, , c" thành ["a", "b", "c"]; backend vẫn bỏ khoảng trắng và kiểm lại. */
function splitList(text: string): string[] {
  return text
    .split(',')
    .map((part) => part.trim())
    .filter(Boolean)
}

function toRequest(draft: ConceptDraft, editing?: ConceptItem): ConceptRequest {
  return {
    name: draft.name,
    description: draft.description,
    synonyms: splitList(draft.synonyms),
    dimensions: splitList(draft.dimensions),
    // Hàng cách tính để trống cả hai ô là hàng thêm rồi bỏ đó, không gửi.
    calculations: draft.calculations.filter((c) => c.name.trim() || c.description.trim()),
    sanity_checks: editing?.sanityChecks ?? [],
    enabled: editing?.enabled ?? true,
  }
}

function itemRequest(item: ConceptItem, enabled: boolean): ConceptRequest {
  return {
    name: item.name,
    description: item.description,
    synonyms: item.synonyms,
    calculations: item.calculations,
    dimensions: item.dimensions,
    sanity_checks: item.sanityChecks,
    enabled,
  }
}

export function DomainsPage() {
  const { ws = '' } = useParams()
  // Đổi workspace bằng thanh bên thì route giữ nguyên trang này; gắn lại để form đang soạn không đi sang workspace khác.
  return <WorkspaceConcepts key={ws} ws={ws} />
}

function WorkspaceConcepts({ ws }: { ws: string }) {
  const me = useMe()
  const concepts = useConcepts(ws)
  const save = useSaveConcept(ws)
  const toggle = useSaveConcept(ws)
  const catalog = useCatalog(ws)
  const setPack = useSetDatasetDomain(ws)
  // Form đang mở: pack là gói của form, editing là khái niệm đang sửa (không có là thêm mới); key tăng mỗi lần mở để
  // form đọc lại giá trị đầu, kể cả khi mở sửa khái niệm khác lúc form đang mở.
  const [editor, setEditor] = useState<{ key: number; pack: string; editing?: ConceptItem }>()
  const isAdmin = me.data?.workspaces.find((m) => m.slug === ws)?.role === 'admin'

  const formError = useMemo<FormError<ConceptField> | undefined>(() => {
    const error = save.error
    if (!error) return undefined
    const field = error instanceof ApiError ? conceptFields.find((f) => f === error.field) : undefined
    return { message: error.message, field }
  }, [save.error])

  const open = (pack: string, editing?: ConceptItem) => {
    save.reset()
    setEditor((current) => ({ key: (current?.key ?? 0) + 1, pack, editing }))
  }
  const submit = (draft: ConceptDraft) => {
    if (!editor) return
    const request = toRequest(draft, editor.editing)
    save.mutate({ pack: editor.pack, id: editor.editing?.id, request }, { onSuccess: () => setEditor(undefined) })
  }
  const flip = (pack: string, item: ConceptItem) =>
    toggle.mutate({ pack, id: item.id, request: itemRequest(item, !item.enabled) })

  if (concepts.error && !concepts.data) {
    return <EmptyState title="Không tải được khái niệm ngành" description={concepts.error.message} />
  }
  if (!concepts.data) return <p className="text-sm text-muted-foreground">Đang tải...</p>
  const packs = toPacks(concepts.data)

  return (
    <div className="flex flex-col gap-8">
      <div className="flex flex-col gap-1">
        <h2 className="text-lg font-semibold tracking-tight">Khái niệm ngành</h2>
        <p className="text-sm text-muted-foreground">
          Thuật ngữ quen dùng của từng ngành, gửi kèm câu hỏi số liệu và câu hỏi nguyên nhân trên dataset gắn gói đó để mô
          hình hiểu đúng chữ. Công thức trên cột thật nằm ở màn Chỉ số của từng dataset.
        </p>
      </div>
      {catalog.error && <FormMessage kind="error">{`Không tải được danh sách dataset: ${catalog.error.message}`}</FormMessage>}
      {catalog.data && (
        <DatasetPacks
          datasets={toDatasetPacks(catalog.data)}
          packs={packs}
          onChange={isAdmin ? (id, domain) => setPack.mutate({ id, domain }) : undefined}
          savingId={setPack.isPending ? setPack.variables?.id : undefined}
        />
      )}
      {setPack.error && <FormMessage kind="error">{setPack.error.message}</FormMessage>}
      {toggle.error && <FormMessage kind="error">{toggle.error.message}</FormMessage>}
      {packs.length === 0 && <EmptyState title="Chưa có gói ngành nào" />}
      {packs.map((pack) => (
        <section key={pack.key} aria-labelledby={`pack-${pack.key}`} className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <h3 id={`pack-${pack.key}`} className="text-[15px] font-semibold tracking-tight">
              {pack.name}
            </h3>
            {isAdmin && !editor && <Button onClick={() => open(pack.key)}>Thêm khái niệm</Button>}
          </div>
          <p className="text-sm text-muted-foreground">{pack.description}</p>
          {editor?.pack === pack.key && (
            <ConceptForm
              key={editor.key}
              packName={pack.name}
              editing={editor.editing}
              pending={save.isPending}
              error={formError}
              onSubmit={submit}
              onCancel={() => setEditor(undefined)}
            />
          )}
          <ConceptTable
            concepts={pack.concepts}
            onEdit={isAdmin ? (c) => open(pack.key, c) : undefined}
            onToggle={isAdmin ? (c) => flip(pack.key, c) : undefined}
            togglingId={toggle.isPending ? toggle.variables?.id : undefined}
          />
        </section>
      ))}
    </div>
  )
}
