// SPDX-License-Identifier: Apache-2.0
/* components/domains/DatasetPacks.tsx - gói ngành của từng dataset, đầu màn Khái niệm ngành: mỗi dataset một dòng, tên
và gói đang dùng. onChange chỉ truyền vào khi người xem là admin: khi đó gói là ô chọn, có cả "Không dùng gói"; member
chỉ đọc tên gói. Gói chọn xong dùng ngay ở câu hỏi kế tiếp trên dataset đó. */

import { NativeSelect } from '@/components/metrics/NativeSelect'
import type { DatasetPackItem, PackItem } from './types'

type Props = {
  datasets: DatasetPackItem[]
  packs: Pick<PackItem, 'key' | 'name'>[]
  onChange?: (datasetId: string, pack: string | null) => void
  /** Id của dataset đang lưu gói, để khóa ô chọn của nó. */
  savingId?: string
}

const noPack = 'Không dùng gói'

/** Tên gói theo khóa; khóa không còn trong workspace (gói đã bỏ khỏi data/domains/) thì hiện nguyên khóa. */
function packName(packs: Props['packs'], key: string | null): string {
  if (key === null) return noPack
  return packs.find((p) => p.key === key)?.name ?? key
}

export function DatasetPacks({ datasets, packs, onChange, savingId }: Props) {
  const options = [{ value: '', label: noPack }, ...packs.map((p) => ({ value: p.key, label: p.name }))]
  return (
    <section aria-labelledby="dataset-packs" className="flex flex-col gap-3">
      <h3 id="dataset-packs" className="text-[15px] font-semibold tracking-tight">
        Gói ngành của từng dataset
      </h3>
      <p className="text-sm text-muted-foreground">
        Câu hỏi trên một dataset nhận các khái niệm đang bật của gói chọn cho dataset đó.
      </p>
      {datasets.length === 0 ? (
        <p className="text-sm text-muted-foreground">Workspace chưa có dataset nào.</p>
      ) : (
        <ul className="divide-y rounded-xl border bg-card shadow-xs">
          {datasets.map((dataset) => {
            const selectId = `dataset-pack-${dataset.id}`
            // Gói đã bỏ khỏi workspace vẫn hiện trong ô chọn, để ô không lặng lẽ nhảy về "Không dùng gói".
            const known = dataset.pack === null || packs.some((p) => p.key === dataset.pack)
            const rowOptions = known ? options : [...options, { value: dataset.pack ?? '', label: dataset.pack ?? '' }]
            return (
              <li key={dataset.id} className="flex flex-wrap items-center justify-between gap-2 px-4 py-3 text-sm">
                <label htmlFor={onChange ? selectId : undefined} className="min-w-0 font-medium break-words">
                  {dataset.name}
                </label>
                {onChange ? (
                  <NativeSelect
                    id={selectId}
                    options={rowOptions}
                    value={dataset.pack ?? ''}
                    disabled={savingId === dataset.id}
                    onChange={(e) => onChange(dataset.id, e.target.value || null)}
                  />
                ) : (
                  <span className="text-muted-foreground">{packName(packs, dataset.pack)}</span>
                )}
              </li>
            )
          })}
        </ul>
      )}
    </section>
  )
}
