// SPDX-License-Identifier: Apache-2.0
/* service/ask/checks.rs - kiểm hợp lý kết quả của một câu hỏi (#36): kết quả đáng ngờ thì kèm cảnh báo (Warning của
ask/contract.rs) thay vì trả im lặng. Cảnh báo không chặn gì: status vẫn như cũ, kết quả vẫn hiện, giao diện hiện lời
phía trên kết quả (frontend/src/components/ask/AskWarnings.tsx).

service/ask/mod.rs gọi build_result_warnings sau run_with_repair. Phần cần Postgres (danh sách giá trị của cột, qua cache
của ask/values.rs) làm ở đó, rồi build_warnings, hàm thuần, dựng cảnh báo theo thứ tự cố định của các luật:
- empty_result: câu không ra dòng nào.
- all_null: có cột kết quả chỉ toàn ô trống. Chỉ xét các dòng đã gửi; kết quả bị cắt thì lời ghi rõ "trong n dòng đầu".
  Với empty_result và all_null, câu lọc theo một cột thời gian thì lời kèm khoảng thời gian dữ liệu có (min, max của
  hồ sơ cột): hỏi năm 2019 trên dữ liệu tới tháng 1/2018 thì người hỏi biết ngay vì sao không có số.
- filter_value_not_found: câu lọc `cột = 'x'` hay `cột IN (...)` trên cột có danh sách giá trị (cột chiều không nhạy
  cảm), hoặc mô hình nêu giá trị đó trong terms, mà cột không có giá trị ấy. Gợi ý giá trị gần nhất: khác hoa thường,
  khác dấu, trùng bí danh, hay khác một hai chữ. Phần tra cứu ở ask/filter_values.rs.
- unusual_magnitude: ở ask/magnitude.rs.

Không kiểm điều kiện phân quyền guard thêm (#31): người hỏi không viết chúng, và chúng lọc theo giá trị của vai dữ liệu
chứ không theo câu hỏi. Lỗi khi tra danh sách giá trị thì bỏ luật filter_value_not_found, ghi log, câu trả lời vẫn đủ.
Kiểm theo chuỗi thời gian (kỳ thiếu dữ liệu, như câu "tại sao") để sau. Test ở ask/checks_tests.rs. */

use crate::auth::WorkspaceId;
use crate::engine::sql_shape::{build_query_shape, QueryShape};
use crate::engine::store::{DatasetVersion, ProfileColumn, ProfileRole};
use crate::service::ask::contract::{AskStatus, ResultTable, Warning, WarningCode};
use crate::service::ask::filter_values::{build_filter_values, build_missing_values, MissingValue};
use crate::service::ask::magnitude::build_magnitude_warnings;
use crate::service::ask::reply::Term;
use crate::service::ask::values::load_value_index;
use crate::service::explain::number::build_number_text;
use crate::state::AppState;

/// Câu đã chạy mà các luật đọc: các trường cùng tên của RunOutcome (ask/run.rs), cộng hồ sơ cột của phiên bản.
pub struct CheckedQuery<'a> {
    pub status: AskStatus,
    pub sql_guarded: Option<&'a str>,
    pub result: Option<&'a ResultTable>,
    /// GuardedSql::row_filters: vế phân quyền, không kiểm.
    pub row_filters: &'a [String],
    pub terms: &'a [Term],
    /// Hồ sơ mọi cột của phiên bản (Store::list_columns).
    pub columns: &'a [ProfileColumn],
    /// Người hỏi bị lọc dòng theo vai dữ liệu (AllowedTable::row_filters không rỗng).
    pub restricted: bool,
}

/// Cảnh báo kiểm hợp lý của một câu thường. Câu chưa answered không có kết quả để xét, chỉ còn giá trị mô hình nêu
/// trong terms. Không bao giờ lỗi: phần nào không đọc được thì bỏ luật của phần đó, kèm log.
pub async fn build_result_warnings(
    state: &AppState,
    ws: &WorkspaceId,
    version: &DatasetVersion,
    checked: &CheckedQuery<'_>,
) -> Vec<Warning> {
    let (shape, missing) = find_missing_values(state, ws, version, checked).await;
    let answered = checked.status == AskStatus::Answered;
    let result = checked.result.filter(|_| answered);
    build_warnings(result, shape.as_ref(), checked.columns, &missing)
}

/// Phần tra cứu của luật filter_value_not_found: cây cú pháp của sql_guarded (None khi chưa answered hay không đọc
/// được) và giá trị lọc mà cột không có. ask/mod.rs còn gọi hàm này để quyết định gửi lại lượt viết SQL kèm đủ giá trị
/// của cột (#230).
pub async fn find_missing_values(
    state: &AppState,
    ws: &WorkspaceId,
    version: &DatasetVersion,
    checked: &CheckedQuery<'_>,
) -> (Option<QueryShape>, Vec<MissingValue>) {
    let answered = checked.status == AskStatus::Answered;
    let shape = match checked
        .sql_guarded
        .filter(|_| answered)
        .map(build_query_shape)
    {
        Some(Ok(shape)) => Some(shape),
        Some(Err(e)) => {
            tracing::warn!(error = ?e, "cannot read the guarded sql, skipping the checks that need it");
            None
        }
        None => None,
    };
    let wanted = build_filter_values(
        shape.as_ref(),
        checked.row_filters,
        checked.terms,
        checked.columns,
    );
    // Danh sách giá trị đã vào cache từ bước context (ask/context.rs), nên tra lại ở đây thường không đọc Postgres.
    if wanted.is_empty() {
        return (shape, Vec::new());
    }
    let missing = match load_value_index(state, ws, version).await {
        Ok(index) if index.loaded => {
            let mut missing = build_missing_values(&wanted, &index);
            // Danh sách giá trị là của cả cột, không theo vai dữ liệu: gợi ý cho người bị lọc dòng có thể lộ giá trị
            // của dòng họ không được xem, nên chỉ báo không có, không gợi ý.
            if checked.restricted {
                missing.iter_mut().for_each(|m| m.suggestions.clear());
            }
            missing
        }
        Ok(_) => {
            tracing::warn!(version_id = %version.id, "column values not loaded, skipping the filter value check");
            Vec::new()
        }
        Err(e) => {
            tracing::warn!(error = ?e, "cannot load column values, skipping the filter value check");
            Vec::new()
        }
    };
    (shape, missing)
}

/// Mọi cảnh báo kiểm hợp lý, theo thứ tự luật ở đầu file. result và shape là None khi câu chưa answered hay khi không
/// đọc được sql_guarded; luật cần chúng thì bỏ qua.
pub fn build_warnings(
    result: Option<&ResultTable>,
    shape: Option<&QueryShape>,
    profile: &[ProfileColumn],
    missing: &[MissingValue],
) -> Vec<Warning> {
    let mut warnings = Vec::new();
    if let Some(result) = result {
        let note = shape.and_then(|shape| build_time_range_note(shape, profile));
        let with_note = |mut warning: Warning| {
            if let Some(note) = &note {
                warning.message = format!("{} {note}", warning.message);
            }
            warning
        };
        warnings.extend(build_empty_warning(result).map(with_note));
        warnings.extend(build_null_warning(result).map(with_note));
    }
    warnings.extend(missing.iter().map(build_missing_warning));
    if let (Some(result), Some(shape)) = (result, shape) {
        warnings.extend(build_magnitude_warnings(result, shape, profile));
    }
    warnings
}

/// Câu ra 0 dòng. Dùng cả cho câu "tại sao" (ask/why.rs).
pub fn build_empty_warning(result: &ResultTable) -> Option<Warning> {
    (result.total_rows == 0).then(|| Warning {
        code: WarningCode::EmptyResult,
        message: "Không có dòng nào khớp câu hỏi. Kiểm lại giá trị lọc và khoảng thời gian: dữ liệu có thể viết \
                  giá trị khác, hoặc không có kỳ đó."
            .to_string(),
    })
}

/// Một cảnh báo cho mọi cột toàn ô trống, để câu tổng hợp trên tập rỗng (một dòng, mọi cột null) không ra một cảnh báo
/// cho mỗi cột.
fn build_null_warning(result: &ResultTable) -> Option<Warning> {
    if result.rows.is_empty() {
        return None;
    }
    let names: Vec<&str> = result
        .columns
        .iter()
        .enumerate()
        .filter(|(i, _)| result.rows.iter().all(|row| row[*i].is_none()))
        .map(|(_, column)| column.name.as_str())
        .collect();
    if names.is_empty() {
        return None;
    }
    let scope = if result.truncated {
        format!(
            "Trong {} dòng đầu, cột",
            build_number_text(&result.rows.len().to_string())
        )
    } else {
        "Cột".to_string()
    };
    Some(Warning {
        code: WarningCode::AllNull,
        message: format!(
            "{scope} {} chỉ có ô trống. Thường do không dòng nào khớp điều kiện lọc, hoặc cột không có dữ liệu; \
             kiểm lại điều kiện lọc và cột đã chọn.",
            names.join(", ")
        ),
    })
}

/// Khoảng thời gian dữ liệu có, của các cột thời gian mà WHERE của câu lọc theo; None khi câu không lọc theo cột thời
/// gian nào hay hồ sơ chưa có min, max. Không xét vế trong hộp đọc bảng: vế đó có thể là điều kiện phân quyền guard
/// thêm, người hỏi không viết.
fn build_time_range_note(shape: &QueryShape, profile: &[ProfileColumn]) -> Option<String> {
    let notes: Vec<String> = profile
        .iter()
        .filter(|c| c.role == ProfileRole::Time.as_str())
        .filter(|c| {
            shape
                .filters
                .iter()
                .any(|f| !f.in_table_box && mentions_column(&f.sql, &c.name))
        })
        .filter_map(|c| {
            let from = build_day_text(c.min_value.as_deref()?);
            let until = build_day_text(c.max_value.as_deref()?);
            Some(format!("cột {} có dữ liệu từ {from} tới {until}", c.name))
        })
        .collect();
    (!notes.is_empty()).then(|| format!("Trong dữ liệu, {}.", notes.join("; ")))
}

/// Cột có mặt trong chữ của một vế: viết trong nháy kép, hoặc viết trơn mà không dính vào tên dài hơn.
fn mentions_column(sql: &str, column: &str) -> bool {
    if sql.contains(&format!("\"{column}\"")) {
        return true;
    }
    sql.match_indices(column).any(|(start, _)| {
        let is_name = |c: char| c.is_alphanumeric() || c == '_';
        let before = sql[..start].chars().next_back();
        let after = sql[start + column.len()..].chars().next();
        !before.is_some_and(is_name) && !after.is_some_and(is_name)
    })
}

/// "2015-01-01 00:00:00" (min, max của cột thời gian trong hồ sơ) thành "01/01/2015"; chữ khác dạng thì giữ nguyên.
fn build_day_text(value: &str) -> String {
    let day = value.get(..10).unwrap_or(value);
    match day.split('-').collect::<Vec<_>>().as_slice() {
        [year, month, date] if year.len() == 4 => format!("{date}/{month}/{year}"),
        _ => value.to_string(),
    }
}

fn build_missing_warning(missing: &MissingValue) -> Warning {
    let MissingValue {
        column,
        value,
        suggestions,
    } = missing;
    let hint = if suggestions.is_empty() {
        String::new()
    } else {
        format!(" Giá trị gần nhất: {}.", suggestions.join(", "))
    };
    Warning {
        code: WarningCode::FilterValueNotFound,
        message: format!(
            "Cột {column} không có giá trị \"{value}\".{hint} Kiểm lại giá trị lọc: kết quả có thể rỗng hoặc thiếu \
             dòng."
        ),
    }
}
