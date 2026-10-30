// SPDX-License-Identifier: Apache-2.0
/* api/metrics.rs - chỉ số của một dataset và cách gộp mặc định của cột số đo (#185): mọi thành viên xem, admin tạo,
sửa (thêm phiên bản), xóa, đổi cách gộp. Logic và kiểm chỉ số ở service/metrics/. Lỗi của một trường trả 400 kèm
`field` trong body. Test ở api/metrics_tests.rs. */

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use utoipa_axum::{router::OpenApiRouter, routes};

use super::extract::{JsonBody, PathParams};
use crate::auth::Member;
use crate::error::ErrorBody;
use crate::service::metrics::contract::{
    ColumnAggregationRequest, Metric, MetricRequest, MetricsResponse,
};
use crate::{error::AppError, service, state::AppState};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_metrics, create_metric))
        .routes(routes!(update_metric, delete_metric))
        .routes(routes!(set_column_aggregation))
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/datasets/{id}/metrics",
    tag = "metrics",
    operation_id = "listMetrics",
    summary = "Metrics of a dataset and the columns of its current version",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Dataset id (UUID)")
    ),
    responses(
        (status = 200, body = MetricsResponse),
        (status = "4XX", body = ErrorBody, description = "401; 404 not a member or no such dataset")
    )
)]
async fn list_metrics(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
) -> Result<Json<MetricsResponse>, AppError> {
    Ok(Json(service::metrics::list(&state, &member, &id).await?))
}

#[utoipa::path(
    post,
    path = "/workspaces/{ws}/datasets/{id}/metrics",
    tag = "metrics",
    operation_id = "createMetric",
    summary = "Create a metric (admin only)",
    description = "The expression is run once on the current version, through the SQL guard, before it is saved. \
                   The metric is confirmed and used by the next question.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Dataset id (UUID)")
    ),
    request_body = MetricRequest,
    responses(
        (status = 201, body = Metric, description = "Created"),
        (status = "4XX", body = ErrorBody,
         description = "400 invalid metric, `field` names the field; 401; 403 not admin; 404 not a member or no such dataset")
    )
)]
async fn create_metric(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
    JsonBody(request): JsonBody<MetricRequest>,
) -> Result<(StatusCode, Json<Metric>), AppError> {
    let metric = service::metrics::create(&state, &member, &id, request).await?;
    Ok((StatusCode::CREATED, Json(metric)))
}

#[utoipa::path(
    put,
    path = "/workspaces/{ws}/datasets/{id}/metrics/{metric_id}",
    tag = "metrics",
    operation_id = "updateMetric",
    summary = "Save a new version of a metric (admin only)",
    description = "Adds a version with the next number and keeps the earlier ones. The name cannot change. Checked \
                   like a new metric.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Dataset id (UUID)"),
        ("metric_id" = String, Path, description = "Id of any version of the metric (UUID)")
    ),
    request_body = MetricRequest,
    responses(
        (status = 200, body = Metric, description = "The new version"),
        (status = "4XX", body = ErrorBody,
         description = "400 invalid metric, `field` names the field; 401; 403 not admin; 404 no such dataset or metric")
    )
)]
async fn update_metric(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id, metric_id)): PathParams<(String, String, String)>,
    JsonBody(request): JsonBody<MetricRequest>,
) -> Result<Json<Metric>, AppError> {
    let metric = service::metrics::update(&state, &member, &id, &metric_id, request).await?;
    Ok(Json(metric))
}

#[utoipa::path(
    delete,
    path = "/workspaces/{ws}/datasets/{id}/metrics/{metric_id}",
    tag = "metrics",
    operation_id = "deleteMetric",
    summary = "Delete a metric with all its versions (admin only)",
    description = "Earlier questions keep the SQL they ran.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Dataset id (UUID)"),
        ("metric_id" = String, Path, description = "Id of any version of the metric (UUID)")
    ),
    responses(
        (status = 204, description = "Deleted"),
        (status = "4XX", body = ErrorBody, description = "401; 403 not admin; 404 no such dataset or metric")
    )
)]
async fn delete_metric(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id, metric_id)): PathParams<(String, String, String)>,
) -> Result<StatusCode, AppError> {
    service::metrics::delete(&state, &member, &id, &metric_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    put,
    path = "/workspaces/{ws}/datasets/{id}/columns/aggregation",
    tag = "metrics",
    operation_id = "setColumnAggregation",
    summary = "Set the default aggregation of a measure column (admin only)",
    description = "Applies to the current version. The SQL prompt asks for AVG on `avg` columns, and why questions \
                   on them compare the average per row.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Dataset id (UUID)")
    ),
    request_body = ColumnAggregationRequest,
    responses(
        (status = 204, description = "Saved"),
        (status = "4XX", body = ErrorBody,
         description = "400 no measure column of that name (`field` is `column`); 401; 403 not admin; 404 no such dataset")
    )
)]
async fn set_column_aggregation(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
    JsonBody(request): JsonBody<ColumnAggregationRequest>,
) -> Result<StatusCode, AppError> {
    service::metrics::set_aggregation(&state, &member, &id, request).await?;
    Ok(StatusCode::NO_CONTENT)
}
