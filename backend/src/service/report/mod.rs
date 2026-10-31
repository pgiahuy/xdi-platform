// SPDX-License-Identifier: Apache-2.0
/* service/report/ - báo cáo: người dùng gom các câu trả lời đã chọn thành một trang có tiêu đề, đoạn mở đầu và thứ tự
riêng, hay để hệ thống tạo báo cáo tự động cho một dataset (auto.rs dựng kế hoạch, auto_run.rs chạy). Mở báo cáo thì
mọi mục được dựng lại từ câu hỏi đã lưu (render.rs); xuất Excel dùng đúng bản dựng lại đó (xlsx.rs). Xuất PDF là việc
của giao diện: trang báo cáo có kiểu in riêng. Hợp đồng ở contract.rs, route ở api/reports.rs, test ở report/tests.rs
và api/report_tests.rs.

Chỉ người tạo đọc và sửa được một báo cáo, kể cả admin của workspace, như cuộc trò chuyện (service/conversation/):
báo cáo gom câu hỏi của một người. Báo cáo của người khác hay của workspace khác trả 404 như id không có. Chỉ thêm được
câu do chính người gọi hỏi và đã trả lời. */

pub mod auto;
mod auto_run;
pub mod contract;
mod render;
#[cfg(test)]
mod tests;
mod xlsx;

use chrono::Utc;
use uuid::Uuid;

use crate::auth::Member;
use crate::engine::store::{QueryStatus, ReportRow};
use crate::service::report::contract::{
    AutoReportRequest, AutoReportResponse, CreateReportRequest, ReportKind, ReportListResponse,
    ReportResponse, ReportSummary, UpdateReportRequest,
};
use crate::{error::AppError, state::AppState};

/// Độ dài tối đa của tiêu đề và đoạn mở đầu, theo ký tự.
const TITLE_MAX_CHARS: usize = 200;
const INTRO_MAX_CHARS: usize = 5000;

/// Số mục tối đa của một báo cáo: mở báo cáo chạy lại mọi câu, nên báo cáo rất dài làm trang mở chậm.
const ITEMS_MAX: i64 = 30;

pub async fn list(state: &AppState, member: &Member) -> Result<ReportListResponse, AppError> {
    let rows = state
        .store
        .list_reports(&member.workspace, member.user.id)
        .await?;
    Ok(ReportListResponse {
        reports: rows.iter().map(build_summary).collect(),
    })
}

pub async fn create(
    state: &AppState,
    member: &Member,
    req: &CreateReportRequest,
) -> Result<ReportSummary, AppError> {
    let title = check_title(&req.title)?;
    let intro = check_intro(&req.intro)?;
    let report = crate::engine::store::NewReport {
        created_by: member.user.id,
        title,
        intro,
        source_id: None,
        model_calls: None,
    };
    let id = state
        .store
        .create_report(&member.workspace, &report)
        .await?;
    load_summary(state, member, id).await
}

pub async fn update(
    state: &AppState,
    member: &Member,
    id: &str,
    req: &UpdateReportRequest,
) -> Result<ReportSummary, AppError> {
    let report = load_owned(state, member, id).await?;
    let title = req.title.as_deref().map(check_title).transpose()?;
    let intro = req.intro.as_deref().map(check_intro).transpose()?;
    state
        .store
        .update_report(&member.workspace, report.id, title, intro)
        .await?;
    load_summary(state, member, report.id).await
}

pub async fn delete(state: &AppState, member: &Member, id: &str) -> Result<(), AppError> {
    let report = load_owned(state, member, id).await?;
    state
        .store
        .delete_report(&member.workspace, report.id)
        .await
}

/// Thêm một câu đã trả lời của người gọi vào cuối báo cáo.
pub async fn add_item(
    state: &AppState,
    member: &Member,
    id: &str,
    query_id: Uuid,
) -> Result<ReportSummary, AppError> {
    let report = load_owned(state, member, id).await?;
    let ws = &member.workspace;
    // Câu của người khác hay của workspace khác: 404 như câu không có, để không lộ là id đó tồn tại.
    let query = state
        .store
        .get_reportable_query(ws, query_id)
        .await?
        .filter(|q| q.user_id == Some(member.user.id))
        .ok_or_else(|| AppError::NotFound("Không có câu hỏi này".to_string()))?;
    if query.status != QueryStatus::Ok.as_str() {
        return Err(AppError::BadRequest(
            "Chỉ thêm được câu đã có câu trả lời".to_string(),
        ));
    }
    if report.item_count >= ITEMS_MAX {
        return Err(AppError::BadRequest(format!(
            "Báo cáo đã có {ITEMS_MAX} mục, mức tối đa; bỏ bớt mục hay tạo báo cáo mới"
        )));
    }
    if !state.store.add_report_item(ws, report.id, query_id).await? {
        return Err(AppError::BadRequest(
            "Câu này đã có trong báo cáo".to_string(),
        ));
    }
    load_summary(state, member, report.id).await
}

pub async fn remove_item(
    state: &AppState,
    member: &Member,
    id: &str,
    item_id: &str,
) -> Result<(), AppError> {
    let report = load_owned(state, member, id).await?;
    let item = parse_id(item_id, "Không có mục này")?;
    if !state
        .store
        .remove_report_item(&member.workspace, report.id, item)
        .await?
    {
        return Err(AppError::NotFound("Không có mục này".to_string()));
    }
    Ok(())
}

/// Thứ tự mới: item_ids phải là đúng mọi mục của báo cáo, mỗi mục một lần.
pub async fn reorder(
    state: &AppState,
    member: &Member,
    id: &str,
    item_ids: &[Uuid],
) -> Result<(), AppError> {
    let report = load_owned(state, member, id).await?;
    let ws = &member.workspace;
    let items = state.store.list_report_items(ws, report.id).await?;
    let mut current: Vec<Uuid> = items.iter().map(|i| i.id).collect();
    let mut wanted = item_ids.to_vec();
    current.sort_unstable();
    wanted.sort_unstable();
    if current != wanted {
        return Err(AppError::BadRequest(
            "Thứ tự mới phải gồm đúng mọi mục của báo cáo, mỗi mục một lần".to_string(),
        ));
    }
    state
        .store
        .reorder_report_items(ws, report.id, item_ids)
        .await
}

/// Báo cáo kèm mọi mục dựng lại.
pub async fn open(state: &AppState, member: &Member, id: &str) -> Result<ReportResponse, AppError> {
    let report = load_owned(state, member, id).await?;
    let ws = &member.workspace;
    let rows = state.store.list_report_items(ws, report.id).await?;
    let labels = state
        .store
        .list_version_labels(ws, &render::collect_version_ids(&rows))
        .await?;
    let rendered_at = Utc::now();
    let mut items = Vec::with_capacity(rows.len());
    // Lần lượt từng mục: mỗi câu chờ lượt trong hàng chạy chung (state.query_jobs), chạy song song cũng không nhanh hơn
    // khi máy bận, và lỗi của một mục không làm hỏng mục khác.
    for row in &rows {
        items.push(render::render_item(state, member, row, &labels).await?);
    }
    Ok(ReportResponse {
        report: build_summary(&report),
        items,
        rendered_at,
    })
}

/// File Excel của báo cáo và tên file gợi ý.
pub async fn export_xlsx(
    state: &AppState,
    member: &Member,
    id: &str,
) -> Result<(Vec<u8>, String), AppError> {
    let report = open(state, member, id).await?;
    let bytes = xlsx::build_report_xlsx(&report)?;
    let name = format!(
        "bao-cao-{}.xlsx",
        &report.report.id.simple().to_string()[..8]
    );
    Ok((bytes, name))
}

pub async fn create_automatic(
    state: &AppState,
    member: &Member,
    req: &AutoReportRequest,
) -> Result<AutoReportResponse, AppError> {
    let built = auto_run::build_auto_report(state, member, req.dataset_id).await?;
    Ok(AutoReportResponse {
        report: load_summary(state, member, built.id).await?,
        skipped: built.skipped,
    })
}

/// Báo cáo của người gọi; mọi trường hợp khác là 404 (lý do ở đầu file).
async fn load_owned(state: &AppState, member: &Member, id: &str) -> Result<ReportRow, AppError> {
    let id = parse_id(id, "Không có báo cáo này")?;
    state
        .store
        .get_report(&member.workspace, id)
        .await?
        .filter(|r| r.created_by == member.user.id)
        .ok_or_else(|| AppError::NotFound("Không có báo cáo này".to_string()))
}

async fn load_summary(
    state: &AppState,
    member: &Member,
    id: Uuid,
) -> Result<ReportSummary, AppError> {
    let report = load_owned(state, member, &id.to_string()).await?;
    Ok(build_summary(&report))
}

fn build_summary(row: &ReportRow) -> ReportSummary {
    ReportSummary {
        id: row.id,
        title: row.title.clone(),
        intro: row.intro.clone(),
        kind: if row.kind == "automatic" {
            ReportKind::Automatic
        } else {
            ReportKind::Manual
        },
        dataset_id: row.source_id,
        model_calls: row.model_calls.and_then(|n| u32::try_from(n).ok()),
        item_count: u32::try_from(row.item_count).unwrap_or(u32::MAX),
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn check_title(title: &str) -> Result<&str, AppError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError::BadRequest(
            "Tiêu đề báo cáo đang trống".to_string(),
        ));
    }
    if title.chars().count() > TITLE_MAX_CHARS {
        return Err(AppError::BadRequest(format!(
            "Tiêu đề báo cáo dài quá {TITLE_MAX_CHARS} ký tự"
        )));
    }
    Ok(title)
}

fn check_intro(intro: &str) -> Result<&str, AppError> {
    let intro = intro.trim();
    if intro.chars().count() > INTRO_MAX_CHARS {
        return Err(AppError::BadRequest(format!(
            "Đoạn mở đầu dài quá {INTRO_MAX_CHARS} ký tự"
        )));
    }
    Ok(intro)
}

fn parse_id(id: &str, missing: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(id.trim()).map_err(|_| AppError::NotFound(missing.to_string()))
}
