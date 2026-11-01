// SPDX-License-Identifier: Apache-2.0
/* service/metrics/mod.rs - màn Chỉ số (#185): mọi thành viên xem chỉ số của một dataset cùng cột của phiên bản hiện
hành; admin tạo, sửa, xóa chỉ số và đổi cách gộp mặc định của cột số đo. API ở api/metrics.rs, ghi xuống Postgres ở
engine/store/metric_edits.rs và engine/store/aggregation.rs.

Chỉ số admin viết là origin 'ui', đã xác nhận: prompt sinh SQL (service/ask/prompt.rs) và form câu "tại sao"
(service/explain/measure.rs) dùng ngay ở câu hỏi kế tiếp. Vì vậy mọi chỉ số chạy thử trước khi lưu (check.rs) trên
phiên bản hiện hành: một công thức sai cột vào prompt như công thức đã duyệt thì mô hình viết SQL lỗi ở mọi câu hỏi.
Sửa là thêm phiên bản (version + 1), không đổi tên; muốn đổi tên thì tạo chỉ số mới rồi xóa chỉ số cũ. */

pub mod check;
pub mod contract;

use uuid::Uuid;

use crate::auth::Member;
use crate::engine::store::{Dataset, DatasetVersion, NewMetric};
use crate::error::AppError;
use crate::service::ask::phrases::build_key;
use crate::service::ingest::{dataset_not_found, parse_dataset_id};
use crate::service::metrics::check::{build_check_table, check_metric, MetricProblem};
use crate::service::metrics::contract::{
    ColumnAggregationRequest, Metric, MetricColumn, MetricRequest, MetricsResponse,
};
use crate::state::AppState;

// Trần độ dài, đếm theo ký tự. Tên và mô tả vào prompt của mọi câu hỏi trên dataset và hiện trên thẻ "Cách tính".
const NAME_MAX_CHARS: usize = 80;
const DESCRIPTION_MAX_CHARS: usize = 300;
const EXPRESSION_MAX_CHARS: usize = 2000;
const UNIT_MAX_CHARS: usize = 30;

/// Chỉ số và cột của một dataset; mọi thành viên xem được.
pub async fn list(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
) -> Result<MetricsResponse, AppError> {
    let dataset = load_dataset(state, member, dataset_id).await?;
    let ws = &member.workspace;
    let metrics = state.store.list_metric_records(ws, dataset.id).await?;
    let columns = match state.store.get_current_version(ws, dataset.id).await? {
        Some(version) => state.store.list_columns(ws, version.id).await?,
        None => Vec::new(),
    };
    Ok(MetricsResponse {
        metrics: metrics.into_iter().map(Metric::from).collect(),
        columns: columns.into_iter().map(MetricColumn::from).collect(),
    })
}

/// Tạo một chỉ số mới (chỉ admin). Tên trùng một chỉ số có sẵn (không phân biệt hoa thường và dấu thanh, như cách
/// prompt tìm chỉ số theo tên) thì từ chối: hai chỉ số gần cùng tên làm mô hình chọn nhầm.
pub async fn create(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
    request: MetricRequest,
) -> Result<Metric, AppError> {
    member.require_admin()?;
    let dataset = load_dataset(state, member, dataset_id).await?;
    let metric = build_new_metric(request)?;
    let existing = state
        .store
        .list_metric_records(&member.workspace, dataset.id)
        .await?;
    let key = build_key(&metric.name);
    if existing.iter().any(|m| build_key(&m.name) == key) {
        return Err(MetricProblem::new(
            "name",
            "Đã có chỉ số tên này; sửa chỉ số đó hoặc đặt tên khác",
        )
        .into());
    }
    save(state, member, &dataset, &metric).await
}

/// Sửa một chỉ số (chỉ admin): thêm phiên bản mới cùng tên, bản cũ giữ lại.
pub async fn update(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
    metric_id: &str,
    request: MetricRequest,
) -> Result<Metric, AppError> {
    member.require_admin()?;
    let dataset = load_dataset(state, member, dataset_id).await?;
    let name = load_metric_name(state, member, &dataset, metric_id).await?;
    let metric = build_new_metric(request)?;
    if metric.name != name {
        return Err(MetricProblem::new(
            "name",
            "Không đổi tên được khi sửa; tạo chỉ số mới với tên mới rồi xóa chỉ số này",
        )
        .into());
    }
    save(state, member, &dataset, &metric).await
}

/// Xóa một chỉ số cùng mọi phiên bản của nó (chỉ admin).
pub async fn delete(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
    metric_id: &str,
) -> Result<(), AppError> {
    member.require_admin()?;
    let dataset = load_dataset(state, member, dataset_id).await?;
    let name = load_metric_name(state, member, &dataset, metric_id).await?;
    state
        .store
        .delete_metric(&member.workspace, dataset.id, &name)
        .await?;
    Ok(())
}

/// Đổi cách gộp mặc định của một cột số đo trong phiên bản hiện hành (chỉ admin). Phiên bản nạp sau lấy cách gộp lớp
/// ngữ nghĩa đề xuất cho chính nó.
pub async fn set_aggregation(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
    request: ColumnAggregationRequest,
) -> Result<(), AppError> {
    member.require_admin()?;
    let dataset = load_dataset(state, member, dataset_id).await?;
    let version = load_current_version(state, member, &dataset).await?;
    let updated = state
        .store
        .set_column_aggregation(
            &member.workspace,
            version.id,
            &request.column,
            request.aggregation,
        )
        .await?;
    if !updated {
        return Err(MetricProblem::new(
            "column",
            format!(
                "Không có cột số đo \"{}\" trong phiên bản hiện hành",
                request.column
            ),
        )
        .into());
    }
    Ok(())
}

/// Chạy thử trên phiên bản hiện hành rồi ghi phiên bản mới; trả chỉ số như danh sách sẽ hiện.
async fn save(
    state: &AppState,
    member: &Member,
    dataset: &Dataset,
    metric: &NewMetric,
) -> Result<Metric, AppError> {
    let version = load_current_version(state, member, dataset).await?;
    let table = build_check_table(state, &member.workspace, dataset, &version).await?;
    check_metric(state, &table, metric).await?;
    if let Some(other) = state
        .store
        .find_metric_name_elsewhere(&member.workspace, dataset.id, &metric.name)
        .await?
    {
        return Err(MetricProblem::new(
            "name",
            format!(
                "Bảng {other} cùng dataset đã có chỉ số tên \"{}\"; đặt tên khác",
                metric.name
            ),
        )
        .into());
    }
    let id = state
        .store
        .add_metric_version(&member.workspace, dataset.id, metric, member.user.id)
        .await?;
    state
        .store
        .list_metric_records(&member.workspace, dataset.id)
        .await?
        .into_iter()
        .find(|m| m.id == id)
        .map(Metric::from)
        .ok_or_else(|| AppError::Internal("saved metric is not in the list".to_string()))
}

async fn load_dataset(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
) -> Result<Dataset, AppError> {
    let id = parse_dataset_id(dataset_id)?;
    state
        .store
        .get_dataset(&member.workspace, id)
        .await?
        .ok_or_else(dataset_not_found)
}

/// Phiên bản hiện hành đã có hồ sơ cột: chạy thử chỉ số cần danh sách cột cho guard.
async fn load_current_version(
    state: &AppState,
    member: &Member,
    dataset: &Dataset,
) -> Result<DatasetVersion, AppError> {
    let no_data = || {
        AppError::BadRequest(
            "Dataset chưa có dữ liệu dùng được; nạp file trước khi định nghĩa chỉ số".to_string(),
        )
    };
    let version = state
        .store
        .get_current_version(&member.workspace, dataset.id)
        .await?
        .ok_or_else(no_data)?;
    if version.parquet_path.is_none() {
        return Err(no_data());
    }
    Ok(version)
}

async fn load_metric_name(
    state: &AppState,
    member: &Member,
    dataset: &Dataset,
    metric_id: &str,
) -> Result<String, AppError> {
    let not_found = || AppError::NotFound("Không có chỉ số này".to_string());
    let id = Uuid::parse_str(metric_id.trim()).map_err(|_| not_found())?;
    state
        .store
        .find_metric_name(&member.workspace, dataset.id, id)
        .await?
        .ok_or_else(not_found)
}

/// Làm sạch request: bỏ khoảng trắng hai đầu, chuỗi rỗng thành không có, kiểm độ dài và ký tự điều khiển.
fn build_new_metric(request: MetricRequest) -> Result<NewMetric, MetricProblem> {
    let name = clean_text(&request.name, "name", "Tên", NAME_MAX_CHARS)?
        .ok_or_else(|| MetricProblem::new("name", "Tên chỉ số không được để trống"))?;
    let description = clean_text(
        &request.description,
        "description",
        "Mô tả",
        DESCRIPTION_MAX_CHARS,
    )?
    .unwrap_or_default();
    let expression = clean_text(
        &request.expression,
        "expression",
        "Công thức",
        EXPRESSION_MAX_CHARS,
    )?
    .ok_or_else(|| MetricProblem::new("expression", "Công thức không được để trống"))?;
    let optional =
        |value: &Option<String>, field: &'static str, label: &str, max: usize| match value {
            Some(text) => clean_text(text, field, label, max),
            None => Ok(None),
        };
    Ok(NewMetric {
        name,
        description,
        expression,
        filter: optional(&request.filter, "filter", "Điều kiện", EXPRESSION_MAX_CHARS)?,
        time_column: optional(
            &request.time_column,
            "time_column",
            "Cột thời gian",
            EXPRESSION_MAX_CHARS,
        )?,
        unit: optional(&request.unit, "unit", "Đơn vị", UNIT_MAX_CHARS)?,
    })
}

/// None khi chuỗi rỗng sau khi bỏ khoảng trắng. Mọi trường vào prompt trên một dòng, nên ký tự điều khiển bị từ chối:
/// xuống dòng trong tên hay mô tả sẽ chèn được dòng giả vào prompt. Riêng công thức và điều kiện, xuống dòng và tab
/// (người dán SQL nhiều dòng) đổi thành dấu cách, cùng nghĩa với SQL.
fn clean_text(
    text: &str,
    field: &'static str,
    label: &str,
    max_chars: usize,
) -> Result<Option<String>, MetricProblem> {
    let sql = matches!(field, "expression" | "filter");
    let text: String = if sql {
        text.chars()
            .map(|c| if c.is_whitespace() { ' ' } else { c })
            .collect()
    } else {
        text.to_string()
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > max_chars {
        return Err(MetricProblem::new(
            field,
            format!("{label} dài tối đa {max_chars} ký tự"),
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(MetricProblem::new(
            field,
            format!("{label} có ký tự không in được, ví dụ xuống dòng"),
        ));
    }
    // Prompt ghi tên trong ngoặc kép (- "Doanh thu"); dấu " trong tên làm dòng đó đọc ra hai tên.
    if field == "name" && trimmed.contains('"') {
        return Err(MetricProblem::new(
            field,
            format!("{label} không được có dấu ngoặc kép (\")"),
        ));
    }
    Ok(Some(trimmed.to_string()))
}
