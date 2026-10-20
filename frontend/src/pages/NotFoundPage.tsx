// SPDX-License-Identifier: Apache-2.0
/* pages/NotFoundPage.tsx - đường dẫn không có trang nào. */

import { Link } from 'react-router'

export function NotFoundPage() {
  return (
    <main className="mx-auto max-w-3xl px-4 py-10">
      <h1 className="text-2xl font-semibold">Không có trang này</h1>
      <Link to="/" className="mt-4 inline-block underline">
        Về trang chủ
      </Link>
    </main>
  )
}
