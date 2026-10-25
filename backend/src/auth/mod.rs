// SPDX-License-Identifier: Apache-2.0
/* auth/mod.rs - danh tính và quyền: băm mật khẩu (password.rs), mã phiên và mã thiết lập (token.rs), kiểu WorkspaceId.
Phiên đăng nhập lưu trong Postgres (engine/store/accounts.rs), không dùng JWT, để thu hồi được ngay.

WorkspaceId là lớp chặn thứ nhất giữ dữ liệu các workspace tách nhau (docs/ARCHITECTURE.md mục 4b): mọi hàm của
engine/store nhận nó, nên một truy vấn không thể quên lọc theo workspace. Trường bên trong để private và chỉ module
auth tạo được giá trị, nên service hay engine không thể dựng một WorkspaceId từ dữ liệu tùy ý (ví dụ từ URL). */

pub mod password;
pub mod token;

use uuid::Uuid;

use crate::engine::store::{Membership, PendingVersion, SemanticCandidate, Store, User};
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
    pub user: User,
    pub workspace: WorkspaceId,
    pub slug: String,
    pub role: Role,
}

/// Vai trong một workspace. Bảng quyền đầy đủ ở docs/ARCHITECTURE.md mục 4b.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Quản lý workspace: dữ liệu, chỉ số, thành viên, đổi tên, xóa.
    Admin,
    /// Xem dữ liệu, hỏi, lưu câu hỏi.
    Member,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Member => "member",
        }
    }

    /// Đọc vai từ chuỗi (database hoặc request). Không nhận được thì None.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "admin" => Some(Role::Admin),
            "member" => Some(Role::Member),
            _ => None,
        }
    }
}

impl Member {
    /// Việc chỉ admin của workspace làm được. Thành viên thường nhận 403: họ biết workspace có thật, chỉ thiếu quyền.
    pub fn require_admin(&self) -> Result<(), AppError> {
        if self.role == Role::Admin {
            Ok(())
        } else {
            Err(AppError::Forbidden(
                "Chỉ admin của workspace làm được việc này".to_string(),
            ))
        }
    }
}

/// Kiểm người dùng có thuộc workspace slug không, và nếu có thì trả Member. Đây là cách duy nhất ngoài test để có
/// WorkspaceId cho một request (cách còn lại là pending_profiles, cho việc nền lúc khởi động): hàm tự đọc dòng
/// memberships cho đúng user, nên không ai dựng được WorkspaceId từ một UUID tùy ý.
/// Không có workspace và không phải thành viên trả cùng một lỗi 404, để người ngoài không dò được tên workspace.
pub async fn member_of(store: &Store, user: User, slug: &str) -> Result<Member, AppError> {
    let membership = store
        .find_membership(user.id, slug)
        .await?
        .ok_or_else(workspace_not_found)?;
    Ok(member_from(user, membership))
}

/// Phiên bản dataset còn chờ lập hồ sơ ở mọi workspace, kèm WorkspaceId của từng phiên bản. Dùng cho việc nền lúc
/// khởi động (service/profile.rs), không gắn với người dùng nào. Như member_of, WorkspaceId dựng từ dòng
/// dataset_versions mà chính hàm này đọc, không từ dữ liệu người gọi đưa vào.
pub async fn pending_profiles(
    store: &Store,
) -> Result<Vec<(WorkspaceId, PendingVersion)>, AppError> {
    let versions = store.list_pending_versions().await?;
    Ok(versions
        .into_iter()
        .map(|v| (WorkspaceId(v.workspace_id), v))
        .collect())
}

/// Như pending_profiles nhưng cho lớp ngữ nghĩa: phiên bản ở structure chưa từng thử, kèm WorkspaceId dựng từ chính
/// dòng đọc được. Việc nền lúc khởi động (service/semantic.rs) dùng, không gắn với người dùng nào.
pub async fn pending_semantic_profiles(
    store: &Store,
) -> Result<Vec<(WorkspaceId, SemanticCandidate)>, AppError> {
    let versions = store.list_semantic_candidates().await?;
    Ok(versions
        .into_iter()
        .map(|v| (WorkspaceId(v.workspace_id), v))
        .collect())
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
        // Database chỉ cho hai giá trị (migration 0003); giá trị lạ thì cho quyền thấp nhất.
        role: Role::parse(&membership.role).unwrap_or(Role::Member),
    }
}
