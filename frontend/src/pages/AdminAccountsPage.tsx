// SPDX-License-Identifier: Apache-2.0
/* pages/AdminAccountsPage.tsx - trang Quản lý tài khoản cho quản trị hệ thống: tạo tài khoản, khóa, mở, đặt lại mật
khẩu. Trang nằm ngoài workspace (globalNav.ts), nên quản trị hệ thống chưa thuộc workspace nào vẫn mở được. Mục điều
hướng chỉ hiện với quản trị hệ thống; người khác gõ thẳng đường dẫn thì thấy một câu báo và trang không gọi API
(backend vẫn trả 403 nếu gọi). */

import { useMemo, useState } from "react";
import { passwordLength, useMe } from "@/api/auth";
import { ApiError } from "@/api/client";
import {
  useCreateUser,
  useResetPassword,
  useSetUserActive,
  useUsers,
} from "@/api/admin";
import { AccountList } from "@/components/admin/AccountList";
import {
  type CreateAccountField,
  CreateAccountForm,
  type CreateAccountValues,
} from "@/components/admin/CreateAccountForm";
import { TemporaryPassword } from "@/components/admin/TemporaryPassword";
import { SystemAdminOnly } from "@/components/admin/SystemAdminOnly";
import type { AccountItem, RowMessage } from "@/components/admin/types";
import { EmptyState } from "@/components/EmptyState";
import type { FormError } from "@/components/forms/formError";
import { Button } from "@/components/ui/button";

export function AdminAccountsPage() {
  const me = useMe();
  const isSystemAdmin = me.data?.user.is_system_admin === true;
  const users = useUsers(isSystemAdmin);
  const createUser = useCreateUser();
  const setActive = useSetUserActive();
  const resetPassword = useResetPassword();
  // Mỗi lần tạo xong thì tăng, để CreateAccountForm gắn lại và xóa các ô.
  const [createFormKey, setCreateFormKey] = useState(0);
  const [createdMessage, setCreatedMessage] = useState<string>();
  const [rowMessage, setRowMessage] = useState<RowMessage>();
  // Độ dài mật khẩu đã kiểm trước khi gửi, nên 400 còn lại là lỗi email (sai dạng, đã có tài khoản): báo ở ô email.
  const createError = useMemo<FormError<CreateAccountField> | undefined>(() => {
    const error = createUser.error;
    if (!error) return undefined;
    const emailProblem = error instanceof ApiError && error.status === 400;
    return {
      message: error.message,
      field: emailProblem ? "email" : undefined,
    };
  }, [createUser.error]);

  if (me.data && !isSystemAdmin) return <SystemAdminOnly />;
  if (users.error) {
    return (
      <EmptyState
        title="Không tải được danh sách tài khoản"
        description={users.error.message}
        action={
          <Button variant="outline" onClick={() => users.refetch()}>
            Thử lại
          </Button>
        }
      />
    );
  }
  if (!users.data || !me.data) {
    return (
      <p aria-busy="true" className="text-sm text-muted-foreground">
        Đang tải...
      </p>
    );
  }

  const rowPending = setActive.isPending || resetPassword.isPending;
  const showRowMessage = (id: string, kind: RowMessage["kind"], text: string) =>
    setRowMessage({ id, seq: Date.now(), kind, text });

  const handleCreate = (values: CreateAccountValues) => {
    setCreatedMessage(undefined);
    createUser.mutate(
      {
        email: values.email,
        display_name: values.displayName,
        password: values.password,
      },
      {
        onSuccess: (user) => {
          setCreatedMessage(
            `Đã tạo tài khoản ${user.email}. Thêm họ vào workspace ở trang Thành viên của workspace đó.`,
          );
          setCreateFormKey((key) => key + 1);
          // Bỏ mật khẩu ban đầu khỏi variables của mutation; câu báo đã lấy email ở trên.
          createUser.reset();
        },
      },
    );
  };
  const handleSetActive = (account: AccountItem, active: boolean) => {
    if (rowPending) return;
    setRowMessage(undefined);
    setActive.mutate(
      { userId: account.id, active },
      {
        onSuccess: () =>
          showRowMessage(
            account.id,
            "success",
            active ? "Đã mở khóa." : "Đã khóa, người này bị đăng xuất.",
          ),
        onError: (error) => showRowMessage(account.id, "error", error.message),
      },
    );
  };
  const handleResetPassword = (account: AccountItem) => {
    if (rowPending) return;
    setRowMessage(undefined);
    resetPassword.mutate(account.id, {
      onError: (error) => showRowMessage(account.id, "error", error.message),
    });
  };

  const accounts: AccountItem[] = users.data.map((u) => ({
    id: u.id,
    name: u.display_name,
    email: u.email,
    isSystemAdmin: u.is_system_admin,
    isActive: u.is_active,
  }));
  const busyId = setActive.isPending
    ? setActive.variables?.userId
    : resetPassword.isPending
      ? resetPassword.variables
      : undefined;
  // Mật khẩu tạm đọc thẳng từ kết quả mutation, không chép ra state: "Ẩn mật khẩu" gọi reset() là bỏ khỏi bộ đệm
  // của TanStack Query.
  const temporary = resetPassword.data;
  const temporaryFor = accounts.find((a) => a.id === resetPassword.variables);

  return (
    // Màn rộng (xl) hai cột: danh sách tài khoản bên trái, form tạo tài khoản bên phải; trang bung hết bề ngang.
    <div className="grid items-start gap-10 xl:grid-cols-2">
      <section
        aria-labelledby="accounts-heading"
        className="flex flex-col gap-3"
      >
        <h2
          id="accounts-heading"
          className="text-[15px] font-semibold tracking-tight"
        >
          Tài khoản ({accounts.length})
        </h2>
        {temporary && temporaryFor && (
          <TemporaryPassword
            email={temporaryFor.email}
            password={temporary.temporary_password}
            onDismiss={() => resetPassword.reset()}
          />
        )}
        <AccountList
          accounts={accounts}
          currentUserId={me.data.user.id}
          busyId={busyId}
          message={rowMessage}
          onSetActive={handleSetActive}
          onResetPassword={handleResetPassword}
        />
      </section>

      <section
        aria-labelledby="create-account-heading"
        className="flex flex-col gap-3 rounded-xl border bg-card p-4 shadow-xs sm:px-6 sm:py-5"
      >
        <h2
          id="create-account-heading"
          className="text-[15px] font-semibold tracking-tight"
        >
          Tạo tài khoản
        </h2>
        <CreateAccountForm
          key={createFormKey}
          onSubmit={handleCreate}
          pending={createUser.isPending}
          error={createError}
          success={createdMessage}
          passwordMin={passwordLength.min}
          passwordMax={passwordLength.max}
        />
      </section>
    </div>
  );
}
