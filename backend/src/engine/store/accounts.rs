// SPDX-License-Identifier: Apache-2.0
/* engine/store/accounts.rs - tài khoản, phiên đăng nhập, và thành viên workspace của một người.

Như workspaces.rs, file này không nhận WorkspaceId: tài khoản và phiên dùng chung cho mọi workspace, và chính phiên là
thứ cho biết người dùng vào được workspace nào. Chuỗi băm mật khẩu chỉ đi ra qua Credentials, không nằm trong User,
để không lỡ trả nó về cho trình duyệt. */

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use super::{db_error, Store, WorkspaceKind};
use crate::error::AppError;

const USER_COLUMNS: &str = "id, email, display_name, is_system_admin";
const USER_COLUMNS_U: &str = "u.id, u.email, u.display_name, u.is_system_admin";

// Số bất kỳ, chỉ cần không trùng khóa tư vấn khác của ứng dụng. Xem create_first_admin.
const SETUP_LOCK: i64 = 0x7864_6901;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub is_system_admin: bool,
}

/// Dữ liệu để kiểm đăng nhập. Không derive Serialize: không bao giờ gửi ra ngoài.
#[derive(sqlx::FromRow)]
pub struct Credentials {
    pub id: Uuid,
    pub password_hash: Option<String>,
    pub is_active: bool,
}

// Debug viết tay để bỏ chuỗi băm: lỡ in struct này vào log cũng không lộ.
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("id", &self.id)
            .field("has_password", &self.password_hash.is_some())
            .field("is_active", &self.is_active)
            .finish()
    }
}

/// Một workspace người dùng thuộc về, kèm vai của họ ở đó.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Membership {
    pub workspace_id: Uuid,
    pub name: String,
    pub slug: String,
    pub role: String,
}

pub struct NewAdmin<'a> {
    pub email: &'a str,
    pub display_name: &'a str,
    pub password_hash: &'a str,
    pub workspace_name: &'a str,
    pub workspace_slug: &'a str,
}

impl Store {
    pub async fn has_any_user(&self) -> Result<bool, AppError> {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users)")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| db_error("check users", e))
    }

    /// Tạo tài khoản đầu tiên của bản cài: quản trị hệ thống, kèm một workspace mà người đó là admin.
    /// Trả None nếu đã có tài khoản. Hai yêu cầu đến cùng lúc thì khóa tư vấn bắt yêu cầu sau chờ yêu cầu trước xong;
    /// không có khóa, cả hai cùng thấy "chưa có ai" và cùng tạo được.
    pub async fn create_first_admin(&self, new: &NewAdmin<'_>) -> Result<Option<User>, AppError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| db_error("begin setup", e))?;
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(SETUP_LOCK)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_error("lock setup", e))?;
        let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users)")
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| db_error("check users", e))?;
        if exists {
            // tx bị hủy khi ra khỏi hàm, Postgres tự rollback và nhả khóa.
            return Ok(None);
        }

        let sql = format!(
            "INSERT INTO users (email, display_name, password_hash, is_system_admin)
             VALUES ($1, $2, $3, true) RETURNING {USER_COLUMNS}"
        );
        let user: User = sqlx::query_as(&sql)
            .bind(new.email)
            .bind(new.display_name)
            .bind(new.password_hash)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| db_error("create admin", e))?;

        // Bản cài có thể đã có workspace mẫu từ trước, nên slug có thể trùng: thêm hậu tố -2, -3 cho tới khi trống.
        let mut slug = new.workspace_slug.to_string();
        let mut n = 2;
        while sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM workspaces WHERE slug = $1)",
        )
        .bind(&slug)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| db_error("check slug", e))?
        {
            slug = format!("{}-{n}", new.workspace_slug);
            n += 1;
        }
        let workspace_id: Uuid = sqlx::query_scalar(
            "INSERT INTO workspaces (name, slug, kind) VALUES ($1, $2, $3) RETURNING id",
        )
        .bind(new.workspace_name)
        .bind(&slug)
        .bind(WorkspaceKind::Team.as_str())
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| db_error("create first workspace", e))?;
        sqlx::query(
            "INSERT INTO memberships (workspace_id, user_id, role) VALUES ($1, $2, 'admin')",
        )
        .bind(workspace_id)
        .bind(user.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_error("create first membership", e))?;

        tx.commit().await.map_err(|e| db_error("commit setup", e))?;
        Ok(Some(user))
    }

    /// Email so không phân biệt hoa thường, khớp với chỉ mục users_email_key.
    pub async fn find_credentials(&self, email: &str) -> Result<Option<Credentials>, AppError> {
        sqlx::query_as(
            "SELECT id, password_hash, is_active FROM users WHERE lower(email) = lower($1)",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| db_error("find credentials", e))
    }

    /// Lưu phiên mới; tiện thể xóa mọi phiên đã hết hạn để bảng không phình mãi, kể cả của người không quay lại.
    /// Trả false, không tạo phiên, nếu lúc này tài khoản đã bị khóa hoặc mật khẩu đã đổi khác password_hash.
    ///
    /// Vì sao kiểm lại ở đây: đăng nhập đọc chuỗi băm, kiểm mật khẩu mất khoảng 0,4 giây, rồi mới tạo phiên. Nếu đúng
    /// lúc đó quản trị khóa tài khoản hoặc đặt lại mật khẩu (xóa mọi phiên), thì phiên tạo sau sẽ còn sống. FOR SHARE
    /// khóa dòng users: giao dịch khóa hay đổi mật khẩu đang chạy thì câu này chờ nó xong rồi đọc lại dòng mới và không
    /// khớp; câu này chạy trước thì giao dịch kia chờ, rồi xóa luôn phiên vừa tạo.
    pub async fn create_session(
        &self,
        user_id: Uuid,
        token_hash: &str,
        expires_at: DateTime<Utc>,
        password_hash: &str,
    ) -> Result<bool, AppError> {
        sqlx::query("DELETE FROM sessions WHERE expires_at <= now()")
            .execute(&self.pool)
            .await
            .map_err(|e| db_error("clean sessions", e))?;
        let result = sqlx::query(
            "INSERT INTO sessions (token_hash, user_id, expires_at)
             SELECT $1, id, $3 FROM users
             WHERE id = $2 AND is_active AND password_hash = $4
             FOR SHARE",
        )
        .bind(token_hash)
        .bind(user_id)
        .bind(expires_at)
        .bind(password_hash)
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("create session", e))?;
        Ok(result.rows_affected() == 1)
    }

    /// Người dùng của phiên còn hạn. Tài khoản bị khóa thì như không có phiên.
    pub async fn find_session_user(&self, token_hash: &str) -> Result<Option<User>, AppError> {
        let sql = format!(
            "SELECT {USER_COLUMNS_U} FROM sessions s JOIN users u ON u.id = s.user_id
             WHERE s.token_hash = $1 AND s.expires_at > now() AND u.is_active"
        );
        sqlx::query_as(&sql)
            .bind(token_hash)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_error("find session", e))
    }

    pub async fn find_user(&self, id: Uuid) -> Result<Option<User>, AppError> {
        let sql = format!("SELECT {USER_COLUMNS} FROM users WHERE id = $1");
        sqlx::query_as(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_error("find user", e))
    }

    pub async fn delete_session(&self, token_hash: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
            .bind(token_hash)
            .execute(&self.pool)
            .await
            .map_err(|e| db_error("delete session", e))?;
        Ok(())
    }

    pub async fn list_memberships(&self, user_id: Uuid) -> Result<Vec<Membership>, AppError> {
        sqlx::query_as(
            "SELECT w.id AS workspace_id, w.name, w.slug, m.role
             FROM memberships m JOIN workspaces w ON w.id = m.workspace_id
             WHERE m.user_id = $1 ORDER BY w.name",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list memberships", e))
    }
}
