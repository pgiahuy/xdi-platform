// SPDX-License-Identifier: Apache-2.0
/* service/report/auto.rs - kế hoạch của báo cáo tự động: chọn độ đo chính, cột thời gian, hai cột chiều, rồi dựng câu
SQL của từng phần. Mọi câu do code dựng từ hồ sơ cột, quan hệ khóa ngoại và bảng metrics, không do mô hình viết; mô
hình chỉ diễn đạt lại các ý code đã tính ở bước sau (auto_run.rs, qua service/insight/ với hai cổng kiểm như câu hỏi).

Ba phần, theo thứ tự:
- Độ đo theo tháng (hay theo ngày khi dữ liệu ngắn): service/insight/ tìm xu hướng, kỳ đổi mạnh nhất so với kỳ liền
  trước và phân rã đúng kỳ đó theo các chiều (như phần giải thích của câu thường, #227).
- Độ đo theo chiều A và theo chiều B: nhóm đứng đầu chiếm bao nhiêu phần tổng, cần mấy nhóm để đạt 80%, nhóm lệch hẳn
  mức trung bình.

Độ đo chính chọn như trang Tổng quan (overview/sql.rs, choose_measure): chỉ số đã xác nhận đầu tiên, chưa có thì cột số
đo đơn vị tiền rồi số lượng. Bảng chính là bảng đầu tiên có độ đo như vậy, ưu tiên bảng có chỉ số đã xác nhận. Cột thời
gian và cột chiều lấy ở bảng chính hay ở một bảng nó trỏ tới bằng khóa ngoại nhiều-một (một bước), để câu nối không cộng
lặp (ask/fanout.rs). Chiều A là cột chiều có nhiều nhóm nhất mà không quá 12 nhóm (biểu đồ cột vẫn đọc được từng nhóm),
chiều B là cột còn lại có nhiều nhóm nhất mà không quá 30 nhóm (đủ nhóm để thấy nhóm nào lệch mức trung bình). Cột
nhạy cảm không dùng: tên nhóm sẽ đi vào bảng kết quả và prompt viết lời. Test ở report/tests.rs. */

use uuid::Uuid;

use crate::engine::store::{DatasetRelation, MetricDefinition, ProfileColumn, ProfileRole};
use crate::service::explain::sql::{build_ident, build_string, build_time_expr};
use crate::service::overview::contract::TimeGrain;
use crate::service::overview::sql::{
    build_grain, build_label, choose_measure, choose_time_column, MeasurePlan,
};

/// Chiều A: nhiều nhóm nhất mà không quá số này.
const FIRST_DIMENSION_MAX_GROUPS: i64 = 12;
/// Chiều B: nhiều nhóm nhất mà không quá số này.
const SECOND_DIMENSION_MAX_GROUPS: i64 = 30;

/// Một bảng của dataset, đã có phiên bản hiện hành lập hồ sơ xong.
pub struct AutoTable<'a> {
    pub id: Uuid,
    pub logical_table: &'a str,
    pub columns: &'a [ProfileColumn],
    pub metrics: &'a [MetricDefinition],
}

/// Các phần dựng được, bảng chính và lý do của phần không dựng được.
#[derive(Debug, Default)]
pub struct AutoPlan {
    /// Bảng chính (bảng của độ đo); None khi không có độ đo nào.
    pub table_id: Option<Uuid>,
    pub measure: String,
    pub sections: Vec<AutoSection>,
    /// Mỗi phần không dựng được một câu tiếng Việt.
    pub skipped: Vec<String>,
}

/// Một phần: câu hỏi hiện làm tiêu đề và câu SQL trên tên bảng logic, trước guard.
#[derive(Debug, Clone, PartialEq)]
pub struct AutoSection {
    pub question: String,
    pub sql: String,
}

/// Một cột dùng được trong câu, kèm bảng chứa nó và phép nối cần để đọc nó (None khi ở bảng chính).
struct PlacedColumn<'a> {
    table: &'a str,
    column: &'a ProfileColumn,
    join: Option<String>,
}

pub fn build_auto_plan(tables: &[AutoTable<'_>], relations: &[DatasetRelation]) -> AutoPlan {
    let Some((main, measure)) = choose_main_table(tables) else {
        return AutoPlan {
            skipped: vec![
                "Dataset chưa có chỉ số đã xác nhận hay cột số đo đơn vị tiền, số lượng nên chưa có gì để tính."
                    .to_string(),
            ],
            ..AutoPlan::default()
        };
    };
    let parents = build_parents(main, tables, relations);
    let mut plan = AutoPlan {
        table_id: Some(main.id),
        measure: measure.main.label.clone(),
        ..AutoPlan::default()
    };
    match choose_time(main, &parents, &measure) {
        Some(time) => plan.sections.push(build_time_section(main, &measure, &time)),
        None => plan.skipped.push(format!(
            "Không có cột thời gian ở bảng {} hay bảng nó trỏ tới, nên không có phần {} theo thời gian.",
            main.logical_table, measure.main.label
        )),
    }
    let dimensions = choose_dimensions(main, &parents);
    if dimensions.is_empty() {
        plan.skipped.push(format!(
            "Không có cột chiều từ 2 tới {SECOND_DIMENSION_MAX_GROUPS} nhóm nên không có phần {} theo nhóm.",
            measure.main.label
        ));
    }
    for dimension in dimensions {
        plan.sections
            .push(build_group_section(main, &measure, &dimension));
    }
    plan
}

/// Bảng đầu tiên có chỉ số đã xác nhận, không có thì bảng đầu tiên có cột số đo dùng được.
fn choose_main_table<'a, 'b>(
    tables: &'a [AutoTable<'b>],
) -> Option<(&'a AutoTable<'b>, MeasurePlan)> {
    let with_metric = tables.iter().find_map(|t| {
        t.metrics
            .iter()
            .any(|m| m.confirmed)
            .then(|| choose_measure(t.metrics, t.columns).map(|m| (t, m)))
            .flatten()
    });
    with_metric.or_else(|| {
        tables
            .iter()
            .find_map(|t| choose_measure(&[], t.columns).map(|m| (t, m)))
    })
}

/// Bảng mà bảng chính trỏ tới bằng khóa ngoại (bảng chính ở phía nhiều), kèm phép nối viết sẵn.
fn build_parents<'a, 'b>(
    main: &AutoTable<'b>,
    tables: &'a [AutoTable<'b>],
    relations: &[DatasetRelation],
) -> Vec<(&'a AutoTable<'b>, String)> {
    relations
        .iter()
        .filter(|r| r.from_dataset_id == main.id && r.from_columns.len() == r.to_columns.len())
        .filter_map(|r| {
            let parent = tables.iter().find(|t| t.id == r.to_dataset_id)?;
            let conditions: Vec<String> = r
                .from_columns
                .iter()
                .zip(&r.to_columns)
                .map(|(from, to)| {
                    format!(
                        "{}.{} = {}.{}",
                        build_ident(main.logical_table),
                        build_ident(from),
                        build_ident(parent.logical_table),
                        build_ident(to)
                    )
                })
                .collect();
            let join = format!(
                "JOIN {} ON {}",
                build_ident(parent.logical_table),
                conditions.join(" AND ")
            );
            Some((parent, join))
        })
        .collect()
}

/// Cột thời gian của độ đo ở bảng chính; không có thì cột thời gian đầu tiên của một bảng cha.
fn choose_time<'a>(
    main: &'a AutoTable<'_>,
    parents: &[(&'a AutoTable<'_>, String)],
    measure: &MeasurePlan,
) -> Option<PlacedColumn<'a>> {
    if let Some(column) = choose_time_column(main.columns, Some(measure)) {
        return Some(PlacedColumn {
            table: main.logical_table,
            column,
            join: None,
        });
    }
    parents.iter().find_map(|(parent, join)| {
        choose_time_column(parent.columns, None).map(|column| PlacedColumn {
            table: parent.logical_table,
            column,
            join: Some(join.clone()),
        })
    })
}

/// Chiều A và chiều B (luật ở đầu file); ít cột hợp thì ít phần hơn.
fn choose_dimensions<'a>(
    main: &'a AutoTable<'_>,
    parents: &[(&'a AutoTable<'_>, String)],
) -> Vec<PlacedColumn<'a>> {
    let mut candidates: Vec<PlacedColumn<'a>> = Vec::new();
    let mut add = |table: &'a AutoTable<'_>, join: Option<&String>| {
        for column in table.columns.iter().filter(|c| is_group_column(c)) {
            candidates.push(PlacedColumn {
                table: table.logical_table,
                column,
                join: join.cloned(),
            });
        }
    };
    add(main, None);
    for (parent, join) in parents {
        add(parent, Some(join));
    }
    let groups = |c: &PlacedColumn| c.column.distinct_count.unwrap_or(0);
    // max_by_key giữ cột cuối khi bằng nhau; duyệt ngược để cột đứng trước (bảng chính, vị trí nhỏ) thắng.
    let first = candidates
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, c)| groups(c) <= FIRST_DIMENSION_MAX_GROUPS)
        .max_by_key(|(_, c)| groups(c))
        .map(|(i, _)| i);
    let second = candidates
        .iter()
        .enumerate()
        .rev()
        .filter(|(i, _)| Some(*i) != first)
        .max_by_key(|(_, c)| groups(c))
        .map(|(i, _)| i);
    let mut picked: Vec<usize> = first.into_iter().chain(second).collect();
    picked.sort_unstable();
    candidates
        .into_iter()
        .enumerate()
        .filter(|(i, _)| picked.contains(i))
        .map(|(_, c)| c)
        .collect()
}

/// Cột chiều không nhạy cảm, có từ 2 tới SECOND_DIMENSION_MAX_GROUPS giá trị khác nhau.
fn is_group_column(column: &ProfileColumn) -> bool {
    let groups = column.distinct_count.unwrap_or(0);
    column.role == ProfileRole::Dimension.as_str()
        && !column.is_sensitive
        && (2..=SECOND_DIMENSION_MAX_GROUPS).contains(&groups)
}

fn build_time_section(
    main: &AutoTable<'_>,
    measure: &MeasurePlan,
    time: &PlacedColumn<'_>,
) -> AutoSection {
    let time_ref = build_column_ref(time);
    let grain = build_section_grain(time.column);
    let (unit, period, word) = match grain {
        TimeGrain::Day => ("day", "Ngày", "ngày"),
        TimeGrain::Month => ("month", "Tháng", "tháng"),
    };
    let key = format!("date_trunc({}, {time_ref})", build_string(unit));
    let mut conditions = vec![format!("{time_ref} IS NOT NULL")];
    conditions.extend(measure.filter.iter().map(|f| format!("({f})")));
    let sql = format!(
        "SELECT {key} AS {period_ident}, {expr} AS {label} FROM {table}{join} WHERE {where_} \
         GROUP BY {period_ident} ORDER BY {period_ident}",
        period_ident = build_ident(period),
        expr = measure.expression,
        label = build_ident(&measure.main.label),
        table = build_ident(main.logical_table),
        join = build_join_text(time),
        where_ = conditions.join(" AND "),
    );
    AutoSection {
        question: format!("{} theo {word}", measure.main.label),
        sql,
    }
}

fn build_group_section(
    main: &AutoTable<'_>,
    measure: &MeasurePlan,
    dimension: &PlacedColumn<'_>,
) -> AutoSection {
    let dim_ref = build_column_ref(dimension);
    let dim_label = build_label(dimension.column);
    let filter = measure
        .filter
        .as_ref()
        .map(|f| format!(" WHERE ({f})"))
        .unwrap_or_default();
    let label = build_ident(&measure.main.label);
    let sql = format!(
        "SELECT {dim_ref} AS {dim_alias}, {expr} AS {label} FROM {table}{join}{filter} \
         GROUP BY {dim_ref} ORDER BY {label} DESC",
        dim_alias = build_ident(&dim_label),
        expr = measure.expression,
        table = build_ident(main.logical_table),
        join = build_join_text(dimension),
    );
    AutoSection {
        question: format!("{} theo {dim_label}", measure.main.label),
        sql,
    }
}

/// `"bảng"."cột"`, đổi chữ thành ngày bằng strptime khi cột thời gian lưu dạng chữ (như build_time_expr).
fn build_column_ref(placed: &PlacedColumn<'_>) -> String {
    let qualified = format!(
        "{}.{}",
        build_ident(placed.table),
        build_ident(&placed.column.name)
    );
    match &placed.column.time_format {
        Some(format) if placed.column.role == ProfileRole::Time.as_str() => {
            format!("strptime({qualified}, {})", build_string(format))
        }
        _ => qualified,
    }
}

fn build_join_text(placed: &PlacedColumn<'_>) -> String {
    placed
        .join
        .as_ref()
        .map(|join| format!(" {join}"))
        .unwrap_or_default()
}

/// Theo ngày khi khoảng dữ liệu của cột ngắn, theo tháng khi dài hay khi hồ sơ không có min, max đọc được.
fn build_section_grain(column: &ProfileColumn) -> TimeGrain {
    // choose_time_column chỉ trả cột build_time_expr đọc được; gọi lại để chắc khi hàm này được dùng chỗ khác.
    if build_time_expr(column).is_none() {
        return TimeGrain::Month;
    }
    let day = |text: &Option<String>| {
        text.as_deref()
            .and_then(|t| t.get(..10))
            .map(str::to_string)
    };
    match (day(&column.min_value), day(&column.max_value)) {
        (Some(start), Some(end)) => build_grain(&start, &end),
        _ => TimeGrain::Month,
    }
}
