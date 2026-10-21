// SPDX-License-Identifier: Apache-2.0
/* engine/store/users.rs - quản lý tài khoản: liệt kê, tạo, khóa, mở, đổi mật khẩu, xóa phiên.
Chỉ quản trị hệ thống gọi được các việc này; kiểm quyền nằm ở service/users.rs, file này chỉ đọc ghi. */

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use super::{db_error, Store, User};
use crate::error::AppError;

// Khác SETUP_LOCK trong accounts.rs. Xem deactivate_user.
const ADMIN_LOCK: i64 = 0x7864_6902;

/// Một dòng trong danh sách tài khoản cho quản trị hệ thống. Không có chuỗi băm mật khẩu.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct UserSummary {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub is_system_admin: bool,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

const SUMMARY_COLUMNS: &str = "id, email, display_name, is_system_admin, is_active, created_at";

/// Kết quả khóa tài khoản. Hai trường hợp từ chối là lỗi của người gọi, nên tách khỏi AppError để service tự chọn lời.
pub enum Deactivated {
    Done,
    NotFound,
    LastSystemAdmin,
}

impl Store {
    pub async fn list_users(&self) -> Result<Vec<UserSummary>, AppError> {
        let sql = format!("SELECT {SUMMARY_COLUMNS} FROM users ORDER BY created_at");
        sqlx::query_as(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| db_error("list users", e))
    }

    pub async fn list_system_admins(&self) -> Result<Vec<UserSummary>, AppError> {
        let sql = format!(
            "SELECT {SUMMARY_COLUMNS} FROM users WHERE is_system_admin ORDER BY created_at"
        );
        sqlx::query_as(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| db_error("list system admins", e))
    }

    pub async fn find_user_id_by_email(&self, email: &str) -> Result<Option<Uuid>, AppError> {
        sqlx::query_scalar("SELECT id FROM users WHERE lower(email) = lower($1)")
            .bind(email)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_error("find user by email", e))
    }

    /// Như find_user_id_by_email nhưng bỏ qua tài khoản đã khóa: không thêm người đã nghỉ vào workspace.
    pub async fn find_active_user_id_by_email(
        &self,
        email: &str,
    ) -> Result<Option<Uuid>, AppError> {
        sqlx::query_scalar("SELECT id FROM users WHERE lower(email) = lower($1) AND is_active")
            .bind(email)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_error("find active user by email", e))
    }

    /// Tài khoản thường (không phải quản trị hệ thống). Email trùng, không phân biệt hoa thường, thì báo lỗi rõ.
    pub async fn create_user(
        &self,
        email: &str,
        display_name: &str,
        password_hash: &str,
    ) -> Result<User, AppError> {
        sqlx::query_as(
            "INSERT INTO users (email, display_name, password_hash) VALUES ($1, $2, $3)
             RETURNING id, email, display_name, is_system_admin",
        )
        .bind(email)
        .bind(display_name)
        .bind(password_hash)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match &e {
            // 23505: vi phạm ràng buộc UNIQUE, ở đây là chỉ mục users_email_key.
            sqlx::Error::Database(db) if db.code().as_deref() == Some("23505") => {
                AppError::BadRequest("Email này đã có tài khoản".to_string())
            }
            _ => db_error("create user", e),
        })
    }

    /// Đổi mật khẩu và xóa mọi phiên của người đó, trừ phiên keep (phiên đang dùng khi người đó tự đổi mật khẩu).
    /// Hai việc trong cùng giao dịch: lỗi giữa chừng thì không có trạng thái mật khẩu đã đổi mà phiên cũ còn sống.
    ///
    /// expected: chuỗi băm người gọi vừa kiểm (tự đổi mật khẩu). Chỉ ghi khi mật khẩu trong database vẫn là chuỗi đó;
    /// nếu quản trị vừa đặt lại mật khẩu, UPDATE chờ giao dịch kia xong, đọc lại dòng mới, thấy khác và không ghi, nên
    /// mật khẩu tạm của quản trị không bị ghi đè. None: không kiểm (quản trị đặt lại).
    /// Trả false nếu không có tài khoản này hoặc mật khẩu đã đổi khác expected.
    pub async fn set_password_and_revoke(
        &self,
        user_id: Uuid,
        hash: &str,
        expected: Option<&str>,
        keep: Option<&str>,
    ) -> Result<bool, AppError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| db_error("begin set password", e))?;
        let updated = sqlx::query(
            "UPDATE users SET password_hash = $2
             WHERE id = $1 AND ($3::text IS NULL OR password_hash = $3)",
        )
        .bind(user_id)
        .bind(hash)
        .bind(expected)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_error("set password", e))?;
        if updated.rows_affected() == 0 {
            return Ok(false);
        }
        sqlx::query(
            "DELETE FROM sessions WHERE user_id = $1 AND ($2::text IS NULL OR token_hash <> $2)",
        )
        .bind(user_id)
        .bind(keep)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_error("revoke sessions", e))?;
        tx.commit()
            .await
            .map_err(|e| db_error("commit set password", e))?;
        Ok(true)
    }

    pub async fn password_hash_of(&self, user_id: Uuid) -> Result<Option<String>, AppError> {
        sqlx::query_scalar("SELECT password_hash FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map(Option::flatten)
            .map_err(|e| db_error("read password hash", e))
    }

    /// Khóa tài khoản và xóa mọi phiên của họ trong cùng giao dịch, nên khóa có hiệu lực ngay.
    /// Không khóa quản trị hệ thống cuối cùng còn mở: hai quản trị khóa lẫn nhau cùng lúc thì khóa tư vấn bắt một bên
    /// chờ, bên sau thấy chỉ còn một người và bị từ chối.
    pub async fn deactivate_user(&self, user_id: Uuid) -> Result<Deactivated, AppError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| db_error("begin deactivate", e))?;
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(ADMIN_LOCK)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_error("lock deactivate", e))?;
        let target: Option<bool> =
            sqlx::query_scalar("SELECT is_system_admin FROM users WHERE id = $1")
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| db_error("find user", e))?;
        let Some(is_system_admin) = target else {
            return Ok(Deactivated::NotFound);
        };
        if is_system_admin {
            let others: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM users WHERE is_system_admin AND is_active AND id <> $1",
            )
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| db_error("count system admins", e))?;
            if others == 0 {
                return Ok(Deactivated::LastSystemAdmin);
            }
        }
        sqlx::query("UPDATE users SET is_active = false WHERE id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_error("deactivate user", e))?;
        sqlx::query("DELETE FROM sessions WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_error("delete sessions", e))?;
        tx.commit()
            .await
            .map_err(|e| db_error("commit deactivate", e))?;
        Ok(Deactivated::Done)
    }

    /// Trả false nếu không có tài khoản này.
    pub async fn activate_user(&self, user_id: Uuid) -> Result<bool, AppError> {
        let result = sqlx::query("UPDATE users SET is_active = true WHERE id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(|e| db_error("activate user", e))?;
        Ok(result.rows_affected() == 1)
    }
}
