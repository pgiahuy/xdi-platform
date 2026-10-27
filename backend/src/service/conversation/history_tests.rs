// SPDX-License-Identifier: Apache-2.0
/* service/conversation/history_tests.rs - test khối "Hội thoại trước" và phần đã hiểu dạng chữ (history.rs). */

use chrono::Utc;
use uuid::Uuid;

use super::history::{
    build_history_section, build_understood_text, HISTORY_MAX_CHARS, HISTORY_TURNS,
};
use crate::engine::store::StoredTurn;
use crate::service::ask::contract::{Understanding, UnderstoodItem};

fn turn(question: &str, sql_generated: Option<&str>, understood: Option<&str>) -> StoredTurn {
    StoredTurn {
        query_id: Uuid::new_v4(),
        question: question.to_string(),
        status: "ok".to_string(),
        sql_generated: sql_generated.map(str::to_string),
        sql_guarded: Some("SELECT guarded LIMIT 100000".to_string()),
        understood: understood.map(str::to_string),
        created_at: Utc::now(),
    }
}

fn item(label: &str, sql: &str) -> UnderstoodItem {
    UnderstoodItem {
        label: label.to_string(),
        sql: sql.to_string(),
    }
}

#[test]
fn no_turns_means_no_section() {
    assert_eq!(build_history_section(&[]), "");
}

#[test]
fn keeps_the_last_turns_in_order_with_the_sql_the_model_wrote() {
    let turns: Vec<StoredTurn> = (1..=6)
        .map(|i| {
            turn(
                &format!("question {i}"),
                Some(&format!("SELECT {i}")),
                Some("measures: Doanh thu"),
            )
        })
        .collect();
    let section = build_history_section(&turns);
    let first_kept = 6 - HISTORY_TURNS as usize + 1;
    assert!(!section.contains("question 1\n"), "{section}");
    assert!(
        section.contains(&format!("Turn 1: question {first_kept}")),
        "{section}"
    );
    assert!(section.contains("Turn 4: question 6"), "{section}");
    assert!(section.contains("SQL: SELECT 6"), "{section}");
    assert!(section.contains("Status: answered"), "{section}");
    assert!(
        section.contains("Understood as: measures: Doanh thu"),
        "{section}"
    );
    assert!(!section.contains("guarded"), "{section}");
}

#[test]
fn falls_back_to_the_guarded_sql() {
    let section = build_history_section(&[turn("q", None, None)]);
    assert!(section.contains("SQL: SELECT guarded"), "{section}");
}

#[test]
fn drops_old_turns_to_stay_under_the_limit() {
    let long_sql = format!("SELECT {}", "x".repeat(HISTORY_MAX_CHARS / 3));
    let turns: Vec<StoredTurn> = (1..=4)
        .map(|i| turn(&format!("question {i}"), Some(&long_sql), None))
        .collect();
    let section = build_history_section(&turns);
    assert!(section.chars().count() <= HISTORY_MAX_CHARS);
    assert!(section.contains("question 4"), "the newest turn stays");
    assert!(
        !section.contains("question 1"),
        "the oldest turn goes first"
    );

    let huge = turn("only", Some(&"y".repeat(HISTORY_MAX_CHARS * 2)), None);
    assert_eq!(
        build_history_section(&[huge]).chars().count(),
        HISTORY_MAX_CHARS
    );
}

#[test]
fn understood_text_leaves_out_row_filters() {
    let understanding = Understanding {
        measures: vec![item("Doanh thu: tổng Order Item Total", "SUM(x)")],
        filters: vec![
            item("Market là Europe", "\"Market\" = 'Europe'"),
            item(
                "Phân quyền: chỉ thấy Market là LATAM",
                "\"Market\" IN ('LATAM')",
            ),
        ],
        group_by: vec![item("Order Region", "\"Order Region\"")],
        order_by: vec![],
        limit: None,
        time_range: Some(item("năm 2017", "y = 2017")),
        assumptions: vec![],
    };
    let text = build_understood_text(&understanding).unwrap();
    assert_eq!(
        text,
        "measures: Doanh thu: tổng Order Item Total; filters: Market là Europe; grouped by: Order Region; \
         period: năm 2017"
    );
}
