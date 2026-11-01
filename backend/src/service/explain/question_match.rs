// SPDX-License-Identifier: Apache-2.0
/* service/explain/question_match.rs - kiểm form câu hỏi nguyên nhân với chính chữ của câu hỏi, sau khi explain/check.rs
đã kiểm form với hồ sơ cột. Form đúng cột, đúng giá trị vẫn có thể trả lời một câu khác câu người hỏi:
- "Vì sao điểm trung bình khác nhau giữa môn AAA và GGG?": mô hình chỉ điền AAA, code so AAA với các môn còn lại, GGG
  biến mất khỏi câu trả lời. "Vì sao tỷ lệ bỏ học kỳ 2014J cao hơn kỳ 2014B?" cũng vậy.
- "tại sao AAA lại ít sinh viên đăng kí": một lượt mô hình so AAA với BBB, môn câu hỏi không nhắc tới.
- Độ đo không phải thứ câu hỏi hỏi: "tỷ lệ bỏ học" mà form đo trọng số bài.

Hai luật, đều chỉ đường sửa cụ thể để mô hình sửa trong một vòng (explain/run.rs):
1. Nhóm câu hỏi nêu tên (check_named_groups). Giá trị của cột nhóm (hay bí danh của giá trị) có trong câu hỏi, so theo từ
   trọn vẹn sau khi bỏ dấu, là nhóm câu hỏi nêu tên. Hai nhóm nêu tên thì form phải so đúng hai nhóm đó. Một nhóm nêu tên
   thì nhóm đó là nhóm đang so, và nhóm gốc phải để trống (các nhóm còn lại, explain/group_base.rs), không được là một
   nhóm câu hỏi không nhắc. Không nhóm nào hay từ ba nhóm trở lên thì không kiểm: luật không đoán được câu muốn gì.
2. Độ đo theo cột lượt phân loại nêu (check_measure_columns). Lượt phân loại (ask/classify.rs) nêu các cột câu hỏi cần,
   ngữ cảnh đánh dấu chúng là picked. Độ đo là một cột không được nêu, trong khi có cột được nêu mà form không dùng ở chỗ
   nào: hai lượt của mô hình hiểu câu khác nhau. Luật này chỉ chạy ở lượt form đầu: lượt sửa vẫn giữ ý cũ thì tin form,
   vì cột lượt phân loại nêu cũng có thể thừa.
Hàm thuần, test ở explain/question_match_tests.rs. */

use crate::service::ask::context::ColumnProfile;
use crate::service::ask::phrases::fold;
use crate::service::ask::values::ValueEntry;
use crate::service::explain::check::WhyProfile;
use crate::service::explain::form::{FieldError, WhyForm};

/// Lỗi khi nhóm của form không khớp các nhóm câu hỏi nêu tên (luật 1 ở đầu file).
pub fn check_named_groups(question: &str, form: &WhyForm, profile: &WhyProfile) -> Vec<FieldError> {
    let Some(groups) = &form.groups else {
        return Vec::new();
    };
    let named = find_named_values(question, &groups.column, profile);
    let base = groups
        .base
        .as_deref()
        .map(str::trim)
        .filter(|b| !b.is_empty());
    let column = &groups.column;
    match named.as_slice() {
        [first, second] => {
            let both_named = [groups.compare.as_str(), base.unwrap_or("")];
            if both_named.contains(&first.as_str()) && both_named.contains(&second.as_str()) {
                return Vec::new();
            }
            // Nhóm đang so giữ như form nếu nó là một trong hai nhóm câu hỏi nêu; không thì nhóm nêu trước.
            let (compare, other) = if groups.compare == *second {
                (second, first)
            } else {
                (first, second)
            };
            // Một bộ lọc trên chính cột nhóm ("2014J" là một kỳ học nên vào filters) bỏ mất nhóm kia khỏi dữ liệu.
            let filtered = form.filters.iter().any(|f| &f.column == column);
            let drop_filter = if filtered {
                format!(", and remove the filter on \"{column}\"")
            } else {
                String::new()
            };
            vec![FieldError::new(
                "groups.base",
                format!(
                    "the question names the two groups \"{first}\" and \"{second}\" of \"{column}\": compare exactly \
                     these two, set \"groups.compare\" to \"{compare}\" and \"groups.base\" to \"{other}\"{drop_filter}"
                ),
            )]
        }
        [only] => {
            let mut errors = Vec::new();
            if groups.compare != *only {
                errors.push(FieldError::new(
                    "groups.compare",
                    format!(
                        "the question asks about the group \"{only}\" of \"{column}\": set \"groups.compare\" to \
                         \"{only}\""
                    ),
                ));
            }
            if let Some(base) = base.filter(|b| b != only) {
                errors.push(FieldError::new(
                    "groups.base",
                    format!(
                        "the question names only \"{only}\" of \"{column}\", not \"{base}\": leave \"groups.base\" out \
                         to compare with the other groups"
                    ),
                ));
            }
            errors
        }
        _ => Vec::new(),
    }
}

/// Giá trị của cột column có trong câu hỏi, theo thứ tự xuất hiện. Giá trị nằm trọn trong một giá trị dài hơn cũng được
/// nêu thì bỏ ("Asia" trong "Pacific Asia").
fn find_named_values(question: &str, column: &str, profile: &WhyProfile) -> Vec<String> {
    let Some(found) = profile.find(column) else {
        return Vec::new();
    };
    let listed = profile
        .values
        .columns
        .iter()
        .chain(&profile.joined.values)
        .find(|v| v.column_id == found.id);
    let Some(listed) = listed else {
        return Vec::new();
    };
    find_named_entries(question, &listed.values)
}

/// Giá trị trong entries (so cả giá trị lẫn bí danh, theo từ trọn vẹn sau khi bỏ dấu) có trong câu hỏi, theo thứ tự
/// xuất hiện. Giá trị nằm trọn trong một giá trị dài hơn cũng được nêu thì bỏ. Lớp hồ sơ ngữ nghĩa dùng lại hàm này để
/// kiểm câu gợi ý nguyên nhân (service/semantic/questions.rs), nên câu gợi ý được đọc nhóm đúng như đường câu hỏi.
pub(crate) fn find_named_entries(question: &str, entries: &[ValueEntry]) -> Vec<String> {
    // Thêm dấu cách hai đầu để so theo ranh giới từ bằng một phép tìm chuỗi, như build_question_kind (explain/form.rs).
    let words = format!(" {} ", fold(question));
    let mut hits: Vec<(usize, String, String)> = Vec::new();
    for entry in entries {
        let texts = std::iter::once(&entry.value).chain(&entry.aliases);
        let first = texts
            .filter_map(|text| {
                let folded = fold(text);
                (!folded.is_empty())
                    .then(|| words.find(&format!(" {folded} ")).map(|at| (at, folded)))
                    .flatten()
            })
            .min_by_key(|(at, _)| *at);
        if let Some((at, folded)) = first {
            hits.push((at, folded, entry.value.clone()));
        }
    }
    let inside_longer = |folded: &str| {
        hits.iter()
            .any(|(_, other, _)| other.len() > folded.len() && other.contains(folded))
    };
    let mut kept: Vec<(usize, String)> = hits
        .iter()
        .filter(|(_, folded, _)| !inside_longer(folded))
        .map(|(at, _, value)| (*at, value.clone()))
        .collect();
    kept.sort();
    kept.into_iter().map(|(_, value)| value).collect()
}

/// Lỗi khi độ đo là cột lượt phân loại không nêu mà một cột được nêu bị bỏ không (luật 2 ở đầu file). context là cột
/// ngữ cảnh của bảng bắt đầu, có cờ picked. Form dùng chỉ số thì không kiểm: chỉ số do người hỏi hay admin đặt tên.
pub fn check_measure_columns(form: &WhyForm, context: &[ColumnProfile]) -> Vec<FieldError> {
    let has_metric = form.metric.as_deref().is_some_and(|m| !m.trim().is_empty());
    let Some(measure) = form.measure.as_ref().filter(|_| !has_metric) else {
        return Vec::new();
    };
    let picked = |name: &str| context.iter().any(|c| c.picked && c.name == name);
    if picked(&measure.column) {
        return Vec::new();
    }
    let mut used: Vec<&str> = form.filters.iter().map(|f| f.column.as_str()).collect();
    used.extend(form.time.iter().map(|t| t.column.as_str()));
    used.extend(form.groups.iter().map(|g| g.column.as_str()));
    used.push(&measure.column);
    // Chỉ cột có thể là độ đo: cột số đo, hay cột chiều cho tỷ lệ dòng có một giá trị (share). Cột thời gian, cột khóa
    // được nêu mà không dùng thường chỉ là cột lọc hay cột nối.
    let unused: Vec<&ColumnProfile> = context
        .iter()
        .filter(|c| c.picked && !used.contains(&c.name.as_str()))
        .filter(|c| c.role == "measure" || c.role == "dimension")
        .collect();
    let Some(first) = unused.first() else {
        return Vec::new();
    };
    let names: Vec<String> = unused.iter().map(|c| format!("\"{}\"", c.name)).collect();
    let how = if first.role == "dimension" {
        format!(
            "for a rate of rows with one value of \"{}\", set \"measure\" to {{\"column\": \"{}\", \"agg\": \"share\", \
             \"value\": <that value>}}",
            first.name, first.name
        )
    } else {
        format!(
            "measure \"{}\" with the aggregation the question asks",
            first.name
        )
    };
    vec![FieldError::new(
        "measure.column",
        format!(
            "the question is about {}, which the form does not use, while the measure \"{}\" is not a column the \
             question names; measure what the question asks: {how}",
            names.join(", "),
            measure.column
        ),
    )]
}
