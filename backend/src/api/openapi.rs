// SPDX-License-Identifier: Apache-2.0
/* api/openapi.rs - phần chung của đặc tả OpenAPI: tên API, địa chỉ gốc /api, cách đăng nhập (cookie phiên), lỗi 500.
Đường dẫn, tham số, request và response của từng endpoint khai báo ngay trên handler bằng #[utoipa::path]; router
(api/mod.rs) gom handler bằng routes! của utoipa_axum, nên route nào có trong router thì có trong đặc tả, không quên
khai báo được. Phần chung thêm vào sau khi gom (apply_common), vì lúc dựng ApiDoc chưa có đường dẫn nào.

Đặc tả là hợp đồng giữa backend và frontend: openapi.json ở gốc repo sinh từ đây (xdi-backend openapi, make openapi),
kiểu TypeScript của frontend sinh từ file đó. Test bên dưới báo lỗi khi openapi.json lệch code.

Mô tả trong đặc tả viết tiếng Anh vì người đọc là người dùng API (AGENTS.md mục 1). utoipa lấy chú thích /// làm mô tả,
nên chú thích tiếng Việt cho đội trên struct và handler của API dùng //; test no_vietnamese_in_spec nhắc khi quên. */

use utoipa::openapi::path::Operation;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityRequirement, SecurityScheme};
use utoipa::openapi::{OpenApi as Spec, Ref, ResponseBuilder};
use utoipa::OpenApi;

use super::auth::SESSION_COOKIE;

/// Tên security scheme của cookie phiên trong đặc tả.
pub const SESSION: &str = "session";

// Thao tác không chạm database hay file thì không có lỗi 500 để khai: health, version, setupStatus đọc từ bộ nhớ,
// demoLogin chưa làm. Mọi thao tác khác có thể trả 500 (ví dụ mất kết nối Postgres).
const NO_INTERNAL_ERROR: [&str; 4] = ["health", "version", "setupStatus", "demoLogin"];

#[derive(OpenApi)]
#[openapi(
    info(
        title = "xdi-platform API",
        description = "Natural-language data analysis with evidence for every number. Errors always have the body \
                       {code, message}: code is a stable English identifier, message is Vietnamese text for the end \
                       user."
    ),
    servers((url = "/api"))
)]
pub struct ApiDoc;

/// Thêm phần chung vào đặc tả đã gom từ router: cookie phiên là cách đăng nhập mặc định cho mọi thao tác (thao tác
/// công khai tự ghi security(()) trên handler), và response 500 cho thao tác có thể lỗi nội bộ.
pub fn apply_common(spec: &mut Spec) {
    let components = spec.components.get_or_insert_with(Default::default);
    components.add_security_scheme(
        SESSION,
        SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
            SESSION_COOKIE,
            "Session cookie (httpOnly) set by /auth/setup and /auth/login.",
        ))),
    );
    spec.security = Some(vec![SecurityRequirement::new(
        SESSION,
        Vec::<String>::new(),
    )]);

    for item in spec.paths.paths.values_mut() {
        let operations = [
            &mut item.get,
            &mut item.post,
            &mut item.put,
            &mut item.patch,
            &mut item.delete,
        ];
        for operation in operations.into_iter().flatten() {
            apply_internal_error(operation);
        }
    }
}

fn apply_internal_error(operation: &mut Operation) {
    let id = operation.operation_id.as_deref().unwrap_or_default();
    if NO_INTERNAL_ERROR.contains(&id) {
        return;
    }
    let response = ResponseBuilder::new()
        .description("Internal error; details are in the server log, not in the response")
        .content(
            "application/json",
            utoipa::openapi::ContentBuilder::new()
                .schema(Some(Ref::from_schema_name("ErrorBody")))
                .build(),
        )
        .build();
    operation
        .responses
        .responses
        .insert("500".to_string(), response.into());
}

/// Đặc tả dạng JSON, thụt lề, kết thúc bằng xuống dòng: đúng nội dung của openapi.json ở gốc repo.
pub fn build_spec_json(spec: &Spec) -> String {
    let mut json = spec.to_pretty_json().expect("serialize OpenAPI spec");
    json.push('\n');
    json
}

#[cfg(test)]
mod tests {
    /// openapi.json ở gốc repo phải khớp đặc tả sinh từ code. Lệch tức là đổi API mà quên chạy make openapi: frontend
    /// sẽ dùng kiểu cũ. Test chạy trong make check-fast, CI và hook pre-push.
    #[test]
    fn openapi_json_is_up_to_date() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../openapi.json");
        let committed = std::fs::read_to_string(path).expect("read openapi.json at the repo root");
        let generated = crate::api::build_openapi_json();
        assert!(
            committed == generated,
            "openapi.json is out of date with the code: run `make openapi` and commit openapi.json and \
             frontend/src/api/schema.ts"
        );
    }

    /// Chú thích /// trên struct và handler API thành mô tả trong đặc tả. Chữ có dấu tiếng Việt lọt vào đặc tả nghĩa là
    /// có chú thích cho đội viết bằng ///; make openapi vẫn khớp nên test lệch ở trên không bắt được.
    /// Bỏ qua example: response mẫu chứa chữ cho người dùng cuối (message, nhãn), vốn là tiếng Việt (AGENTS.md mục 1).
    #[test]
    fn no_vietnamese_in_spec() {
        let mut spec: serde_json::Value =
            serde_json::from_str(&crate::api::build_openapi_json()).expect("parse spec");
        remove_examples(&mut spec);
        let json = spec.to_string();
        // Chữ cái riêng của tiếng Việt: đ, ă, ơ, ư và nguyên âm mang dấu (dải Latin Extended Additional,
        // U+1EA0..U+1EF9).
        let vietnamese = |c: char| "đĐăĂơƠưƯ".contains(c) || ('\u{1EA0}'..='\u{1EF9}').contains(&c);
        let found: String = json.chars().filter(|c| vietnamese(*c)).take(20).collect();
        assert!(
            found.is_empty(),
            "Vietnamese text in the OpenAPI spec ({found}): use // for team comments on API structs and handlers, \
             /// only for English API descriptions"
        );
    }

    // Không schema nào có trường tên example hay examples, nên xóa theo tên khóa ở mọi tầng không xóa nhầm mô tả.
    fn remove_examples(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                map.remove("example");
                map.remove("examples");
                map.values_mut().for_each(remove_examples);
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(remove_examples),
            _ => {}
        }
    }
}
