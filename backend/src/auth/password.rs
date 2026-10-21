// SPDX-License-Identifier: Apache-2.0
/* auth/password.rs - băm và kiểm mật khẩu bằng Argon2id, tham số mặc định của crate argon2.

Hai hàm đều tốn CPU có chủ đích (chậm để đoán mật khẩu cũng chậm), khoảng vài chục mili giây. Gọi từ code async thì
bọc trong tokio::task::spawn_blocking, nếu không một lần đăng nhập sẽ chặn luồng đang phục vụ các request khác. */

use argon2::{
    password_hash::{phc::PasswordHash, PasswordHasher, PasswordVerifier},
    Argon2,
};

use crate::error::AppError;

/// Độ dài tối thiểu, đếm theo ký tự (không theo byte, để mật khẩu tiếng Việt có dấu không bị tính dài hơn thực tế).
pub const MIN_LENGTH: usize = 8;

/// Kiểm mật khẩu mới trước khi băm. Thông báo trả thẳng cho người dùng.
pub fn validate(password: &str) -> Result<(), AppError> {
    if password.chars().count() < MIN_LENGTH {
        return Err(AppError::BadRequest(format!(
            "Mật khẩu cần ít nhất {MIN_LENGTH} ký tự"
        )));
    }
    Ok(())
}

/// Trả chuỗi PHC ($argon2id$v=19$...): chứa cả thuật toán, tham số và salt ngẫu nhiên, nên lưu một cột là đủ.
pub fn hash(password: &str) -> Result<String, AppError> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("hash password: {e}")))
}

/// Sai mật khẩu và chuỗi băm hỏng đều trả false: người gọi chỉ cần biết đăng nhập được hay không.
pub fn verify(password: &str, stored_hash: &str) -> bool {
    match PasswordHash::new(stored_hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(e) => {
            tracing::error!(error = %e, "stored password hash is not valid PHC");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_then_verify() {
        let stored = hash("correct horse").unwrap();
        assert!(stored.starts_with("$argon2id$"), "got {stored}");
        assert!(verify("correct horse", &stored));
        assert!(!verify("wrong horse", &stored));
    }

    #[test]
    fn same_password_gets_different_salt() {
        assert_ne!(
            hash("same password").unwrap(),
            hash("same password").unwrap()
        );
    }

    #[test]
    fn broken_hash_does_not_verify() {
        assert!(!verify("anything", "not a phc string"));
    }

    #[test]
    fn length_counts_characters_not_bytes() {
        // 7 ký tự có dấu là 7 ký tự, dù chiếm hơn 8 byte.
        assert!(validate("mậtkhẩu").is_err());
        assert!(validate("mậtkhẩu1").is_ok());
    }
}
