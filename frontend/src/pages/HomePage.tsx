// SPDX-License-Identifier: Apache-2.0
/* pages/HomePage.tsx - trang chủ. Trang là nơi duy nhất nối hook gọi API với component hiển thị. */

import { useVersion } from '@/api/system'
import { VersionCard } from '@/components/VersionCard'

export function HomePage() {
  const version = useVersion()
  return (
    <main className="mx-auto max-w-3xl px-4 py-10">
      <h1 className="text-2xl font-semibold">xdi-platform</h1>
      <p className="mt-2 text-muted-foreground">
        Hỏi dữ liệu bằng tiếng Việt, nhận câu trả lời kèm căn cứ. Các tính năng đang được làm.
      </p>
      <div className="mt-6">
        <VersionCard info={version.data} error={version.error?.message} loading={version.isPending} />
      </div>
    </main>
  )
}
