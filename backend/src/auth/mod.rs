// SPDX-License-Identifier: Apache-2.0
/* auth/mod.rs - danh tính và quyền: băm mật khẩu (password.rs), mã phiên và mã thiết lập (token.rs), kiểu WorkspaceId.
Phiên đăng nhập lưu trong Postgres (engine/store/accounts.rs), không dùng JWT, để thu hồi được ngay.

WorkspaceId là lớp chặn thứ nhất giữ dữ liệu các workspace tách nhau (docs/ARCHITECTURE.md mục 4b): mọi hàm của
engine/store nhận nó, nên một truy vấn không thể quên lọc theo workspace. Trường bên trong để private và chỉ module
auth tạo được giá trị, nên service hay engine không thể dựng một WorkspaceId từ dữ liệu tùy ý (ví dụ từ URL). */

pub mod password;
pub mod token;

use uuid::Uuid;

use crate::engine::store::{Membership, Store, User};
use crate::error::AppError;

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

/// Kiểm người dùng có thuộc workspace slug không, và nếu có thì trả Member. Đây là cách duy nhất ngoài test để có
/// WorkspaceId: hàm tự đọc dòng memberships cho đúng user, nên không ai dựng được WorkspaceId từ một UUID tùy ý.
/// Không có workspace và không phải thành viên trả cùng một lỗi 404, để người ngoài không dò được tên workspace.
pub async fn member_of(store: &Store, user: User, slug: &str) -> Result<Member, AppError> {
    let membership = store
        .find_membership(user.id, slug)
        .await?
        .ok_or_else(workspace_not_found)?;
    Ok(member_from(user, membership))
}

pub fn workspace_not_found() -> AppError {
    AppError::NotFound("Không có workspace này hoặc bạn không thuộc workspace này".to_string())
}

// Riêng tư: chỉ member_of gọi, sau khi chính nó đọc dòng memberships.
fn member_from(user: User, membership: Membership) -> Member {
    Member {
        user,
        workspace: WorkspaceId(membership.workspace_id),
        slug: membership.slug,
        role: membership.role,
    }
}
