// SPDX-License-Identifier: Apache-2.0
/* api/reports.rs - báo cáo của người gọi: danh sách, tạo, sửa tiêu đề và đoạn mở đầu, xóa, thêm và bỏ mục, đổi thứ tự,
mở (mọi mục dựng lại từ câu hỏi đã lưu), xuất Excel, và tạo báo cáo tự động cho một dataset. Logic ở service/report/.
Chỉ người tạo đọc và sửa được một báo cáo; báo cáo của người khác trả 404. Test ở api/report_tests.rs. */

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use utoipa_axum::{router::OpenApiRouter, routes};

use super::extract::{JsonBody, PathParams};
use crate::auth::Member;
use crate::error::ErrorBody;
use crate::service::report::contract::{
    AddReportItemRequest, AutoReportRequest, AutoReportResponse, CreateReportRequest,
    ReorderReportRequest, ReportListResponse, ReportResponse, ReportSummary, UpdateReportRequest,
};
use crate::{error::AppError, service, state::AppState};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_reports, create_report))
        .routes(routes!(create_automatic_report))
        .routes(routes!(get_report, update_report, delete_report))
        .routes(routes!(add_report_item))
        .routes(routes!(remove_report_item))
        .routes(routes!(reorder_report_items))
        .routes(routes!(export_report_xlsx))
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/reports",
    tag = "reports",
    operation_id = "listReports",
    summary = "Reports of the caller",
    description = "Last changed first, without items.",
    params(("ws" = String, Path, description = "Workspace slug")),
    responses(
        (status = 200, body = ReportListResponse),
        (status = "4XX", body = ErrorBody, description = "401; 404 not a member")
    )
)]
async fn list_reports(
    State(state): State<AppState>,
    member: Member,
    PathParams(_ws): PathParams<String>,
) -> Result<Json<ReportListResponse>, AppError> {
    Ok(Json(service::report::list(&state, &member).await?))
}

#[utoipa::path(
    post,
    path = "/workspaces/{ws}/reports",
    tag = "reports",
    operation_id = "createReport",
    summary = "Create an empty report",
    description = "Add answers with addReportItem. Every member can create reports; only the creator reads them.",
    params(("ws" = String, Path, description = "Workspace slug")),
    request_body = CreateReportRequest,
    responses(
        (status = 200, body = ReportSummary),
        (status = "4XX", body = ErrorBody, description = "400 empty title, title over 200 or intro over 5000 \
                                                         characters; 401; 404 not a member")
    )
)]
async fn create_report(
    State(state): State<AppState>,
    member: Member,
    PathParams(_ws): PathParams<String>,
    JsonBody(request): JsonBody<CreateReportRequest>,
) -> Result<Json<ReportSummary>, AppError> {
    Ok(Json(
        service::report::create(&state, &member, &request).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/workspaces/{ws}/reports/automatic",
    tag = "reports",
    operation_id = "createAutomaticReport",
    summary = "Build an automatic report for one dataset",
    description = "The backend builds a fixed set of analyses with its own SQL (no SQL from the language model): the \
                   main measure over time with the strongest change and its breakdown, then the measure by two \
                   dimensions with the top groups, concentration and groups far from the average. Every query goes \
                   through the guard with the caller's row filters. The language model only rewords the computed \
                   facts, with the same two checks as an answer; `report.model_calls` counts its calls. One JSON \
                   response, no streaming.",
    params(("ws" = String, Path, description = "Workspace slug")),
    request_body = AutoReportRequest,
    responses(
        (status = 200, body = AutoReportResponse),
        (status = "4XX", body = ErrorBody, description = "400 the dataset has no measure, or no section could be \
                                                         built; 401; 404 not a member or no such dataset")
    )
)]
async fn create_automatic_report(
    State(state): State<AppState>,
    member: Member,
    PathParams(_ws): PathParams<String>,
    JsonBody(request): JsonBody<AutoReportRequest>,
) -> Result<Json<AutoReportResponse>, AppError> {
    Ok(Json(
        service::report::create_automatic(&state, &member, &request).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/reports/{id}",
    tag = "reports",
    operation_id = "getReport",
    summary = "Open a report with every item rendered again",
    description = "Result rows are not stored: the stored SQL of every item runs again through the guard with the \
                   caller's access, on the data versions the question read. An item whose SQL cannot run again has \
                   `error` and is still listed.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Report id (UUID)")
    ),
    responses(
        (status = 200, body = ReportResponse),
        (status = "4XX", body = ErrorBody, description = "401; 404 not a member, or no such report of the caller")
    )
)]
async fn get_report(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
) -> Result<Json<ReportResponse>, AppError> {
    Ok(Json(service::report::open(&state, &member, &id).await?))
}

#[utoipa::path(
    patch,
    path = "/workspaces/{ws}/reports/{id}",
    tag = "reports",
    operation_id = "updateReport",
    summary = "Change the title or the intro of a report",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Report id (UUID)")
    ),
    request_body = UpdateReportRequest,
    responses(
        (status = 200, body = ReportSummary),
        (status = "4XX", body = ErrorBody, description = "400 empty or long title, long intro; 401; 404 not a \
                                                         member, or no such report of the caller")
    )
)]
async fn update_report(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
    JsonBody(request): JsonBody<UpdateReportRequest>,
) -> Result<Json<ReportSummary>, AppError> {
    Ok(Json(
        service::report::update(&state, &member, &id, &request).await?,
    ))
}

#[utoipa::path(
    delete,
    path = "/workspaces/{ws}/reports/{id}",
    tag = "reports",
    operation_id = "deleteReport",
    summary = "Delete a report",
    description = "The questions of its items stay in the history.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Report id (UUID)")
    ),
    responses(
        (status = 204, description = "Deleted"),
        (status = "4XX", body = ErrorBody, description = "401; 404 not a member, or no such report of the caller")
    )
)]
async fn delete_report(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
) -> Result<StatusCode, AppError> {
    service::report::delete(&state, &member, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/workspaces/{ws}/reports/{id}/items",
    tag = "reports",
    operation_id = "addReportItem",
    summary = "Add an answered question of the caller at the end of a report",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Report id (UUID)")
    ),
    request_body = AddReportItemRequest,
    responses(
        (status = 200, body = ReportSummary),
        (status = "4XX", body = ErrorBody, description = "400 the question was not answered, is already in the \
                                                         report, or the report has 30 items; 401; 404 not a member, \
                                                         no such report of the caller or no such question of the \
                                                         caller")
    )
)]
async fn add_report_item(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
    JsonBody(request): JsonBody<AddReportItemRequest>,
) -> Result<Json<ReportSummary>, AppError> {
    Ok(Json(
        service::report::add_item(&state, &member, &id, request.query_id).await?,
    ))
}

#[utoipa::path(
    delete,
    path = "/workspaces/{ws}/reports/{id}/items/{item_id}",
    tag = "reports",
    operation_id = "removeReportItem",
    summary = "Remove an item from a report",
    description = "The question stays in the history.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Report id (UUID)"),
        ("item_id" = String, Path, description = "Item id (`ReportItem.id`, UUID)")
    ),
    responses(
        (status = 204, description = "Removed"),
        (status = "4XX", body = ErrorBody, description = "401; 404 not a member, no such report of the caller or no \
                                                         such item")
    )
)]
async fn remove_report_item(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id, item_id)): PathParams<(String, String, String)>,
) -> Result<StatusCode, AppError> {
    service::report::remove_item(&state, &member, &id, &item_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    put,
    path = "/workspaces/{ws}/reports/{id}/order",
    tag = "reports",
    operation_id = "reorderReportItems",
    summary = "Set the order of the items",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Report id (UUID)")
    ),
    request_body = ReorderReportRequest,
    responses(
        (status = 204, description = "Order saved"),
        (status = "4XX", body = ErrorBody, description = "400 the list is not exactly every item of the report; \
                                                         401; 404 not a member, or no such report of the caller")
    )
)]
async fn reorder_report_items(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
    JsonBody(request): JsonBody<ReorderReportRequest>,
) -> Result<StatusCode, AppError> {
    service::report::reorder(&state, &member, &id, &request.item_ids).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/reports/{id}/xlsx",
    tag = "reports",
    operation_id = "exportReportXlsx",
    summary = "Download a report as an Excel file",
    description = "Built from the same rendering as getReport. Sheets: a summary (one row per item with its answer \
                   and checks), one sheet per item with its result table and breakdown, and `Cách tính` (for every \
                   item: measure, filters, grouping, period, SQL that ran, tables and versions, time asked).",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Report id (UUID)")
    ),
    responses(
        (status = 200, description = "The workbook",
            content_type = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        (status = "4XX", body = ErrorBody, description = "401; 404 not a member, or no such report of the caller")
    )
)]
async fn export_report_xlsx(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let (bytes, name) = service::report::export_xlsx(&state, &member, &id).await?;
    let headers = [
        (
            header::CONTENT_TYPE,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(),
        ),
        (
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{name}\""),
        ),
    ];
    Ok((headers, bytes))
}
