// SPDX-License-Identifier: Apache-2.0
/* api/ask_fanout_tests.rs - chặn câu cộng lặp khi nối bảng một-nhiều (service/ask/fanout.rs, #214) qua route thật, trên
Postgres thật: chụp hai bảng giống schema edu của database mẫu (students và bài nộp student_assessments trỏ tới
students), rồi mô hình giả trả câu SUM tín chỉ trên bảng bài nộp. Câu đầu không chạy mà vào vòng sửa; câu sửa đúng ra
số đúng, câu sửa vẫn cộng lặp thì câu hỏi bị từ chối. Không có DATABASE_URL thì bỏ qua. */

use axum::http::StatusCode;
use serde_json::{json, Value};

use super::ask_tests::{sql_reply, step_names};
use super::source_relations_tests::{dataset_id, snapshot};
use super::sources_tests::{start, Setup, KEY, SHOP};
use crate::api::test_support::call;
use crate::engine::store::test_support::{exec, fetch_count};

/// Ba sinh viên; sinh viên 1 nộp ba bài, sinh viên 2 nộp hai bài, sinh viên 3 nộp một bài. Tổng tín chỉ đúng: East
/// Anglian 60 + 120 = 180, Wales 30. Cộng trên bảng bài nộp: East Anglian 60 * 3 + 120 * 2 = 420.
fn edu_schema(user: &str) -> Vec<String> {
    let statements = [
        "CREATE SCHEMA edu",
        "CREATE TABLE edu.students (code_module text, code_presentation text, id_student int, region text,
             studied_credits int, PRIMARY KEY (code_module, code_presentation, id_student))",
        "CREATE TABLE edu.student_assessments (id_assessment int, id_student int, code_module text,
             code_presentation text, score numeric, PRIMARY KEY (id_assessment, id_student),
             FOREIGN KEY (code_module, code_presentation, id_student)
                 REFERENCES edu.students (code_module, code_presentation, id_student))",
        "INSERT INTO edu.students VALUES ('AAA', '2013J', 1, 'East Anglian', 60),
             ('AAA', '2013J', 2, 'East Anglian', 120), ('AAA', '2013J', 3, 'Wales', 30)",
        "INSERT INTO edu.student_assessments VALUES (101, 1, 'AAA', '2013J', 70), (102, 1, 'AAA', '2013J', 80),
             (103, 1, 'AAA', '2013J', 65), (101, 2, 'AAA', '2013J', 50), (102, 2, 'AAA', '2013J', 90),
             (101, 3, 'AAA', '2013J', 40)",
    ];
    let mut all: Vec<String> = statements.iter().map(|s| s.to_string()).collect();
    all.push(format!("GRANT USAGE ON SCHEMA edu TO {user}"));
    all.push(format!(
        "GRANT SELECT ON edu.students, edu.student_assessments TO {user}"
    ));
    all
}

/// Nguồn Postgres có schema edu, chụp students kèm student_assessments; trả id dataset students.
async fn snapshot_edu(s: &Setup) -> String {
    for sql in edu_schema(&s.user) {
        exec(&s.db.store, &sql).await;
    }
    let url = format!("{SHOP}/sources");
    let (status, _, source) = call(&s.app, "POST", &url, Some(&s.lan), Some(s.form.clone())).await;
    assert_eq!(status, StatusCode::CREATED, "{source}");
    let id = source["id"].as_str().unwrap();
    let body = json!({"table": "edu.students", "tables": ["edu.student_assessments"]});
    let (status, result) = snapshot(s, id, body).await;
    assert_eq!(status, StatusCode::CREATED, "{result}");
    dataset_id(&result, "students")
}

async fn ask(s: &Setup, dataset: &str, question: &str) -> Value {
    let body = json!({"question": question, "choices": []});
    let url = format!("{SHOP}/datasets/{dataset}/ask");
    let (status, _, answer) = call(&s.app, "POST", &url, Some(&s.lan), Some(body)).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    answer
}

const JOIN: &str = "FROM student_assessments sa JOIN students s ON sa.code_module = s.code_module \
     AND sa.code_presentation = s.code_presentation AND sa.id_student = s.id_student";

#[tokio::test]
async fn credits_summed_over_submissions_are_repaired_or_refused() {
    let wrong = format!(
        "SELECT s.region, SUM(s.studied_credits) AS credits {JOIN} GROUP BY s.region ORDER BY s.region"
    );
    let fixed = format!(
        "WITH one_row AS (SELECT DISTINCT s.code_module, s.code_presentation, s.id_student, s.region, \
         s.studied_credits {JOIN}) SELECT region, SUM(studied_credits) AS credits FROM one_row \
         GROUP BY region ORDER BY region"
    );
    let replies = [
        sql_reply(&wrong),
        sql_reply(&fixed),
        sql_reply(&wrong),
        sql_reply(&wrong),
    ];
    let Some(s) = start(Some(KEY), &replies).await else {
        return;
    };
    let students = snapshot_edu(&s).await;

    // Câu đầu cộng lặp: không chạy, bước guard ghi lý do, mô hình sửa, câu sửa chạy ra số đúng.
    let answer = ask(&s, &students, "Tổng tín chỉ đăng ký theo vùng").await;
    assert_eq!(answer["status"], "answered", "{answer}");
    assert_eq!(
        answer["result"]["rows"],
        json!([["East Anglian", "180"], ["Wales", "30"]]),
        "{answer}"
    );
    let names = step_names(&answer);
    let guard = names.iter().position(|n| *n == "guard").unwrap();
    assert_eq!(names[guard..guard + 4], ["guard", "repair", "guard", "run"]);
    let refused = &answer["steps"][guard];
    assert_eq!(refused["ok"], false);
    let detail = refused["detail"].as_str().unwrap();
    assert!(
        detail.contains("students") && detail.contains("student_assessments"),
        "{detail}"
    );

    // Câu sửa vẫn cộng lặp: không chạy, lời báo nói rõ lý do.
    let answer = ask(&s, &students, "Tổng tín chỉ đăng ký theo vùng").await;
    assert_eq!(answer["status"], "rejected", "{answer}");
    assert!(answer["result"].is_null(), "{answer}");
    let message = answer["message"].as_str().unwrap();
    assert!(
        message.contains("tính lặp") && message.contains("students"),
        "{message}"
    );
    // Ba lần chặn cộng lặp (câu đầu của lượt một, hai câu của lượt hai), mỗi lần một dòng nhật ký lỗi (#228).
    let sql = "SELECT count(*) FROM error_log WHERE step = 'fanout' AND model_output LIKE '%SUM(s.studied_credits)%'";
    assert_eq!(fetch_count(&s.db.store, sql).await, 3);
    s.finish().await;
}
