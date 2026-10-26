// SPDX-License-Identifier: Apache-2.0
/* engine/llm/mock.rs - nhà cung cấp giả, chạy không cần khóa và dùng trong test.
Trả câu trả lời viết sẵn trong mock_answers.json theo loại việc và câu hỏi; câu khác thì báo cần cấu hình LLM thật.
Không đọc prompt (xem LlmRequest).

Câu mẫu viết trên dataset DataCo Smart Supply Chain (Mendeley Data, DOI 10.17632/8gx2fvg2k6.5, CC BY 4.0), bảng logic
dataco. File nhúng vào binary lúc biên dịch: image Docker chỉ chép backend/ khi build và binary cài bằng make install
không kèm thư mục data/, nên đọc file lúc chạy sẽ không có file để đọc.
Các câu giả định cột ngày "order date (DateOrders)" là chữ dạng "1/31/2018 22:56" (DuckDB đọc CSV của DataCo ra
VARCHAR) nên tự parse bằng strptime; hồ sơ (#22) hay bước seed (#25) đổi cột này sang kiểu ngày thì phải sửa các câu.
Tỷ lệ và số đếm về đơn tính theo đơn hàng (đếm "Order Id" phân biệt), không theo dòng: một đơn có nhiều dòng.
Câu "Doanh thu theo ngày giao hàng" cố ý đọc ngày theo ngày/tháng/năm nên DuckDB báo lỗi, và có câu sửa (task repair)
đọc đúng tháng/ngày/năm: để chạy thử vòng sửa (service/ask_run.rs) mà không cần mô hình thật. Ba câu nữa để thử các
nhánh còn lại của luồng hỏi (service/ask.rs) trên giao diện: "Xóa các đơn hàng bị hủy" viết câu DELETE nên guard từ
chối (rejected); "Doanh thu theo ngày đặt hàng" đọc sai định dạng ngày cả ở câu sửa (failed); "Doanh thu ở thị trường
châu Á" lọc Market = 'Asia', giá trị DataCo không có (cảnh báo filter_value_not_found). "Doanh thu theo tháng giảm so
với cùng kỳ năm trước" so từng tháng của năm mới nhất trong dữ liệu với năm liền trước (câu con lấy năm lớn nhất, nên
chạy được với cả DataCo đầy đủ lẫn bản nhỏ): hai tổng tiền theo 12 tháng, để thử luật biểu đồ vẽ hai đường trên một
thang đo (service/ask_chart.rs). Câu narrate cho câu
"tại sao" mẫu là lời diễn giải viết sẵn; mọi số trong đó có trong bảng phân rã mẫu (service/ask_examples.rs) nên qua
được bước kiểm số của #41.
Câu trả lời của task sql và repair là object {terms, assumptions, sql} như mô hình thật bị ép trả (service/ask_reply.rs);
terms ghi cách hiểu từng cụm chữ ("châu Âu" là Market = Europe) để luồng hỏi có dữ liệu thử phần kiểm cách hiểu. Câu
trả lời của task why là form câu "tại sao" (service/ask_why_prompt.rs); lần sửa form cũng là task why nên mock trả lại
đúng form đó. */

use std::collections::HashMap;

use async_trait::async_trait;
use serde::Deserialize;

use super::{LlmProvider, LlmRequest, LlmTask};
use crate::error::AppError;

const ANSWERS_JSON: &str = include_str!("mock_answers.json");

/// Một mục của mock_answers.json.
#[derive(Deserialize)]
struct Entry {
    task: LlmTask,
    question: String,
    /// Object JSON: {terms, assumptions, sql} với task sql và repair, form với task why, hồ sơ với task profile.
    answer: serde_json::Value,
}

pub struct Mock {
    /// Khóa là loại việc và câu hỏi đã chuẩn hóa.
    answers: HashMap<(LlmTask, String), String>,
}

impl Mock {
    pub fn new() -> Self {
        // File nhúng lúc biên dịch và test answers_file_is_valid đọc đúng file này, nên file hỏng thì CI đỏ trước khi
        // tới được máy chủ.
        let answers = parse_answers(ANSWERS_JSON).expect("mock_answers.json is invalid");
        Self { answers }
    }
}

#[async_trait]
impl LlmProvider for Mock {
    fn name(&self) -> String {
        "mock".to_string()
    }

    async fn complete(&self, request: &LlmRequest<'_>) -> Result<String, AppError> {
        let key = (request.task, normalize(request.question));
        self.answers.get(&key).cloned().ok_or_else(|| {
            AppError::BadRequest(
                "Chế độ mock chỉ trả lời các câu hỏi mẫu của dataset mẫu. Đặt LLM_PROVIDER=gemini hoặc ollama để hỏi tự do"
                    .to_string(),
            )
        })
    }
}

/// Đọc danh sách câu mẫu. Hai câu trùng nhau sau chuẩn hóa là lỗi: câu sau sẽ lặng lẽ đè câu trước.
fn parse_answers(json: &str) -> Result<HashMap<(LlmTask, String), String>, String> {
    let entries: Vec<Entry> =
        serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    let mut answers = HashMap::new();
    for entry in entries {
        // Mô hình thật luôn trả văn bản, nên object cũng trả dạng chuỗi JSON để người gọi xử lý như nhau.
        let answer = match entry.answer {
            serde_json::Value::String(text) => text,
            other => other.to_string(),
        };
        let key = (entry.task, normalize(&entry.question));
        if answers.insert(key, answer).is_some() {
            return Err(format!(
                "duplicate question after normalization: {}",
                entry.question
            ));
        }
    }
    Ok(answers)
}

/// Chuẩn hóa câu hỏi để so: bỏ dấu, chữ thường, gộp khoảng trắng, bỏ dấu câu cuối câu.
/// "Doanh thu theo  THÁNG?" và "doanh thu theo thang" thành cùng một chuỗi.
fn normalize(question: &str) -> String {
    // Gộp khoảng trắng trước khi bỏ dấu: deunicode bỏ hẳn tab và xuống dòng nằm sau một ký tự có dấu, nên
    // "tháng\ttheo" sẽ thành "thangtheo", dính hai từ.
    let spaced = join_words(question);
    let plain = deunicode::deunicode(&spaced).to_lowercase();
    // Gộp lần nữa: deunicode đổi khoảng trắng đặc biệt (như U+00A0) thành dấu cách thường, có thể thành hai dấu liền.
    join_words(&plain)
        .trim_end_matches(['?', '.', '!', ' '])
        .to_string()
}

fn join_words(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sql_request(question: &str) -> LlmRequest<'_> {
        LlmRequest {
            task: LlmTask::Sql,
            question,
            prompt: "",
            json_schema: None,
        }
    }

    #[test]
    fn answers_file_is_valid() {
        let answers = parse_answers(ANSWERS_JSON).unwrap();
        let count = |wanted: LlmTask| answers.keys().filter(|(task, _)| *task == wanted).count();
        assert_eq!(count(LlmTask::Sql), 15);
        assert_eq!(count(LlmTask::Repair), 2);
        assert_eq!(count(LlmTask::Narrate), 1);
        assert_eq!(count(LlmTask::Why), 3);
        let profile = &answers[&(LlmTask::Profile, "dataco".to_string())];
        let profile: serde_json::Value = serde_json::from_str(profile).unwrap();
        assert!(profile["columns"].is_array());
    }

    #[test]
    fn duplicate_after_normalization_is_rejected() {
        let json = r#"[
            {"task": "sql", "question": "Doanh thu theo tháng", "answer": "SELECT 1"},
            {"task": "sql", "question": "doanh thu theo thang?", "answer": "SELECT 2"}
        ]"#;
        let err = parse_answers(json).unwrap_err();
        assert!(err.contains("duplicate"), "{err}");
    }

    #[test]
    fn normalize_ignores_case_accents_spaces_and_end_punctuation() {
        assert_eq!(
            normalize("  Doanh THU theo   tháng?? "),
            normalize("doanh thu theo thang")
        );
        assert_eq!(normalize("Tỷ lệ giao trễ."), "ty le giao tre");
        // Tab, xuống dòng và khoảng trắng không ngắt (U+00A0) nằm sau chữ có dấu vẫn ngăn từ.
        assert_eq!(
            normalize("Doanh\u{a0}thu\ttheo\ntháng\r\nnay"),
            "doanh thu theo thang nay"
        );
    }

    /// Các mục của file, chưa chuẩn hóa, để test so với đúng câu viết trong file.
    fn file_entries() -> Vec<Entry> {
        serde_json::from_str(ANSWERS_JSON).unwrap()
    }

    /// Mọi tên cột trong dấu nháy kép của các câu SQL, và cột trong terms, phải có trong hồ sơ dataco: gõ sai tên cột
    /// thì test đỏ, không cần tải DataCo về máy.
    #[test]
    fn sql_answers_use_only_profile_columns() {
        let entries = file_entries();
        let profile = entries
            .iter()
            .find(|e| e.task == LlmTask::Profile && e.question == "dataco")
            .unwrap();
        let columns: Vec<&str> = profile.answer["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap())
            .collect();
        let sql_entries = entries
            .iter()
            .filter(|e| matches!(e.task, LlmTask::Sql | LlmTask::Repair));
        for entry in sql_entries {
            let sql = entry.answer["sql"].as_str().unwrap();
            // Tách theo dấu nháy kép: phần ở vị trí lẻ là phần nằm trong nháy.
            let quoted = sql.split('"').skip(1).step_by(2);
            let terms = entry.answer["terms"].as_array().unwrap();
            let term_columns = terms.iter().filter_map(|t| t["column"].as_str());
            for column in quoted.chain(term_columns) {
                assert!(
                    columns.contains(&column),
                    "unknown column {column:?} in answer to {:?}",
                    entry.question
                );
            }
        }
    }

    /// Cột trong form câu "tại sao" (time, measure, groups, filters, terms) cũng phải có trong hồ sơ dataco.
    #[test]
    fn why_forms_use_only_profile_columns() {
        let entries = file_entries();
        let profile = entries
            .iter()
            .find(|e| e.task == LlmTask::Profile && e.question == "dataco")
            .unwrap();
        let columns: Vec<&str> = profile.answer["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap())
            .collect();
        for entry in entries.iter().filter(|e| e.task == LlmTask::Why) {
            let form = &entry.answer;
            let mut used: Vec<&serde_json::Value> = vec![
                &form["time"]["column"],
                &form["measure"]["column"],
                &form["groups"]["column"],
            ];
            used.extend(
                form["filters"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|f| &f["column"]),
            );
            used.extend(
                form["terms"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|t| &t["column"]),
            );
            for column in used.into_iter().filter_map(|c| c.as_str()) {
                assert!(
                    columns.contains(&column),
                    "unknown column {column:?} in the form for {:?}",
                    entry.question
                );
            }
        }
    }

    #[tokio::test]
    async fn sample_question_returns_its_own_sql() {
        let entries = file_entries();
        let expected = entries
            .iter()
            .find(|e| e.task == LlmTask::Sql && e.question == "Doanh thu theo tháng")
            .unwrap()
            .answer
            .clone();
        let reply = Mock::new()
            .complete(&sql_request("doanh thu theo THANG ?"))
            .await
            .unwrap();
        let reply: serde_json::Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(reply, expected);
    }

    /// Câu mẫu của vòng sửa: câu sinh lần đầu đọc sai định dạng ngày, câu sửa đọc đúng.
    #[tokio::test]
    async fn repair_question_returns_the_fixed_sql() {
        let question = "Doanh thu theo ngày giao hàng";
        let mock = Mock::new();
        let first = mock.complete(&sql_request(question)).await.unwrap();
        let fixed = mock
            .complete(&LlmRequest {
                task: LlmTask::Repair,
                question,
                prompt: "",
                json_schema: None,
            })
            .await
            .unwrap();
        assert!(first.contains("'%d/%m/%Y %H:%M'"), "{first}");
        assert!(fixed.contains("'%m/%d/%Y %H:%M'"), "{fixed}");
    }

    #[tokio::test]
    async fn same_question_with_other_task_is_not_answered() {
        let request = LlmRequest {
            task: LlmTask::Profile,
            question: "Doanh thu theo tháng",
            prompt: "",
            json_schema: None,
        };
        assert!(Mock::new().complete(&request).await.is_err());
    }

    #[tokio::test]
    async fn other_question_explains_how_to_enable_real_llm() {
        let err = Mock::new()
            .complete(&sql_request("anything else"))
            .await
            .unwrap_err();
        match err {
            AppError::BadRequest(msg) => assert!(msg.contains("LLM_PROVIDER")),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
