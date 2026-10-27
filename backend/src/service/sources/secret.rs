// SPDX-License-Identifier: Apache-2.0
/* service/sources/secret.rs - mã hóa và giải mã thông tin kết nối của nguồn database (cột sources.secret_encrypted).

AES-256-GCM, khóa 32 byte đọc từ SOURCE_SECRET_KEY dạng base64. Mỗi lần mã hóa một nonce ngẫu nhiên 12 byte, ghi
liền trước bản mã: nonce || bản mã có thẻ xác thực. Id workspace là dữ liệu kèm (associated data): bản mã chép sang
workspace khác thì giải mã lỗi, nên không ai dời một kết nối sang workspace mình bằng cách sửa database. Đổi khóa thì
mọi nguồn cũ không giải mã được nữa, admin phải thêm lại nguồn. */

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use uuid::Uuid;

use crate::error::AppError;

const NONCE_LEN: usize = 12;

/// Khóa đã kiểm độ dài. Không derive Debug để khóa không lọt vào log.
pub struct SecretKey(Key<Aes256Gcm>);

impl SecretKey {
    /// Đọc khóa base64. Rỗng nghĩa là tính năng tắt; sai dạng cũng tắt, kèm lời báo cách tạo khóa.
    pub fn from_base64(raw: &str) -> Result<Self, AppError> {
        if raw.trim().is_empty() {
            return Err(AppError::Unavailable(
                "Tính năng nối database đang tắt: đặt SOURCE_SECRET_KEY (32 byte dạng base64, tạo bằng \
                 openssl rand -base64 32) rồi khởi động lại backend"
                    .to_string(),
            ));
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(raw.trim())
            .ok()
            .filter(|b| b.len() == 32)
            .ok_or_else(|| {
                AppError::Unavailable(
                    "SOURCE_SECRET_KEY phải là đúng 32 byte dạng base64 (tạo bằng openssl rand -base64 32)"
                        .to_string(),
                )
            })?;
        Ok(Self(*Key::<Aes256Gcm>::from_slice(&bytes)))
    }

    pub fn encrypt(&self, workspace: Uuid, plain: &[u8]) -> Result<Vec<u8>, AppError> {
        let cipher = Aes256Gcm::new(&self.0);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let sealed = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plain,
                    aad: workspace.as_bytes(),
                },
            )
            .map_err(|_| AppError::Internal("encrypt source secret".to_string()))?;
        let mut out = nonce.to_vec();
        out.extend_from_slice(&sealed);
        Ok(out)
    }

    pub fn decrypt(&self, workspace: Uuid, sealed: &[u8]) -> Result<Vec<u8>, AppError> {
        let undecryptable = || {
            AppError::BadRequest(
                "Không giải mã được thông tin kết nối của nguồn này (SOURCE_SECRET_KEY đã đổi?): xóa nguồn rồi thêm lại"
                    .to_string(),
            )
        };
        if sealed.len() <= NONCE_LEN {
            return Err(undecryptable());
        }
        let (nonce, body) = sealed.split_at(NONCE_LEN);
        Aes256Gcm::new(&self.0)
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: body,
                    aad: workspace.as_bytes(),
                },
            )
            .map_err(|_| undecryptable())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=";

    #[test]
    fn round_trip_and_wrong_workspace() {
        let key = SecretKey::from_base64(KEY).unwrap();
        let ws = Uuid::new_v4();
        let sealed = key.encrypt(ws, b"password=s3cret").unwrap();
        assert!(!sealed.windows(6).any(|w| w == b"s3cret"));
        assert_eq!(key.decrypt(ws, &sealed).unwrap(), b"password=s3cret");
        // Hai lần mã hóa cùng nội dung ra hai bản mã khác nhau (nonce ngẫu nhiên).
        assert_ne!(key.encrypt(ws, b"password=s3cret").unwrap(), sealed);
        assert!(key.decrypt(Uuid::new_v4(), &sealed).is_err());
        let other = SecretKey::from_base64("ZmVkY2JhOTg3NjU0MzIxMGZlZGNiYTk4NzY1NDMyMTA=").unwrap();
        assert!(other.decrypt(ws, &sealed).is_err());
        assert!(key.decrypt(ws, b"short").is_err());
    }

    #[test]
    fn rejects_missing_or_short_key() {
        assert!(matches!(
            SecretKey::from_base64(""),
            Err(AppError::Unavailable(_))
        ));
        assert!(matches!(
            SecretKey::from_base64("c2hvcnQ="),
            Err(AppError::Unavailable(_))
        ));
    }
}
