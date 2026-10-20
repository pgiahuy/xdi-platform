// SPDX-License-Identifier: Apache-2.0
/* error.rs - một enum lỗi chung cho cả crate, và cách đổi lỗi thành phản hồi HTTP.
Mọi hàm ở service và engine trả Result<_, AppError>; handler chỉ việc dùng dấu ?.
Body lỗi luôn có dạng {"code": "...", "message": "..."}: code tiếng Anh cho máy, message tiếng Việt cho người. */

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug)]
pub enum AppError {
    /// Tính năng có trong thiết kế nhưng chưa cài đặt. Tham số là tên tính năng.
    NotImplemented(&'static str),
    BadRequest(String),
    NotFound(String),
    // Khung: dùng khi có Postgres và DuckDB. Gỡ allow khi có code tạo ra biến thể này.
    #[allow(dead_code)]
    Internal(String),
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

impl AppError {
    fn status_and_body(&self) -> (StatusCode, ErrorBody) {
        match self {
            AppError::NotImplemented(feature) => (
                StatusCode::NOT_IMPLEMENTED,
                ErrorBody {
                    code: "not_implemented",
                    message: format!("Chưa cài đặt: {feature}"),
                },
            ),
            AppError::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                ErrorBody {
                    code: "bad_request",
                    message: msg.clone(),
                },
            ),
            AppError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                ErrorBody {
                    code: "not_found",
                    message: msg.clone(),
                },
            ),
            AppError::Internal(msg) => {
                // Ghi chi tiết vào log, không trả chi tiết nội bộ cho client.
                tracing::error!(error = %msg, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorBody {
                        code: "internal",
                        message: "Lỗi nội bộ, xem log của backend".to_string(),
                    },
                )
            }
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, body) = self.status_and_body();
        (status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_implemented_maps_to_501() {
        let (status, body) = AppError::NotImplemented("ask").status_and_body();
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert_eq!(body.code, "not_implemented");
    }

    #[test]
    fn internal_hides_details() {
        let (status, body) = AppError::Internal("db password is wrong".into()).status_and_body();
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!body.message.contains("password"));
    }
}
