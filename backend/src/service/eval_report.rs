// SPDX-License-Identifier: Apache-2.0
/* service/eval_report.rs - phần chọn câu và phần báo cáo của bộ đánh giá (#202), thuần không gọi mô hình hay database:
service/eval.rs dùng để dựng báo cáo, cli.rs dùng để chọn câu trước khi chạy và so với lần chạy trước.

- Tỷ lệ đúng theo loại câu, kèm khoảng tin cậy Wilson 95%: với vài chục câu mỗi loại, biên sai số rộng cỡ ±15 điểm,
  in kèm khoảng để không đọc chênh lệch 5 điểm giữa hai lần chạy thành cải thiện thật. Wilson thay cho khoảng
  p ± 1.96 sqrt(p(1-p)/n) vì khoảng đó co về 0 khi đạt 100% hay 0%, đúng lúc mẫu nhỏ nhất.
- Tập con: issue đo độ dao động bằng 3 lượt trên khoảng 30 câu; chọn xoay vòng theo loại để loại nào cũng có mặt,
  không lấy 30 câu đầu file (toàn câu why_groups). Câu tiếp nối được chọn thì kéo theo câu gốc của nó, nên tập con có
  thể nhiều hơn size một vài câu.
- Cách hỏi khác: mỗi câu có trường variants; bật thì mỗi cách hỏi thành một câu chấm riêng, cùng đáp án.
- Câu hồi quy: đạt ở lần chạy trước (file --out cũ), trượt ở lần này. Câu cần xem lại không tính, vì chưa chấm được. */

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::service::eval::{CaseOutcome, EvalCase, Verdict};

/// Loại ghi cho câu không có trường kind (bộ câu cũ eval/dataco.json, edu.json, sales_joins.json).
pub const NO_KIND: &str = "unknown";

/// z của khoảng tin cậy 95%.
const Z: f64 = 1.96;

/// Một dòng của bảng theo loại câu.
#[derive(Debug, Serialize, PartialEq)]
pub struct KindRow {
    pub kind: String,
    pub passed: usize,
    pub failed: usize,
    pub review: usize,
    /// passed / (passed + failed); 0 khi chưa chấm được câu nào.
    pub accuracy: f64,
    /// Khoảng tin cậy Wilson 95% của accuracy.
    pub low: f64,
    pub high: f64,
}

/// Tóm tắt một lượt khi chạy --repeat.
#[derive(Debug, Serialize)]
pub struct RunSummary {
    pub passed: usize,
    pub failed: usize,
    pub review: usize,
    pub accuracy: f64,
}

/// Bảng theo loại câu, theo thứ tự loại xuất hiện lần đầu trong báo cáo (thứ tự của file câu).
pub fn build_kind_rows(cases: &[CaseOutcome]) -> Vec<KindRow> {
    let mut order: Vec<&str> = Vec::new();
    let mut counts: HashMap<&str, [usize; 3]> = HashMap::new();
    for case in cases {
        let kind = case.kind.as_deref().unwrap_or(NO_KIND);
        let slot = counts.entry(kind).or_insert_with(|| {
            order.push(kind);
            [0; 3]
        });
        let index = match case.verdict {
            Verdict::Pass => 0,
            Verdict::Fail => 1,
            Verdict::Review => 2,
        };
        slot[index] += 1;
    }
    order
        .into_iter()
        .map(|kind| {
            let [passed, failed, review] = counts[kind];
            let graded = passed + failed;
            let (low, high) = build_wilson(passed, graded);
            KindRow {
                kind: kind.to_string(),
                passed,
                failed,
                review,
                accuracy: passed as f64 / graded.max(1) as f64,
                low,
                high,
            }
        })
        .collect()
}

/// Khoảng tin cậy Wilson 95% cho passed trên graded câu. Chưa chấm câu nào thì chưa biết gì: cả khoảng [0, 1].
pub fn build_wilson(passed: usize, graded: usize) -> (f64, f64) {
    if graded == 0 {
        return (0.0, 1.0);
    }
    let n = graded as f64;
    let p = passed as f64 / n;
    let z2 = Z * Z;
    let scale = 1.0 + z2 / n;
    let center = (p + z2 / (2.0 * n)) / scale;
    let half = Z * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt() / scale;
    ((center - half).max(0.0), (center + half).min(1.0))
}

/// Chọn size câu (cộng câu gốc của câu tiếp nối), xoay vòng giữa các loại (mỗi vòng lấy câu kế tiếp của từng loại), giữ thứ tự của file. Cùng file và
/// cùng size thì luôn ra cùng tập con, để các lần chạy so được với nhau.
pub fn build_subset(cases: Vec<EvalCase>, size: usize) -> Vec<EvalCase> {
    let mut groups: Vec<(Option<&str>, Vec<usize>)> = Vec::new();
    for (i, case) in cases.iter().enumerate() {
        let kind = case.kind.as_deref();
        match groups.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, members)) => members.push(i),
            None => groups.push((kind, vec![i])),
        }
    }
    let mut chosen = HashSet::new();
    let mut round = 0;
    while chosen.len() < size.min(cases.len()) {
        for (_, members) in &groups {
            if let Some(&i) = members.get(round) {
                if chosen.len() < size {
                    chosen.insert(i);
                }
            }
        }
        round += 1;
    }
    let parents: Vec<usize> = chosen
        .iter()
        .filter_map(|&i| cases[i].follow_up_of.as_deref())
        .filter_map(|parent| cases.iter().position(|c| c.id == parent))
        .collect();
    chosen.extend(parents);
    cases
        .into_iter()
        .enumerate()
        .filter(|(i, _)| chosen.contains(i))
        .map(|(_, case)| case)
        .collect()
}

/// Thêm mỗi cách hỏi khác thành một câu riêng ngay sau câu gốc, id có hậu tố ~v1, ~v2, cùng đáp án và cùng loại.
pub fn build_variant_cases(cases: Vec<EvalCase>) -> Vec<EvalCase> {
    let mut out = Vec::new();
    for case in cases {
        let variants: Vec<EvalCase> = case
            .variants
            .iter()
            .enumerate()
            .map(|(i, question)| EvalCase {
                id: format!("{}~v{}", case.id, i + 1),
                question: question.clone(),
                variants: Vec::new(),
                ..case.clone()
            })
            .collect();
        out.push(case);
        out.extend(variants);
    }
    out
}

/// Phần của báo cáo cũ cần để tìm câu hồi quy.
#[derive(Debug, Deserialize)]
pub struct PreviousCase {
    pub id: String,
    pub verdict: Verdict,
}

#[derive(Deserialize)]
struct PreviousReport {
    cases: Vec<PreviousCase>,
}

/// Đọc báo cáo của lần chạy trước (file --out). Báo cáo trước #202 không có verdict: lỗi parse, là lỗi hạ tầng.
pub fn load_previous(path: &Path) -> Result<Vec<PreviousCase>, AppError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| AppError::Internal(format!("read {}: {e}", path.display())))?;
    let report: PreviousReport = serde_json::from_str(&text)
        .map_err(|e| AppError::Internal(format!("parse {}: {e}", path.display())))?;
    Ok(report.cases)
}

/// Id các câu đạt ở mọi lượt của lần trước mà trượt ở ít nhất một lượt lần này (với --repeat, một id có nhiều dòng).
/// Câu mới chưa có ở lần trước không phải hồi quy.
pub fn build_regressions(previous: &[PreviousCase], now: &[CaseOutcome]) -> Vec<String> {
    let mut passed_before: HashMap<&str, bool> = HashMap::new();
    for case in previous {
        let all = passed_before.entry(case.id.as_str()).or_insert(true);
        *all &= case.verdict == Verdict::Pass;
    }
    let mut out: Vec<String> = Vec::new();
    for case in now {
        let regressed = case.verdict == Verdict::Fail
            && passed_before.get(case.id.as_str()) == Some(&true)
            && !out.contains(&case.id);
        if regressed {
            out.push(case.id.clone());
        }
    }
    out
}

/// Id các câu có kết quả khác nhau giữa các lượt của --repeat (chỉ xét đạt và trượt).
pub fn build_unstable(cases: &[CaseOutcome]) -> Vec<String> {
    let mut seen: HashMap<&str, Verdict> = HashMap::new();
    let mut out: Vec<String> = Vec::new();
    for case in cases.iter().filter(|c| c.verdict != Verdict::Review) {
        match seen.get(case.id.as_str()) {
            Some(&first) if first != case.verdict && !out.contains(&case.id) => {
                out.push(case.id.clone())
            }
            Some(_) => {}
            None => {
                seen.insert(case.id.as_str(), case.verdict);
            }
        }
    }
    out
}
