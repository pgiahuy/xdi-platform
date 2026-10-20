// SPDX-License-Identifier: Apache-2.0
/* auth/mod.rs - danh tính và quyền. Hiện có kiểu WorkspaceId; JWT, vai, đăng nhập thêm ở các issue auth.

WorkspaceId là lớp chặn thứ nhất giữ dữ liệu các workspace tách nhau (docs/ARCHITECTURE.md mục 4b): mọi hàm của
engine/store nhận nó, nên một truy vấn không thể quên lọc theo workspace. Trường bên trong để private và chỉ module
auth tạo được giá trị, nên service hay engine không thể dựng một WorkspaceId từ dữ liệu tùy ý (ví dụ từ URL). */

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspaceId(Uuid);

impl WorkspaceId {
    /// Đọc giá trị để truyền vào câu SQL. Không có hàm ngược lại ngoài module auth.
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    /// Chỉ dùng trong test. Hàm tạo thật đến từ phiên đăng nhập (issue chế độ xác thực).
    #[cfg(test)]
    pub fn for_test(id: Uuid) -> Self {
        Self(id)
    }
}
