// SPDX-License-Identifier: Apache-2.0
/* service/semantic/questions.rs - kiểm câu gợi ý của mô hình bằng code trước khi lưu (mod.rs, profile_version gọi sau
build_profile_answer). Câu gợi ý hiện trên trang hỏi, bấm vào là hỏi luôn; câu không trả lời được làm người dùng mất
lòng tin ngay câu đầu. Lỗi gặp ngày 01/11 trên edu.student_daily_clicks: "Vì sao số đơn ngày 10 thay đổi so với ngày
0", trong khi bảng ghi lượt bấm, và cột date là số ngày tính từ đầu học phần chứ không phải ngày.

Hai luật, đều chỉ đọc chữ của câu và hồ sơ cột, không gọi mô hình. Luật nghiêng về giữ câu: chỉ bỏ khi chắc câu sai.
1. Câu hỏi nguyên nhân (build_question_kind của explain/form.rs) phải tách được thành hai nhóm hay hai kỳ có thật:
   - Hai nhóm: đúng hai giá trị của cùng một cột dimension có trong câu, đọc bằng find_named_entries, đúng cách đường
     câu hỏi nguyên nhân đọc nhóm câu hỏi nêu tên (explain/question_match.rs).
   - Hai kỳ: kỳ viết dạng "năm 2017", "tháng 11/2016", "tháng 5 năm 2017", "quý 3/2017", "ngày 31/5/2017" hay năm đứng
     riêng (build_periods), mỗi kỳ chạm khoảng min, max của một cột time. Một kỳ cũng được khi kỳ liền trước nó cũng
     chạm khoảng đó, vì đường câu hỏi nguyên nhân so với kỳ liền trước khi câu chỉ nêu một kỳ. Câu có hai nhóm mà nêu
     thêm kỳ ngoài khoảng dữ liệu cũng bị bỏ.
2. Số dòng gọi bằng thứ bảng không ghi: "số <danh từ>" mà danh từ không có trong tên bảng, tên dataset, mô tả bảng, tên,
   mô tả, bí danh của cột hay giá trị của cột (kể cả mô tả, bí danh mô hình vừa viết trong cùng lượt). Chỉ kiểm khi có
   ít nhất một mô tả cột để đối chiếu: bảng chỉ có tên cột tiếng Anh thì chữ tiếng Việt của câu không có gì để so.
Hàm thuần, test ở cuối file. */

use std::collections::HashSet;

use chrono::NaiveDate;

use crate::engine::store::{
    ColumnValue, ProfileColumn, ProfileRole, SemanticColumnUpdate, SemanticValueUpdate,
};
use crate::service::ask::data_range::read_date;
use crate::service::ask::phrases::fold;
use crate::service::ask::values::ValueEntry;
use crate::service::explain::form::{build_question_kind, QuestionKind};
use crate::service::explain::period::{Grain, Period};
use crate::service::explain::question_match::find_named_entries;

/// Những gì luật kiểm cần: hồ sơ cột, giá trị, và phần mô hình vừa viết trong cùng lượt.
pub struct QuestionCheck<'a> {
    /// Tên dataset, tên bảng, mô tả bảng: chữ để đối chiếu danh từ của "số ...".
    pub table_texts: Vec<&'a str>,
    pub columns: &'a [ProfileColumn],
    pub values: &'a [ColumnValue],
    pub column_updates: &'a [SemanticColumnUpdate],
    pub value_updates: &'a [SemanticValueUpdate],
}

/// Từ đếm đứng ngay sau "số" mà không nói một dòng ghi gì ("số lượt bấm", "số lần nộp", "số lượng hàng"): bỏ qua để xét
/// từ kế tiếp.
const COUNTER_WORDS: &[&str] = &[
    "luot", "lan", "dong", "luong", "nguoi", "ca", "cai", "chiec",
];

/// Từ đứng trước "so" làm nó không phải "số" đếm dòng: "doanh số", "chỉ số", "tỷ số", "hệ số", "tần số", "điểm số",
/// "trọng số", "cơ sở" (bỏ dấu cùng thành "so"). Thiếu một từ ở đây thì câu như "doanh số trung bình theo tháng" bị bỏ nhầm.
const NOT_COUNT_BEFORE: &[&str] = &[
    "doanh", "chi", "ty", "ti", "he", "tan", "diem", "trong", "co",
];

/// Giữ các câu qua được luật ở đầu file, theo thứ tự cũ. Câu bị bỏ ghi log kèm lý do (câu gợi ý là chữ mô hình viết, chỉ
/// gồm tên cột và giá trị của cột dimension, không phải dòng dữ liệu).
pub fn build_checked_questions(questions: Vec<String>, check: &QuestionCheck) -> Vec<String> {
    questions
        .into_iter()
        .filter(|q| match check_question(q, check) {
            Ok(()) => true,
            Err(reason) => {
                tracing::info!(question = %q, reason, "suggested question dropped");
                false
            }
        })
        .collect()
}

/// Ok khi câu qua cả hai luật; Err kèm lý do (tiếng Anh, cho log).
pub fn check_question(question: &str, check: &QuestionCheck) -> Result<(), &'static str> {
    check_count_noun(question, check)?;
    if build_question_kind(question) == QuestionKind::Why {
        check_why_parts(question, check)?;
    }
    Ok(())
}

/// Luật 1: hai nhóm hay hai kỳ có thật.
fn check_why_parts(question: &str, check: &QuestionCheck) -> Result<(), &'static str> {
    let ranges = build_time_ranges(check.columns);
    let periods = build_periods(question);
    let inside = |p: Period| ranges.iter().any(|r| overlaps(p, *r));
    if names_two_groups(question, check) {
        // Năm đứng riêng có thể là giá trị của cột nhóm (cột năm dạng số), nên chỉ kiểm kỳ khi bảng có cột time.
        if !ranges.is_empty() && !periods.iter().all(|p| inside(*p)) {
            return Err("a period of the question is outside the data");
        }
        return Ok(());
    }
    if ranges.is_empty() {
        return Err("no two groups, and the table has no time column");
    }
    match periods.as_slice() {
        [] => Err("no two groups and no period"),
        [only] if inside(*only) && inside(only.previous()) => Ok(()),
        [_] => Err("one period, and it or the period before it is outside the data"),
        many if many.iter().all(|p| inside(*p)) && many.iter().any(|p| *p != many[0]) => Ok(()),
        _ => Err("a period is outside the data, or the two periods are the same"),
    }
}

/// Câu nêu đúng hai giá trị của cùng một cột dimension không nhạy cảm (giá trị hay bí danh, kể cả bí danh vừa viết).
fn names_two_groups(question: &str, check: &QuestionCheck) -> bool {
    check
        .columns
        .iter()
        .filter(|c| c.role == ProfileRole::Dimension.as_str() && !c.is_sensitive)
        .any(|c| {
            let entries = build_value_entries(c, check);
            find_named_entries(question, &entries).len() == 2
        })
}

/// Giá trị của một cột kèm bí danh đã có và bí danh mô hình vừa viết.
fn build_value_entries(column: &ProfileColumn, check: &QuestionCheck) -> Vec<ValueEntry> {
    check
        .values
        .iter()
        .filter(|v| v.column_id == column.id)
        .map(|v| {
            let mut aliases = v.aliases.clone();
            for update in check.value_updates {
                if update.column_id == column.id && update.value == v.value {
                    aliases.extend(update.aliases.iter().cloned());
                }
            }
            ValueEntry {
                value: v.value.clone(),
                aliases,
            }
        })
        .collect()
}

/// Khoảng [ngày đầu, ngày cuối] của mỗi cột time không nhạy cảm có min, max đọc được.
fn build_time_ranges(columns: &[ProfileColumn]) -> Vec<(NaiveDate, NaiveDate)> {
    columns
        .iter()
        .filter(|c| c.role == ProfileRole::Time.as_str() && !c.is_sensitive)
        .filter_map(|c| {
            let from = read_date(c.min_value.as_deref()?)?;
            let to = read_date(c.max_value.as_deref()?)?;
            Some((from, to))
        })
        .collect()
}

/// Kỳ có ít nhất một ngày nằm trong khoảng: kỳ chưa trọn (tháng cuối của dữ liệu) vẫn hỏi được.
fn overlaps(period: Period, (from, to): (NaiveDate, NaiveDate)) -> bool {
    period
        .range()
        .is_some_and(|(start, end)| start <= to && end > from)
}

/// Các kỳ viết trong câu, theo thứ tự (xem đầu file). Chữ đã bỏ dấu và tách từ bằng fold, nên "tháng 11/2016" thành
/// "thang 11 2016". "Ngày 10" không có tháng, năm thì không phải kỳ.
pub fn build_periods(question: &str) -> Vec<Period> {
    let folded = fold(question);
    let words: Vec<&str> = folded.split(' ').collect();
    let mut periods = Vec::new();
    let mut i = 0;
    while i < words.len() {
        match read_period(&words[i..]) {
            Some((period, used)) => {
                periods.push(period);
                i += used;
            }
            None => i += 1,
        }
    }
    periods
}

/// Một kỳ ở đầu dãy từ, kèm số từ đã dùng.
fn read_period(words: &[&str]) -> Option<(Period, usize)> {
    match words {
        ["ngay", d, m, y, ..] if number_in(d, 1, 31) && number_in(m, 1, 12) && is_year(y) => {
            let text = format!("{y}-{:02}-{:02}", number(m)?, number(d)?);
            Some((Period::parse(Grain::Day, &text).ok()?, 4))
        }
        ["thang", m, "nam", y, ..] if number_in(m, 1, 12) && is_year(y) => {
            month_period(y, m).map(|p| (p, 4))
        }
        ["thang", m, y, ..] if number_in(m, 1, 12) && is_year(y) => {
            month_period(y, m).map(|p| (p, 3))
        }
        ["quy", q, "nam", y, ..] if number_in(q, 1, 4) && is_year(y) => {
            quarter_period(y, q).map(|p| (p, 4))
        }
        ["quy", q, y, ..] if number_in(q, 1, 4) && is_year(y) => {
            quarter_period(y, q).map(|p| (p, 3))
        }
        ["nam", y, ..] if is_year(y) => Some((Period::parse(Grain::Year, y).ok()?, 2)),
        [y, ..] if is_year(y) => Some((Period::parse(Grain::Year, y).ok()?, 1)),
        _ => None,
    }
}

fn month_period(year: &str, month: &str) -> Option<Period> {
    Period::parse(Grain::Month, &format!("{year}-{:02}", number(month)?)).ok()
}

fn quarter_period(year: &str, quarter: &str) -> Option<Period> {
    Period::parse(Grain::Quarter, &format!("{year}-Q{}", number(quarter)?)).ok()
}

fn number(word: &str) -> Option<u32> {
    if word.is_empty() || word.len() > 2 || !word.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    word.parse().ok()
}

fn number_in(word: &str, min: u32, max: u32) -> bool {
    number(word).is_some_and(|n| (min..=max).contains(&n))
}

/// Năm bốn chữ số trong khoảng dữ liệu thật hay gặp; "2014j" (mã kỳ học) không phải năm.
fn is_year(word: &str) -> bool {
    word.len() == 4
        && word.chars().all(|c| c.is_ascii_digit())
        && word
            .parse::<u32>()
            .is_ok_and(|y| (1900..=2100).contains(&y))
}

/// Luật 2: danh từ của "số ..." phải là thứ bảng ghi.
fn check_count_noun(question: &str, check: &QuestionCheck) -> Result<(), &'static str> {
    let has_description = check.columns.iter().any(|c| c.description.is_some())
        || check.column_updates.iter().any(|u| u.description.is_some());
    if !has_description {
        return Ok(());
    }
    let folded = fold(question);
    let words: Vec<&str> = folded.split(' ').collect();
    let Some(noun) = find_count_noun(&words) else {
        return Ok(());
    };
    if build_vocabulary(check).contains(noun) {
        Ok(())
    } else {
        Err("the question counts something the table does not record")
    }
}

/// Từ đầu tiên sau "số" (bỏ qua COUNTER_WORDS), khi "số" là số đếm; None khi câu không đếm dòng.
fn find_count_noun<'w>(words: &[&'w str]) -> Option<&'w str> {
    let at = words
        .iter()
        .enumerate()
        .position(|(i, w)| *w == "so" && (i == 0 || !NOT_COUNT_BEFORE.contains(&words[i - 1])))?;
    words[at + 1..]
        .iter()
        .find(|w| !COUNTER_WORDS.contains(*w))
        .copied()
        .filter(|w| !w.chars().all(|c| c.is_ascii_digit()))
}

/// Mọi từ (đã bỏ dấu) của tên bảng, tên dataset, mô tả bảng, tên, mô tả, bí danh của cột và giá trị cột.
fn build_vocabulary(check: &QuestionCheck) -> HashSet<String> {
    let mut texts: Vec<&str> = check.table_texts.clone();
    for c in check.columns {
        texts.push(&c.name);
        texts.extend(c.description.as_deref());
        texts.extend(c.aliases.iter().map(String::as_str));
    }
    for u in check.column_updates {
        texts.extend(u.description.as_deref());
        texts.extend(u.aliases.iter().map(String::as_str));
    }
    for v in check.values {
        texts.push(&v.value);
        texts.extend(v.aliases.iter().map(String::as_str));
    }
    for u in check.value_updates {
        texts.extend(u.aliases.iter().map(String::as_str));
    }
    texts
        .iter()
        .flat_map(|t| fold(t).split(' ').map(str::to_string).collect::<Vec<_>>())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn column(name: &str, data_type: &str, role: ProfileRole) -> ProfileColumn {
        ProfileColumn {
            id: Uuid::new_v4(),
            position: 0,
            name: name.to_string(),
            data_type: data_type.to_string(),
            role: role.as_str().to_string(),
            description: None,
            description_source: None,
            aliases: Vec::new(),
            is_sensitive: false,
            null_count: Some(0),
            distinct_count: Some(3),
            min_value: None,
            max_value: None,
            time_format: None,
            unit: None,
            aggregation: "sum".to_string(),
        }
    }

    fn value(column: &ProfileColumn, text: &str) -> ColumnValue {
        ColumnValue {
            column_id: column.id,
            value: text.to_string(),
            aliases: Vec::new(),
            frequency: Some(1),
        }
    }

    fn check<'a>(columns: &'a [ProfileColumn], values: &'a [ColumnValue]) -> QuestionCheck<'a> {
        QuestionCheck {
            table_texts: vec!["orders"],
            columns,
            values,
            column_updates: &[],
            value_updates: &[],
        }
    }

    #[test]
    fn periods_are_read_from_vietnamese_text() {
        let texts: Vec<String> = build_periods(
            "Vì sao tỷ lệ giao trễ tháng 11/2016 tăng so với tháng 10 năm 2016, quý 3/2017, năm 2015, ngày 31/5/2017",
        )
        .into_iter()
        .map(Period::text)
        .collect();
        assert_eq!(
            texts,
            ["2016-11", "2016-10", "2017-Q3", "2015", "2017-05-31"]
        );
        // Ngày tương đối và mã kỳ học không phải kỳ.
        assert!(build_periods("Vì sao số đơn ngày 10 thay đổi so với ngày 0").is_empty());
        assert!(build_periods("Vì sao tỷ lệ bỏ học kỳ 2014J cao hơn kỳ 2014B").is_empty());
    }

    /// Câu hỏi nguyên nhân giữ khi tách được hai kỳ trong khoảng dữ liệu hay hai nhóm có thật; bỏ khi không.
    #[test]
    fn why_questions_need_two_periods_or_two_groups() {
        let mut date = column("order_date", "TIMESTAMP", ProfileRole::Time);
        date.min_value = Some("2015-01-01 00:00:00".to_string());
        date.max_value = Some("2018-01-31 23:00:00".to_string());
        let market = column("market", "VARCHAR", ProfileRole::Dimension);
        let values = vec![value(&market, "Europe"), value(&market, "LATAM")];
        let columns = vec![date, market];
        let c = check(&columns, &values);
        for kept in [
            "Vì sao doanh thu tháng 11/2016 thay đổi so với tháng 10/2016",
            "Vì sao doanh thu tháng 5 năm 2017 thay đổi",
            "Vì sao doanh thu khác nhau giữa Europe và LATAM",
            "Vì sao doanh thu năm 2017 khác nhau giữa Europe và LATAM",
        ] {
            assert_eq!(check_question(kept, &c), Ok(()), "{kept}");
        }
        for dropped in [
            "Vì sao doanh thu tháng 5 năm 2020 thay đổi so với tháng 4 năm 2020",
            "Vì sao doanh thu tháng 1/2015 thay đổi",
            "Vì sao doanh thu khác nhau giữa Europe và Africa",
            "Vì sao doanh thu năm 2019 khác nhau giữa Europe và LATAM",
            "Vì sao doanh thu thay đổi",
        ] {
            assert!(check_question(dropped, &c).is_err(), "{dropped}");
        }
        // Câu thường không cần kỳ hay nhóm.
        assert_eq!(check_question("Doanh thu theo tháng", &c), Ok(()));
    }

    /// Lỗi gặp ngày 01/11: bảng lượt bấm, cột date là số ngày (vai trò text, không phải time), mô hình viết "số đơn".
    #[test]
    fn clicks_table_drops_the_order_count_question() {
        let mut day = column("date", "INTEGER", ProfileRole::Text);
        day.description = Some("Ngày, tính bằng số ngày từ ngày bắt đầu học phần".to_string());
        let mut clicks = column("sum_click", "INTEGER", ProfileRole::Measure);
        clicks.description = Some("Tổng số lần bấm trong ngày".to_string());
        let columns = vec![day, clicks];
        let c = QuestionCheck {
            table_texts: vec![
                "edu_demo",
                "student_daily_clicks",
                "Số lần bấm của sinh viên",
            ],
            ..check(&columns, &[])
        };
        assert!(check_question("Vì sao số đơn ngày 10 thay đổi so với ngày 0", &c).is_err());
        assert!(check_question("Số đơn theo ngày", &c).is_err());
        assert_eq!(
            check_question("Số lần bấm trung bình theo ngày", &c),
            Ok(())
        );
        assert_eq!(check_question("Số sinh viên theo ngày", &c), Ok(()));
        // "Chỉ số", "doanh số" không phải đếm dòng.
        assert_eq!(check_question("Chỉ số đơn giản theo ngày", &c), Ok(()));
        assert_eq!(check_question("Doanh số trung bình theo ngày", &c), Ok(()));
        assert_eq!(check_question("Điểm số trung bình theo ngày", &c), Ok(()));
        assert_eq!(check_question("Trọng số trung bình theo ngày", &c), Ok(()));
    }

    /// Không có mô tả cột nào để đối chiếu thì không kiểm danh từ: bảng tên cột tiếng Anh vẫn giữ "số đơn hàng".
    #[test]
    fn count_noun_is_not_checked_without_descriptions() {
        let columns = vec![column("Order Id", "BIGINT", ProfileRole::Key)];
        assert_eq!(
            check_question("Số đơn hàng theo tháng", &check(&columns, &[])),
            Ok(())
        );
    }
}
