// SPDX-License-Identifier: Apache-2.0
/* components/admin/AccountList.tsx - mọi tài khoản của bản cài, cho quản trị hệ thống: khóa, mở, đặt lại mật khẩu.
Dòng của chính mình không có nút: backend không cho tự khóa, còn mật khẩu của mình thì đổi bằng mật khẩu hiện tại.
Khóa và đặt lại mật khẩu đều đăng xuất mọi phiên của người đó, nên hỏi lại trước.
Câu báo kết quả nhận focus khi hiện (ResultMessage): khóa xong thì nút Khóa đổi thành Mở khóa, focus không còn chỗ đứng.
Nút không bị disabled lúc chờ (trang chặn bấm trùng), cùng lý do. */

import { ConfirmDialog } from '@/components/ConfirmDialog'
import { ResultMessage } from '@/components/forms/ResultMessage'
import { Button } from '@/components/ui/button'
import type { AccountItem, RowMessage } from './types'

type Props = {
  accounts: AccountItem[]
  currentUserId: string
  busyId?: string
  message?: RowMessage
  onSetActive: (account: AccountItem, active: boolean) => void
  onResetPassword: (account: AccountItem) => void
}

export function AccountList({ accounts, currentUserId, busyId, message, ...props }: Props) {
  return (
    <ul className="divide-y rounded-xl border bg-card shadow-xs">
      {accounts.map((account) => {
        const name = account.name || account.email
        const busy = account.id === busyId
        const notes = [account.isSystemAdmin && 'Quản trị hệ thống', !account.isActive && 'Đã khóa'].filter(Boolean)
        return (
          <li key={account.id} className="flex flex-col gap-2 px-3 py-2.5">
            <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
              <div className="min-w-0 flex-1 basis-48">
                <p className="truncate text-sm font-medium" title={name}>
                  {name}
                  {account.id === currentUserId && <span className="font-normal text-muted-foreground"> (bạn)</span>}
                </p>
                <p className="truncate text-xs text-muted-foreground" title={account.email}>
                  {[account.name && account.email, ...notes].filter(Boolean).join(', ')}
                </p>
              </div>
              {account.id !== currentUserId && (
                <div className="flex flex-wrap items-center gap-2">
                  {account.isActive ? (
                    <ConfirmDialog
                      trigger={
                        <Button variant="outline" size="sm" aria-disabled={busy}>
                          Khóa
                        </Button>
                      }
                      title={`Khóa tài khoản ${name}?`}
                      description={
                        'Người này bị đăng xuất ngay và không đăng nhập được cho tới khi mở lại. ' +
                        'Vai trong các workspace vẫn giữ.'
                      }
                      confirmLabel="Khóa tài khoản"
                      onConfirm={() => props.onSetActive(account, false)}
                    />
                  ) : (
                    <Button
                      variant="outline"
                      size="sm"
                      aria-disabled={busy}
                      onClick={() => props.onSetActive(account, true)}
                    >
                      Mở khóa
                    </Button>
                  )}
                  <ConfirmDialog
                    trigger={
                      <Button variant="outline" size="sm" aria-disabled={busy}>
                        Đặt lại mật khẩu
                      </Button>
                    }
                    title={`Đặt lại mật khẩu cho ${name}?`}
                    description={
                      'Mật khẩu cũ hết dùng được, người này bị đăng xuất. ' +
                      'Mật khẩu tạm hiện một lần để bạn chuyển cho họ.'
                    }
                    confirmLabel="Đặt lại mật khẩu"
                    onConfirm={() => props.onResetPassword(account)}
                  />
                </div>
              )}
            </div>
            {message?.id === account.id && (
              <ResultMessage key={message.seq} kind={message.kind}>
                {message.text}
              </ResultMessage>
            )}
          </li>
        )
      })}
    </ul>
  )
}
