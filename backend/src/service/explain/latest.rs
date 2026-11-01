// SPDX-License-Identifier: Apache-2.0
/* service/explain/latest.rs - kỳ cần giải thích khi câu "tại sao" so hai kỳ mà không nêu kỳ nào ("Tại sao doanh thu
giảm", #164). explain/run.rs gọi find_latest trước khi kiểm form: code đếm số dòng theo kỳ trên DuckDB, chọn kỳ gần
nhất có đủ dữ liệu, ghi vào form (compare, time) rồi kiểm form như câu có nêu kỳ; kỳ gốc vẫn là kỳ liền trước như
mọi câu so hai kỳ (explain/check.rs). Lựa chọn why:compare trên thẻ "Cách tính" cho người hỏi đổi sang kỳ khác.

Code chọn chứ không nhờ mô hình đoán: mô hình không thấy dữ liệu nên không biết kỳ nào có dòng, và cùng câu hỏi phải
luôn ra cùng kỳ. Test ở explain/run_tests.rs (trọn đường) và cuối file này (quy tắc chọn kỳ). */

use crate::engine::store::{ProfileColumn, ProfileRole};
use crate::error::AppError;
use crate::service::ask::contract::{CellValue, Choice, ChoiceOption};
use crate::service::explain::breakdown::{run_code_query, CodeQuery, QueryEnv};
use crate::service::explain::form::{FormTime, ReadForm, WhyForm};
use crate::service::explain::period::{Grain, Period};
use crate::service::explain::sql::{build_equals, build_ident, build_string, build_time_expr};

/// Id lựa chọn kỳ cần giải thích, cùng dạng why:<trường> với BASE_CHOICE_ID. Phương án là kỳ viết như trong form
/// ("2017-Q4"), để lần hỏi sau đọc lại bằng Period::parse.
pub const COMPARE_CHOICE_ID: &str = "why:compare";
/// Số kỳ gần nhất có dữ liệu đưa làm phương án: đủ để lùi vài kỳ mà chip không dài quá một dòng.
const LATEST_OPTIONS: usize = 6;

/// Kỳ code đã chọn, kèm các kỳ gần nhất có dữ liệu để người hỏi đổi.
#[derive(Debug, Clone, PartialEq)]
pub struct LatestPeriod {
    pub time: FormTime,
    pub compare: Period,
    /// Kỳ gần nhất có dữ liệu, mới nhất trước.
    pub options: Vec<Period>,
    /// true khi kỳ là phương án người hỏi đã chọn (why:compare), false khi code chọn kỳ gần nhất.
    pub chosen: bool,
}

/// Số dòng của một kỳ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeriodRows {
    pub period: Period,
    pub rows: i64,
}

/// Cột thời gian và grain khi code phải chọn kỳ: form so hai kỳ (không có groups) mà compare trống. Form không ghi
/// time thì lấy cột vai trò time duy nhất của hồ sơ; nhiều cột time thì không đoán cột nào, để kiểm form báo lỗi như
/// cũ. Grain mặc định là tháng khi form không ghi.
pub fn build_latest_time(form: &WhyForm, columns: &[ProfileColumn]) -> Option<FormTime> {
    let has_compare = form
        .compare
        .as_deref()
        .is_some_and(|t| !t.trim().is_empty());
    if form.groups.is_some() || has_compare {
        return None;
    }
    if let Some(time) = &form.time {
        return Some(time.clone());
    }
    let times: Vec<&ProfileColumn> = columns
        .iter()
        .filter(|c| c.role == ProfileRole::Time.as_str())
        .collect();
    let [column] = times.as_slice() else {
        return None;
    };
    Some(FormTime {
        column: column.name.clone(),
        grain: Grain::Month,
    })
}

/// Đếm dòng theo kỳ trên cột thời gian, áp các bộ lọc "=" của form để "Tại sao doanh thu Europe giảm" chọn kỳ có
/// dòng Europe. Lấy thêm một kỳ so với LATEST_OPTIONS để kỳ cũ nhất trong phương án vẫn so được số dòng với kỳ trước.
pub fn build_period_rows_sql(
    table: &str,
    column: &ProfileColumn,
    grain: Grain,
    form: &WhyForm,
) -> Option<String> {
    let time = build_time_expr(column)?;
    let mut conditions = vec![format!("{time} IS NOT NULL")];
    conditions.extend(
        form.filters
            .iter()
            .filter(|f| f.op == "=")
            .map(|f| build_equals(&f.column, &f.value).to_string()),
    );
    let unit = build_string(grain.as_str());
    Some(format!(
        "SELECT strftime(date_trunc({unit}, {time}), '%Y-%m-%d') AS period_start, COUNT(*) AS row_count \
         FROM {} WHERE {} GROUP BY 1 ORDER BY 1 DESC LIMIT {}",
        build_ident(table),
        conditions.join(" AND "),
        LATEST_OPTIONS + 1
    ))
}

/// Kỳ gần nhất có đủ dữ liệu, từ số dòng theo kỳ (mới nhất trước). Quy tắc: kỳ cuối có dòng, trừ khi nó có chưa tới
/// một nửa số dòng của kỳ liền trước nó trong danh sách: khi đó dữ liệu dừng giữa kỳ (xuất file giữa tháng), so kỳ
/// dở dang với kỳ đủ thì phần lớn chênh lệch chỉ là ngày chưa có, nên lùi một kỳ. Chỉ lùi một lần: kỳ trước đó đã là
/// kỳ đủ theo chính quy tắc này. None khi không kỳ nào có dòng.
pub fn build_latest_compare(rows: &[PeriodRows]) -> Option<Period> {
    match rows {
        [last, before, ..] if last.rows * 2 < before.rows => Some(before.period),
        [last, ..] => Some(last.period),
        [] => None,
    }
}

/// Chạy câu đếm dòng rồi chọn kỳ. chosen là phương án why:compare người hỏi gửi: đọc được theo grain thì dùng nó,
/// không thì code chọn. None khi form không cần chọn kỳ, hay câu đếm không chạy được (để kiểm form báo thiếu compare
/// như cũ, vòng sửa của mô hình vẫn còn).
pub async fn find_latest(
    env: &QueryEnv<'_>,
    table: &str,
    columns: &[ProfileColumn],
    form: &WhyForm,
    chosen: Option<&str>,
) -> Result<Option<LatestPeriod>, AppError> {
    let Some(time) = build_latest_time(form, columns) else {
        return Ok(None);
    };
    let Some(column) = columns.iter().find(|c| c.name == time.column) else {
        return Ok(None);
    };
    let Some(sql) = build_period_rows_sql(table, column, time.grain, form) else {
        return Ok(None);
    };
    let rows = match run_code_query(env, &sql, None).await? {
        CodeQuery::Ran { result, .. } => read_period_rows(time.grain, &result.rows),
        CodeQuery::Refused(reason) | CodeQuery::Failed(reason) => {
            tracing::warn!(%reason, "period rows query failed");
            return Ok(None);
        }
    };
    let chosen = chosen.and_then(|text| Period::parse(time.grain, text).ok());
    let (compare, by_user) = match (chosen, build_latest_compare(&rows)) {
        (Some(period), _) => (period, true),
        (None, Some(period)) => (period, false),
        (None, None) => return Ok(None),
    };
    Ok(Some(LatestPeriod {
        time,
        compare,
        options: rows.iter().take(LATEST_OPTIONS).map(|r| r.period).collect(),
        chosen: by_user,
    }))
}

// Dòng kết quả là (ngày đầu kỳ dạng chữ, số dòng); số đếm tới dạng chuỗi chữ số như mọi ô BIGINT. Dòng đọc không được
// thì bỏ qua: thiếu một kỳ chỉ làm phương án ngắn đi.
fn read_period_rows(grain: Grain, rows: &[Vec<Option<CellValue>>]) -> Vec<PeriodRows> {
    rows.iter()
        .filter_map(|row| {
            let (Some(Some(CellValue::String(day))), Some(Some(count))) = (row.first(), row.get(1))
            else {
                return None;
            };
            let date = chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").ok()?;
            let rows = match count {
                CellValue::String(text) => text.parse().ok()?,
                CellValue::Number(n) => *n as i64,
                CellValue::Boolean(_) => return None,
            };
            Some(PeriodRows {
                period: Period::containing(grain, date),
                rows,
            })
        })
        .collect()
}

/// Ghi kỳ đã chọn vào form và vào object JSON lưu cùng câu hỏi, để kiểm form, SQL và phần lưu lại cùng một kỳ.
pub fn apply_latest(read: &mut ReadForm, latest: &LatestPeriod) {
    read.form.time = Some(latest.time.clone());
    read.form.compare = Some(latest.compare.text());
    if let Some(object) = read.json.as_object_mut() {
        object.insert(
            "time".to_string(),
            serde_json::json!({ "column": latest.time.column, "grain": latest.time.grain }),
        );
        object.insert(
            "compare".to_string(),
            serde_json::Value::String(latest.compare.text()),
        );
    }
}

/// Lựa chọn kỳ cần giải thích trên thẻ "Cách tính". Nhãn nói rõ đây là giả định của code ("Kỳ gần nhất có dữ
/// liệu") khi người hỏi chưa chọn; phương án là các kỳ gần nhất có dữ liệu, cộng kỳ đang dùng nếu nó không nằm trong đó.
pub fn build_compare_choice(latest: &LatestPeriod, base: Period) -> Choice {
    let mut options = latest.options.clone();
    if !options.contains(&latest.compare) {
        options.push(latest.compare);
    }
    let lead = if latest.chosen {
        "Kỳ đã chọn"
    } else {
        "Kỳ gần nhất có dữ liệu"
    };
    Choice {
        id: COMPARE_CHOICE_ID.to_string(),
        label: format!("{lead}: {} so với {}", latest.compare.label(), base.label()),
        selected: Some(latest.compare.text()),
        options: options
            .into_iter()
            .map(|period| ChoiceOption {
                id: period.text(),
                label: period.label(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quarter(text: &str) -> Period {
        Period::parse(Grain::Quarter, text).unwrap()
    }

    fn rows(list: &[(&str, i64)]) -> Vec<PeriodRows> {
        list.iter()
            .map(|(text, rows)| PeriodRows {
                period: quarter(text),
                rows: *rows,
            })
            .collect()
    }

    #[test]
    fn latest_period_with_rows_is_chosen() {
        let list = rows(&[("2017-Q4", 900), ("2017-Q3", 1000), ("2017-Q2", 950)]);
        assert_eq!(build_latest_compare(&list), Some(quarter("2017-Q4")));
    }

    // Quý 4 chỉ có 300 dòng so với 1000 của quý 3: dữ liệu dừng giữa quý, lùi về quý 3.
    #[test]
    fn partial_last_period_steps_back_once() {
        let list = rows(&[("2018-Q1", 300), ("2017-Q4", 1000), ("2017-Q3", 100)]);
        assert_eq!(build_latest_compare(&list), Some(quarter("2017-Q4")));
        let list = rows(&[("2017-Q4", 499), ("2017-Q3", 1000)]);
        assert_eq!(build_latest_compare(&list), Some(quarter("2017-Q3")));
    }

    #[test]
    fn no_rows_has_no_period() {
        assert_eq!(build_latest_compare(&[]), None);
    }

    #[test]
    fn period_containing_a_day() {
        let day = chrono::NaiveDate::from_ymd_opt(2017, 11, 15).unwrap();
        assert_eq!(Period::containing(Grain::Quarter, day), quarter("2017-Q4"));
        assert_eq!(Period::containing(Grain::Month, day).text(), "2017-11");
        assert_eq!(Period::containing(Grain::Year, day).text(), "2017");
    }

    #[test]
    fn choice_label_names_both_periods() {
        let latest = LatestPeriod {
            time: FormTime {
                column: "Order Date".to_string(),
                grain: Grain::Quarter,
            },
            compare: quarter("2017-Q4"),
            options: vec![quarter("2017-Q4"), quarter("2017-Q3")],
            chosen: false,
        };
        let choice = build_compare_choice(&latest, quarter("2017-Q3"));
        assert_eq!(choice.id, COMPARE_CHOICE_ID);
        assert_eq!(
            choice.label,
            "Kỳ gần nhất có dữ liệu: Quý 4/2017 so với Quý 3/2017"
        );
        assert_eq!(choice.selected.as_deref(), Some("2017-Q4"));
        assert_eq!(choice.options.len(), 2);
    }
}
