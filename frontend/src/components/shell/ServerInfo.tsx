// SPDX-License-Identifier: Apache-2.0
/* components/shell/ServerInfo.tsx - phiên bản máy chủ và mô hình ngôn ngữ đang dùng, ở chân thanh bên.
Người dùng cần biết câu hỏi của mình gửi tới mô hình nào (docs/PRIVACY.md); phiên bản giúp ghi báo lỗi cho đúng. */

import type { ServerInfo as ServerInfoData } from './types'

export function ServerInfo({ info }: { info: ServerInfoData }) {
  return (
    <dl className="grid grid-cols-[auto_1fr] gap-x-2 gap-y-0.5 text-xs text-muted-foreground">
      <dt>Phiên bản</dt>
      <dd className="truncate">
        {info.name} {info.version}
      </dd>
      <dt>Mô hình</dt>
      <dd className="truncate">{info.llm}</dd>
    </dl>
  )
}
