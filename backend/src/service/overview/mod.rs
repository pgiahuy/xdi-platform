// SPDX-License-Identifier: Apache-2.0
/* service/overview/mod.rs - tổng quan một dataset (#198), cho mọi thành viên hỏi được dataset đó: tóm tắt (số dòng,
khoảng thời gian, phiên bản), hồ sơ từng cột, chỉ số đã định nghĩa, và biểu đồ dựng sẵn. Route ở
api/datasets/overview.rs, hợp đồng ở overview/contract.rs. Không gọi mô hình ngôn ngữ, không gửi gì cho nó.

Hai nguồn số liệu:
- Hồ sơ trong Postgres (columns, column_values, metrics; overview/columns.rs): nhãn, mô tả, vai trò, đơn vị, % ô trống,
  số giá trị khác nhau, số dòng theo giá trị. Đọc thẳng, không chạy câu nào trên dữ liệu.
- Ba câu SQL do code dựng (overview/sql.rs, overview/time.rs): tóm tắt, độ đo chính theo kỳ, số dòng theo kỳ. Mọi câu
  qua guard với vai dữ liệu của người xem như câu hỏi và truy dòng nguồn, kết quả giữ theo phiên bản (overview/cache.rs).

Hồ sơ đếm trên mọi dòng, nên người bị lọc dòng theo vai dữ liệu không nhận số đếm của hồ sơ (restricted); phần tính bằng
SQL thì họ nhận đúng phần được xem. Không mở thêm quyền: ai gọi được route hỏi thì gọi được route này. */

pub mod cache;
mod charts;
mod columns;
pub mod contract;
mod missing;
mod query;
mod sql;
#[cfg(test)]
mod tests;
mod time;

use crate::auth::Member;
use crate::engine::guard::AllowedTable;
use crate::engine::store::{DatasetVersion, ProfileStatus};
use crate::service::ask::context::build_profiles;
use crate::service::ask::load_row_filters;
use crate::service::ingest;
use crate::service::overview::charts::build_value_chart;
use crate::service::overview::columns::{build_columns, build_metrics, shows_values, values_of};
use crate::service::overview::contract::{
    DatasetOverviewResponse, MissingCode, OverviewSummary, OverviewVersion,
};
use crate::service::overview::missing::build_missing;
use crate::service::overview::time::{add_time_parts, TimeInput};
use crate::{error::AppError, state::AppState};

pub async fn build_overview(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
) -> Result<DatasetOverviewResponse, AppError> {
    // dataset_detail: 404 cho id của workspace khác, và semantic_unavailable đã tính theo mô hình đang chạy.
    let dataset = ingest::dataset_detail(state, member, dataset_id).await?;
    let ws = &member.workspace;
    let metrics = state.store.list_metrics(ws, dataset.id).await?;
    let mut overview = DatasetOverviewResponse {
        dataset_id: dataset.id,
        name: dataset.name.clone(),
        version: None,
        summary: OverviewSummary {
            row_count: None,
            time_column: None,
            time_start: None,
            time_end: None,
            restricted: false,
        },
        columns: Vec::new(),
        metrics: build_metrics(&metrics),
        main_measure: None,
        charts: Vec::new(),
        missing: Vec::new(),
    };
    let Some(version) = state.store.get_current_version(ws, dataset.id).await? else {
        overview.missing.push(build_missing(MissingCode::NoVersion));
        return Ok(overview);
    };
    overview.version = Some(build_version(&version));
    let status = version.profile_status.as_str();
    if status == ProfileStatus::Pending.as_str() || status == ProfileStatus::Failed.as_str() {
        let code = if status == ProfileStatus::Pending.as_str() {
            MissingCode::ProfilePending
        } else {
            MissingCode::ProfileFailed
        };
        overview.missing.push(build_missing(code));
        return Ok(overview);
    }
    if status == ProfileStatus::Structure.as_str() {
        let code = if dataset.semantic_unavailable {
            MissingCode::SemanticUnavailable
        } else {
            MissingCode::SemanticPending
        };
        overview.missing.push(build_missing(code));
    }

    let columns = state.store.list_columns(ws, version.id).await?;
    let row_filters = load_row_filters(state, member, dataset.id).await?;
    let restricted = !row_filters.is_empty();
    overview.summary.restricted = restricted;
    if restricted {
        overview
            .missing
            .push(build_missing(MissingCode::Restricted));
    }
    let values = if restricted {
        Vec::new()
    } else {
        let ids: Vec<uuid::Uuid> = columns
            .iter()
            .filter(|c| shows_values(c))
            .map(|c| c.id)
            .collect();
        state.store.list_column_values(ws, &ids).await?
    };
    overview.columns = build_columns(&columns, &values, version.row_count, restricted);
    let profile = build_profiles(columns.clone());

    let tables = [AllowedTable {
        name: dataset.logical_table.clone(),
        columns: columns.iter().map(|c| c.name.clone()).collect(),
        parquet_dir: state.config.data_dir.join(parquet_path(&version)?),
        row_filters,
    }];
    let input = TimeInput {
        state,
        version_id: version.id,
        table: &dataset.logical_table,
        tables: &tables,
        columns: &columns,
        metrics: &metrics,
        profile: &profile,
    };
    add_time_parts(&mut overview, &input).await?;
    if !restricted {
        let charts = columns.iter().filter(|c| shows_values(c)).filter_map(|c| {
            build_value_chart(&dataset.logical_table, c, &values_of(c, &values), &profile)
        });
        overview.charts.extend(charts);
    }
    Ok(overview)
}

fn build_version(version: &DatasetVersion) -> OverviewVersion {
    OverviewVersion {
        id: version.id,
        number: version.version,
        profile_status: version.profile_status.clone(),
        created_at: version.created_at,
    }
}

/// Phiên bản nạp từ file luôn có đường dẫn Parquet; nguồn database (sau này) thì không, và cần cách chạy khác.
fn parquet_path(version: &DatasetVersion) -> Result<&str, AppError> {
    version
        .parquet_path
        .as_deref()
        .ok_or_else(|| AppError::Internal("dataset version has no parquet path".to_string()))
}
