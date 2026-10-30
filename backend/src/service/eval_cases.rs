// SPDX-License-Identifier: Apache-2.0
/* service/eval_cases.rs - định dạng file bộ câu đánh giá (#202): mỗi file một dataset, mỗi câu ghi câu hỏi, các cách
hỏi khác, SQL đáp án, loại câu, câu có phải từ chối không, người viết và người duyệt. service/eval.rs đọc file qua
load_case_file rồi chạy từng câu. File nằm ở eval/cases/, định dạng mô tả ở eval/README.md.

Đọc xong thì kiểm cả file (check_case_file) trước khi hỏi câu nào: một lần chạy với Gemini tốn hạn mức và vài phút,
file sai (trùng id, câu tiếp nối trỏ tới câu không có, câu phải từ chối mà có đáp án) phải báo ngay chứ không phải sau
khi đã hỏi nửa bộ. Trường lạ cũng là lỗi (deny_unknown_fields): gõ nhầm "orderd" thì câu lặng lẽ thành không xét thứ
tự.

Hiện runner dùng question, expected_sql, ordered, must_refuse, mock. Các trường variants, follow_up_of, check_sql,
required_columns đã đọc và kiểm hình dạng, phần chạy và chấm theo chúng thuộc các bước sau của #202 (bộ chấm, runner).
Test ở service/eval_cases_tests.rs. */

use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// Một file bộ câu: dataset các câu hỏi trên, và các câu.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseFile {
    pub dataset: CaseDataset,
    pub cases: Vec<EvalCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseDataset {
    /// Tên dataset khi nạp, cũng là tên bảng logic mà SQL đáp án dùng.
    pub name: String,
    /// Đường dẫn file dữ liệu, tính từ gốc repo (lệnh eval chạy ở đó).
    pub file: String,
    /// File chỉ số (dạng data/samples/dataco.metrics.json) ghi vào dataset sau khi nạp. Câu "tại sao" nêu chỉ số theo
    /// tên ("doanh thu"); dataset không có chỉ số thì form bị từ chối và câu rơi về đường câu thường.
    #[serde(default)]
    pub metrics: Option<String>,
}

/// Loại câu, để báo cáo tách tỷ lệ đúng theo loại. Danh sách theo phần "Bộ câu" của #202.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseKind {
    /// Câu thường một bước: gộp theo một chiều, lọc, top N đơn giản.
    PlainSimple,
    /// So cùng kỳ năm trước.
    PlainSamePeriod,
    /// Top N trong từng nhóm.
    PlainTopNInGroup,
    /// Giá trị lọc gõ không dấu hay sai hoa thường.
    PlainNoDiacritics,
    /// Nối bảng dễ nhân dòng (database nhiều bảng).
    PlainJoinTrap,
    /// Câu "tại sao" so hai kỳ.
    WhyPeriods,
    /// Câu "tại sao" so hai nhóm.
    WhyGroups,
    /// Câu "tại sao" trên chỉ số tỷ lệ.
    WhyRatio,
    /// Câu tìm nguyên nhân không có chữ "vì sao" ("Điều gì khiến doanh thu giảm").
    WhyNoKeyword,
    /// Câu "tại sao" có tiền đề sai (hỏi giảm mà số tăng).
    WhyFalsePremise,
    /// Câu "tại sao" không nêu kỳ.
    WhyNoPeriod,
    /// Câu tiếp nối một câu khác trong cùng cuộc trò chuyện.
    FollowUp,
    /// Câu hệ thống phải từ chối: ghi dữ liệu, hỏi thứ dữ liệu không có.
    Refuse,
}

impl CaseKind {
    /// Câu tìm nguyên nhân: hệ thống phải trả lời bằng đường giải thích, không phải đường câu thường.
    pub fn is_why(self) -> bool {
        matches!(
            self,
            CaseKind::WhyPeriods
                | CaseKind::WhyGroups
                | CaseKind::WhyRatio
                | CaseKind::WhyNoKeyword
                | CaseKind::WhyFalsePremise
                | CaseKind::WhyNoPeriod
        )
    }
}

/// Một câu gốc của bộ đánh giá.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalCase {
    /// Duy nhất trong file, ví dụ "dc-why-01".
    pub id: String,
    pub kind: CaseKind,
    pub question: String,
    /// Các cách hỏi khác cùng nghĩa (văn nói, không dấu, đảo trật tự); mỗi cách là một lượt chấm, cùng đáp án.
    #[serde(default)]
    pub variants: Vec<String>,
    /// Id của câu đứng trước trong cùng cuộc trò chuyện; câu đó phải nằm trước trong file.
    #[serde(default)]
    pub follow_up_of: Option<String>,
    /// SQL đáp án trên tên bảng logic, khớp một câu là đạt. Rỗng khi câu phải từ chối.
    #[serde(default)]
    pub expected_sql: Vec<String>,
    /// SQL người duyệt viết riêng để tính lại đáp án; phải ra cùng kết quả với expected_sql đầu tiên.
    #[serde(default)]
    pub check_sql: Option<String>,
    /// Vị trí (từ 0) các cột của đáp án mà kết quả bắt buộc có. Rỗng là mọi cột.
    #[serde(default)]
    pub required_columns: Vec<usize>,
    /// So cả thứ tự dòng: câu top N, chuỗi thời gian có ORDER BY.
    #[serde(default)]
    pub ordered: bool,
    /// Đạt khi hệ thống từ chối (status rejected).
    #[serde(default)]
    pub must_refuse: bool,
    /// Mock có câu trả lời cho câu hỏi này; chạy với mock thì chỉ hỏi các câu này.
    #[serde(default)]
    pub mock: bool,
    /// Tài khoản GitHub của người viết.
    pub author: String,
    /// Tài khoản GitHub của người duyệt; #202 cần hai người duyệt độc lập.
    #[serde(default)]
    pub reviewers: Vec<String>,
}

/// Đọc và kiểm một file bộ câu. Lỗi đọc, JSON hỏng hay file sai luật là lỗi hạ tầng (mã thoát 2 ở cli), không phải
/// câu trả lời sai.
pub fn load_case_file(path: &Path) -> Result<CaseFile, AppError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| AppError::Internal(format!("read {}: {e}", path.display())))?;
    let file: CaseFile = serde_json::from_str(&text)
        .map_err(|e| AppError::Internal(format!("parse {}: {e}", path.display())))?;
    check_case_file(&file).map_err(|errors| {
        AppError::Internal(format!("{}:\n  {}", path.display(), errors.join("\n  ")))
    })?;
    Ok(file)
}

/// Mọi chỗ sai của file, mỗi chỗ một dòng có id của câu; rỗng thì Ok.
pub fn check_case_file(file: &CaseFile) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    if file.cases.is_empty() {
        errors.push("file has no cases".to_string());
    }
    let mut seen: HashSet<&str> = HashSet::new();
    for case in &file.cases {
        check_case(case, &seen, &mut errors);
        if !seen.insert(case.id.as_str()) {
            errors.push(format!("{}: duplicate id", case.id));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// earlier là id của các câu đứng trước câu này trong file.
fn check_case(case: &EvalCase, earlier: &HashSet<&str>, errors: &mut Vec<String>) {
    let id = &case.id;
    let mut fail = |message: &str| errors.push(format!("{id}: {message}"));
    if id.trim().is_empty() {
        fail("empty id");
    }
    if case.question.trim().is_empty() {
        fail("empty question");
    }
    if case.author.trim().is_empty() {
        fail("empty author");
    }
    let mut questions: HashSet<&str> = HashSet::from([case.question.trim()]);
    for variant in &case.variants {
        if variant.trim().is_empty() {
            fail("empty variant");
        } else if !questions.insert(variant.trim()) {
            fail(&format!("variant repeats a question: {variant}"));
        }
    }
    // Câu phải từ chối không có kết quả để so; câu khác thiếu đáp án thì không chấm được.
    match (case.must_refuse, case.expected_sql.is_empty()) {
        (true, false) => fail("must_refuse case has expected_sql"),
        (false, true) => fail("expected_sql is empty"),
        _ => {}
    }
    if case.expected_sql.iter().any(|sql| sql.trim().is_empty()) {
        fail("empty expected_sql entry");
    }
    if case.must_refuse && (case.check_sql.is_some() || !case.required_columns.is_empty()) {
        fail("must_refuse case has check_sql or required_columns");
    }
    if case.kind == CaseKind::Refuse && !case.must_refuse {
        fail("kind refuse needs must_refuse");
    }
    let unique: HashSet<usize> = case.required_columns.iter().copied().collect();
    if unique.len() != case.required_columns.len() {
        fail("required_columns repeats a column");
    }
    match &case.follow_up_of {
        Some(previous) if previous == id => fail("follow_up_of points to itself"),
        Some(previous) if !earlier.contains(previous.as_str()) => {
            fail(&format!("follow_up_of {previous} is not an earlier case"))
        }
        None if case.kind == CaseKind::FollowUp => fail("kind follow_up needs follow_up_of"),
        _ => {}
    }
    let mut reviewers: HashSet<&str> = HashSet::new();
    for reviewer in &case.reviewers {
        if reviewer == &case.author {
            fail("the author cannot review their own case");
        } else if !reviewers.insert(reviewer.as_str()) {
            fail(&format!("reviewer {reviewer} listed twice"));
        }
    }
}
