// SPDX-License-Identifier: Apache-2.0
/* service/ask/data_range.rs - khoảng dữ liệu của câu trả lời (AskResponse.data_range): ngày đầu và ngày cuối của cột
thời gian mà câu trả lời dựa vào, để câu trả lời ghi dữ liệu có đến ngày nào khi bảng có cột thời gian. Người hỏi
"doanh thu tháng 6/2027" trên dữ liệu tới 31/01/2018 thấy ngay vì sao không có số; người hỏi kỳ chưa hết thấy kỳ đó mới
có dữ liệu tới đâu.

Lấy ngày từ hồ sơ cột (min_value, max_value do service/profile.rs ghi lúc nạp), không chạy thêm câu nào trên DuckDB. Hồ
sơ tính trên cả bảng, nên người hỏi bị lọc dòng theo vai dữ liệu vẫn thấy khoảng của cả bảng. Cột thời gian: cột đầu
tiên có tên trong câu SQL đã chạy, không có thì cột thời gian đầu tiên của bảng; cột nhạy cảm không dùng. Hàm thuần, test
ở ask/data_range_tests.rs. */

use chrono::NaiveDate;

use crate::engine::store::{ProfileColumn, ProfileRole};
use crate::service::ask::contract::DataRange;

/// None khi bảng không có cột thời gian dùng được (không có, nhạy cảm, hay hồ sơ chưa có ngày đầu, ngày cuối).
pub fn build_data_range(columns: &[ProfileColumn], sql: Option<&str>) -> Option<DataRange> {
    let usable: Vec<(&ProfileColumn, NaiveDate, NaiveDate)> = columns
        .iter()
        .filter(|c| c.role == ProfileRole::Time.as_str() && !c.is_sensitive)
        .filter_map(|c| {
            let from = read_date(c.min_value.as_deref()?)?;
            let to = read_date(c.max_value.as_deref()?)?;
            Some((c, from, to))
        })
        .collect();
    let in_sql = |c: &ProfileColumn| sql.is_some_and(|s| s.contains(&format!("\"{}\"", c.name)));
    let (column, from, to) = usable
        .iter()
        .find(|(c, _, _)| in_sql(c))
        .or_else(|| usable.first())?;
    Some(DataRange {
        column: column.name.clone(),
        label: column
            .aliases
            .iter()
            .map(|a| a.trim())
            .find(|a| !a.is_empty())
            .map(str::to_string),
        from: from.to_string(),
        to: to.to_string(),
        partial: sql.is_some_and(|s| asks_past(s, *to)),
    })
}

// "2015-01-01 00:00:00" hay "2015-01-01" thành ngày; chuỗi khác dạng thì None.
fn read_date(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text.get(..10)?, "%Y-%m-%d").ok()
}

// Câu SQL lọc tới quá ngày cuối của dữ liệu: có mốc ngày (dạng 'YYYY-MM-DD' trong câu) mà ngày liền trước nó vẫn sau
// ngày cuối. Mốc trên của khoảng thường là mốc loại trừ ("< TIMESTAMP '2018-02-01'" cho cả tháng 1), nên lùi một ngày
// trước khi so: hỏi tháng 1/2018 trên dữ liệu tới 31/01/2018 không bị coi là kỳ chưa đủ dữ liệu.
fn asks_past(sql: &str, last: NaiveDate) -> bool {
    sql.split('\'')
        .skip(1)
        .step_by(2)
        .filter_map(read_date)
        .filter_map(|date| date.pred_opt())
        .any(|covered| covered > last)
}
