// SPDX-License-Identifier: Apache-2.0
/* api/query.rs - hỏi dữ liệu (ask) và truy dòng nguồn (drill). Toàn bộ luồng chín bước nằm ở service/ask.rs; kiểu
response của ask ở service/ask/contract.rs, mẫu ở service/ask/examples.rs. Truy dòng nguồn ở service/drill.rs, hợp đồng
ở service/drill/contract.rs.
Route nằm dưới /workspaces/{ws}/ và nhận Member: chỉ thành viên workspace hỏi được dữ liệu của nó.

Ask trả JSON một lần, không SSE (#33). Câu không trả lời được (guard từ chối, câu mơ hồ, SQL chạy lỗi, kể cả mô hình
lỗi ở vòng sửa) vẫn là 200 với status trong body; mã lỗi chỉ dành cho request sai, quyền, và dịch vụ mô hình lỗi trước
khi có SQL. Lý do ở docs/ARCHITECTURE.md mục 8. */

use axum::{extract::State, Json};
use serde::Deserialize;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

use super::extract::{JsonBody, PathParams};
use crate::auth::Member;
use crate::error::ErrorBody;
use crate::service::ask::contract::{AskResponse, ChosenOption};
use crate::service::ask::examples::{
    build_failed_example, build_needs_choice_example, build_rejected_example, build_simple_example,
    build_why_example,
};
use crate::service::drill::contract::{DrillRequest, DrillResponse};
use crate::{error::AppError, service, state::AppState};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(ask))
        .routes(routes!(drill))
}

#[derive(Deserialize, ToSchema)]
pub struct AskRequest {
    pub question: String,
    // Lần hỏi đầu không có lựa chọn; đổi giả định hay trả lời câu hỏi lại thì gửi lại cùng câu hỏi kèm lựa chọn. Câu
    // đã sửa chữ là câu mới: lựa chọn cũ có thể không còn hợp, giữ lại thì áp dụng ngầm hoặc sinh cảnh báo khó hiểu.
    /// Choices for the question, empty the first time. To change an assumption or answer a clarification, send the
    /// same question with `choices` of the previous answer, the changed entry replaced or added. A question whose text
    /// was edited is a new question: send `choices` empty. The backend does not compare with earlier questions; it
    /// applies the choices it recognizes for this question and ignores the others with a warning
    /// `choice_not_applied`. Choices not listed use their default.
    #[serde(default)]
    pub choices: Vec<ChosenOption>,
}

#[utoipa::path(
    post,
    path = "/workspaces/{ws}/datasets/{id}/ask",
    tag = "query",
    operation_id = "ask",
    summary = "Ask a question about a dataset",
    description = "One JSON response, no streaming. A question that cannot be answered (`rejected`, `needs_choice`, \
                   `failed`) is still 200, with `status` and `message` in the body and the evidence gathered so far. \
                   Error codes are for invalid requests, access, and the language model service.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Dataset id (UUID)")
    ),
    request_body = AskRequest,
    responses(
        (status = 200, body = AskResponse, description = "Answer, or why there is none",
            examples(
                ("why" = (summary = "Why question with a breakdown", value = json!(build_why_example()))),
                ("simple" = (summary = "One dimension, one measure", value = json!(build_simple_example()))),
                ("needs_choice" = (summary = "Ambiguous question", value = json!(build_needs_choice_example()))),
                ("rejected" = (summary = "SQL refused by the guard", value = json!(build_rejected_example()))),
                ("failed" = (summary = "SQL failed, also after one fix round", value = json!(build_failed_example())))
            )
        ),
        (status = "4XX", body = ErrorBody, description = "400 empty question, invalid JSON body, dataset with no \
                                                         data or whose profile is not ready, or a question the mock \
                                                         model has no answer for; 401; 404 not a member or unknown \
                                                         dataset"),
        (status = 502, body = ErrorBody, description = "The language model service failed or did not answer before \
                                                       any SQL was written"),
        (status = 503, body = ErrorBody, description = "The language model is not configured on the server")
    )
)]
async fn ask(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
    JsonBody(req): JsonBody<AskRequest>,
) -> Result<Json<AskResponse>, AppError> {
    let response = service::ask::ask(&state, &member, &id, &req.question, &req.choices).await?;
    Ok(Json(response))
}

// Request chỉ có query_id và khóa của chỗ được bấm, không bao giờ có SQL (docs/ARCHITECTURE.md mục 8): backend dựng câu
// từ câu đã lưu, qua guard với quyền của người đang bấm.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/datasets/{id}/drill",
    tag = "query",
    operation_id = "drill",
    summary = "Source rows behind a number of an answer",
    description = "Reads the stored question, keeps its table and filters, adds the keys of the clicked cell, and runs \
                   that on the dataset version the question used, through the guard with the row filters of the \
                   member calling now. One page of rows per call, in the order the analytics engine reads them.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Dataset id (UUID)")
    ),
    request_body = DrillRequest,
    responses(
        (status = 200, body = DrillResponse, description = "One page of source rows and their total count"),
        (status = "4XX", body = ErrorBody, description = "400 invalid body, keys that do not match the result, a \
                                                         question that was rejected or failed, a question whose \
                                                         form cannot be traced yet (the message says why), a \
                                                         breakdown row of a question without that breakdown, or a \
                                                         deleted version; \
                                                         401; 404 not a member, or no such question in this dataset")
    )
)]
async fn drill(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
    JsonBody(req): JsonBody<DrillRequest>,
) -> Result<Json<DrillResponse>, AppError> {
    let response = service::drill::drill(&state, &member, &id, &req).await?;
    Ok(Json(response))
}
