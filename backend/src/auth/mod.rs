// SPDX-License-Identifier: Apache-2.0
/* auth/mod.rs - danh tính và quyền: băm mật khẩu (password.rs), mã phiên và mã thiết lập (token.rs), kiểu WorkspaceId.
Phiên đăng nhập lưu trong Postgres (engine/store/accounts.rs), không dùng JWT, để thu hồi được ngay.

WorkspaceId là lớp chặn thứ nhất giữ dữ liệu các workspace tách nhau (docs/ARCHITECTURE.md mục 4b): mọi hàm của
engine/store nhận nó, nên một truy vấn không thể quên lọc theo workspace. Trường bên trong để private và chỉ module
auth tạo được giá trị, nên service hay engine không thể dựng một WorkspaceId từ dữ liệu tùy ý (ví dụ từ URL). */

pub mod password;
pub mod token;

use uuid::Uuid;

use crate::engine::store::{Membership, User};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspaceId(Uuid);

impl WorkspaceId {
    /// Đọc giá trị để truyền vào câu SQL. Không có hàm ngược lại ngoài module auth.
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    /// Chỉ dùng trong test.
    #[cfg(test)]
    pub fn for_test(id: Uuid) -> Self {
        Self(id)
    }
}

/// Người đã đăng nhập và là thành viên của workspace trong đường dẫn (/api/workspaces/{ws}/...). Handler nhận kiểu
/// này thì không cần kiểm lại: tạo được Member nghĩa là phiên còn hạn và dòng memberships có thật.
#[derive(Debug, Clone)]
pub struct Member {
    // Khung: kiểm vai theo từng việc (#44) đọc role và user. Gỡ allow khi có code đọc tới.
    #[allow(dead_code)]
    pub user: User,
    pub workspace: WorkspaceId,
    #[allow(dead_code)]
    pub slug: String,
    #[allow(dead_code)]
    pub role: String,
}

impl Member {
    /// Cách duy nhất ngoài test để có WorkspaceId: từ một dòng memberships vừa đọc cho đúng người dùng của phiên
    /// (service/workspaces.rs). Membership đi kèm user là bằng chứng người đó thuộc workspace.
    pub fn from_membership(user: User, membership: Membership) -> Self {
        Self {
            user,
            workspace: WorkspaceId(membership.workspace_id),
            slug: membership.slug,
            role: membership.role,
        }
    }
}
