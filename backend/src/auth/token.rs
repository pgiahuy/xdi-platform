// SPDX-License-Identifier: Apache-2.0
/* auth/token.rs - mã ngẫu nhiên: mã phiên trong cookie và mã thiết lập in ra log lần chạy đầu.

Mã phiên gửi cho trình duyệt, còn database chỉ giữ SHA-256 của nó (bảng sessions). Mã có 256 bit ngẫu nhiên nên băm
nhanh một lần là đủ, không cần thuật toán chậm như mật khẩu. */

use sha2::{Digest, Sha256};

use crate::error::AppError;

/// Mã phiên mới: 32 byte ngẫu nhiên từ hệ điều hành, viết dạng hex (64 ký tự).
pub fn new_session_token() -> Result<String, AppError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| AppError::Internal(format!("random bytes: {e}")))?;
    Ok(to_hex(&bytes))
}

/// Giá trị lưu trong sessions.token_hash.
pub fn hash_session_token(token: &str) -> String {
    to_hex(&Sha256::digest(token.as_bytes()))
}

/// Mã thiết lập người cài đọc từ log rồi gõ vào màn hình. Bỏ các ký tự dễ nhầm (0 và O, 1 và I, L).
pub fn new_setup_code() -> Result<String, AppError> {
    const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
    let mut bytes = [0u8; 12];
    getrandom::fill(&mut bytes).map_err(|e| AppError::Internal(format!("random bytes: {e}")))?;
    // Lấy phần dư làm lệch phân bố một chút (256 không chia hết cho 31); với mã dùng một lần lúc cài thì chấp nhận.
    let code: String = bytes
        .iter()
        .map(|b| ALPHABET[usize::from(*b) % ALPHABET.len()] as char)
        .collect();
    Ok(format!("{}-{}-{}", &code[0..4], &code[4..8], &code[8..12]))
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_token_is_64_hex_and_unique() {
        let a = new_session_token().unwrap();
        let b = new_session_token().unwrap();
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn hash_is_stable_and_differs_from_token() {
        let token = new_session_token().unwrap();
        assert_eq!(hash_session_token(&token), hash_session_token(&token));
        assert_ne!(hash_session_token(&token), token);
        assert_eq!(hash_session_token(&token).len(), 64);
    }

    #[test]
    fn setup_code_shape() {
        let code = new_setup_code().unwrap();
        assert_eq!(code.len(), 14, "got {code}");
        assert!(code.split('-').all(|part| part.len() == 4));
        assert!(!code.contains(['0', 'O', '1', 'I', 'L']), "got {code}");
    }
}
