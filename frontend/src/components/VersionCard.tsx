// SPDX-License-Identifier: Apache-2.0
/* components/VersionCard.tsx - hiện phiên bản backend và mô hình ngôn ngữ đang dùng.
Chỉ nhận dữ liệu qua props, không gọi API, không dùng router: dùng lại được ở bất kỳ trang nào. */

import type { VersionInfo } from '@/api/system'

type Props = {
  info?: VersionInfo
  error?: string
  loading: boolean
}

export function VersionCard({ info, error, loading }: Props) {
  return (
    <section className="rounded-lg border bg-card p-4 text-card-foreground">
      <h2 className="text-sm font-medium text-muted-foreground">Máy chủ</h2>
      {loading && <p className="mt-1">Đang kiểm tra...</p>}
      {error && <p className="mt-1 text-destructive">{error}</p>}
      {info && (
        <dl className="mt-2 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
          <dt className="text-muted-foreground">Phiên bản</dt>
          <dd>
            {info.name} {info.version}
          </dd>
          <dt className="text-muted-foreground">Mô hình ngôn ngữ</dt>
          <dd>{info.llm}</dd>
        </dl>
      )}
    </section>
  )
}
