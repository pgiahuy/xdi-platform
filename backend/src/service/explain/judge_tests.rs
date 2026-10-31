// SPDX-License-Identifier: Apache-2.0
/* service/explain/judge_tests.rs - test phần chữ của cổng 2 (explain/judge.rs, #201): prompt gửi mô hình chấm là đúng
file luật hệ thống dùng (backend/prompts/judge_answer.txt), chỉ điền thêm JSON (tên độ đo, tên hai bên, khẳng định) và
lời; đọc kết luận match, warning, mismatch (kể cả warning hay mismatch không nêu chỗ nào), và từ chối câu trả lời không
đúng khuôn. */

use crate::engine::prompts;
use crate::service::ask::examples::build_why_example;
use crate::service::explain::claims::NarrateReply;
use crate::service::explain::claims_tests::build_example_reply;
use crate::service::explain::judge::{
    build_judge_facts, build_judge_json, build_judge_prompt, parse_judge_reply, Verdict,
};

/// Prompt là nguyên văn file luật mà binary nhúng và gửi đi, với hai chỗ trống được điền: không thêm, không bớt chữ
/// nào của luật. JSON điền vào có tên hai bên lấy từ bảng và số có dấu; nhóm không có trong khẳng định nào (USCA) không
/// lọt vào.
#[test]
fn prompt_is_the_rules_file_filled_with_the_facts_and_the_text() {
    let explanation = build_why_example().explanation.expect("why example");
    let reply: NarrateReply = serde_json::from_value(build_example_reply()).unwrap();
    let facts = build_judge_facts(&explanation, &reply.claims);
    let text = reply.build_text();
    let prompt = build_judge_prompt(&text, &facts);

    let rules = prompts::JUDGE_ANSWER.text;
    let (head, rest) = rules.split_once("{{meaning}}").expect("meaning slot");
    let (before, rest) = rest.split_once("{{facts}}").expect("facts slot");
    let (between, after) = rest.split_once("{{text}}").expect("text slot");
    // fill bỏ đúng một dấu xuống dòng ở cuối mỗi file (engine/prompts.rs).
    let after = after.strip_suffix('\n').unwrap_or(after);
    let meaning = prompts::JUDGE_MEANING_WHY.text;
    let meaning = meaning.strip_suffix('\n').unwrap_or(meaning);
    let facts_json = build_judge_json(&facts);
    assert_eq!(
        prompt,
        format!("{head}{meaning}{before}{facts_json}{between}{text}{after}")
    );

    assert!(!prompt.contains("{{"), "{prompt}");
    for expected in [
        "\"measure\": \"Doanh thu\"",
        "\"base\": \"Quý 2/2017\"",
        "\"compare\": \"Quý 3/2017\"",
        "\"value\": \"-639.780,25\"",
        "\"amount\": -639780.25",
    ] {
        assert!(prompt.contains(expected), "{expected}");
    }
    assert!(!prompt.contains("USCA"), "{prompt}");
}

#[test]
fn verdicts_are_read() {
    assert_eq!(
        parse_judge_reply(r#"{"verdict": "match", "problems": []}"#),
        Ok(Verdict::Match)
    );
    assert_eq!(
        parse_judge_reply(r#"{"verdict": "mismatch", "problems": ["Sai nhóm", "  "]}"#),
        Ok(Verdict::Mismatch(vec!["Sai nhóm".to_string()]))
    );
    assert_eq!(
        parse_judge_reply(r#"{"verdict": "mismatch", "problems": []}"#),
        Ok(Verdict::Mismatch(vec![
            "Lời chưa khớp các khẳng định".to_string()
        ]))
    );
    assert_eq!(
        parse_judge_reply(
            r#"{"verdict": "warning", "problems": ["\"giảm mạnh\" chưa rõ là so với kỳ nào"]}"#
        ),
        Ok(Verdict::Warning(vec![
            "\"giảm mạnh\" chưa rõ là so với kỳ nào".to_string()
        ]))
    );
    assert_eq!(
        parse_judge_reply(r#"{"verdict": "warning", "problems": []}"#),
        Ok(Verdict::Warning(vec![
            "Lời có chỗ chưa rõ nghĩa".to_string()
        ]))
    );
}

#[test]
fn unreadable_replies_are_errors() {
    for reply in [
        "khớp",
        r#"{"problems": []}"#,
        r#"{"verdict": "maybe", "problems": []}"#,
    ] {
        assert!(parse_judge_reply(reply).is_err(), "{reply}");
    }
}
