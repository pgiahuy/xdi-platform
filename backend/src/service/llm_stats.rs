// SPDX-License-Identifier: Apache-2.0
/* service/llm_stats.rs - lệnh `xdi-backend llm-stats` (#184): đọc queries.timings của các câu hỏi đã ghi và in, theo
loại câu (câu thường, câu "tại sao"), thời gian từ đầu câu tới lúc ghi, thời gian chờ mô hình, số lần gọi và số token;
thêm một bảng theo loại việc của mô hình (LlmTask). Có giá token thì in thêm chi phí trung bình mỗi câu.

Số liệu chỉ có từ khi câu hỏi ghi lần gọi mô hình (engine/llm/meter.rs); bản ghi cũ hơn (timings là mảng các bước) bị
bỏ qua. Câu needs_choice không ghi lịch sử nên không có ở đây. Giá không ghi cứng trong code: người chạy lệnh đưa giá
của mô hình đang dùng qua --price-in, --price-out (USD cho một triệu token). */

use std::collections::BTreeMap;

use crate::engine::store::QueryTimings;

/// Giá token, USD cho một triệu token.
#[derive(Debug, Clone, Copy)]
pub struct TokenPrice {
    pub input: f64,
    pub output: f64,
}

/// Một câu hỏi đã đọc từ timings.
#[derive(Debug, Clone, PartialEq)]
pub struct QuestionStat {
    /// "why" khi câu có lần gọi điền form "tại sao", "plain" cho câu còn lại.
    pub kind: &'static str,
    pub status: String,
    pub question: String,
    /// Từ đầu câu hỏi tới lúc ghi lịch sử (bước record).
    pub elapsed_ms: u64,
    pub model_ms: u64,
    pub calls: Vec<CallStat>,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CallStat {
    pub task: String,
    pub duration_ms: u64,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

/// None cho bản ghi trước #184 hay timings không đọc được.
pub fn build_question_stat(row: &QueryTimings) -> Option<QuestionStat> {
    let calls_json = row.timings.get("model_calls")?.as_array()?;
    let elapsed_ms = row.timings.get("elapsed_ms")?.as_u64()?;
    let calls: Vec<CallStat> = calls_json
        .iter()
        .filter_map(|call| {
            Some(CallStat {
                task: call.get("task")?.as_str()?.to_string(),
                duration_ms: call.get("duration_ms")?.as_u64()?,
                input_tokens: call.get("input_tokens").and_then(|v| v.as_u64()),
                output_tokens: call.get("output_tokens").and_then(|v| v.as_u64()),
            })
        })
        .collect();
    let kind = if calls.iter().any(|c| c.task == "why") {
        "why"
    } else {
        "plain"
    };
    Some(QuestionStat {
        kind,
        status: row.status.clone(),
        question: row.question.clone(),
        elapsed_ms,
        model_ms: calls.iter().map(|c| c.duration_ms).sum(),
        input_tokens: calls.iter().filter_map(|c| c.input_tokens).sum(),
        output_tokens: calls.iter().filter_map(|c| c.output_tokens).sum(),
        calls,
    })
}

/// Chi phí USD của một lượng token theo giá.
pub fn build_cost(input_tokens: u64, output_tokens: u64, price: TokenPrice) -> f64 {
    (input_tokens as f64 * price.input + output_tokens as f64 * price.output) / 1_000_000.0
}

/// Trung vị; dùng thay trung bình vì một câu gặp mô hình chậm bất thường không kéo cả dòng lên.
fn median(mut values: Vec<u64>) -> u64 {
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[mid - 1] + values[mid]) / 2
    } else {
        values[mid]
    }
}

fn mean(values: &[u64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<u64>() as f64 / values.len() as f64
}

/// Báo cáo dạng chữ: mỗi câu một dòng, rồi bảng theo loại câu, rồi bảng theo loại việc của mô hình. Cột cách nhau
/// bằng tab để dán được vào bảng tính.
pub fn build_report(stats: &[QuestionStat], price: Option<TokenPrice>) -> String {
    let mut out = String::new();
    out.push_str(
        "kind\tstatus\telapsed_ms\tmodel_ms\tcalls\tinput_tokens\toutput_tokens\tquestion\n",
    );
    for s in stats {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            s.kind,
            s.status,
            s.elapsed_ms,
            s.model_ms,
            s.calls.len(),
            s.input_tokens,
            s.output_tokens,
            s.question
        ));
    }
    out.push_str("\nby kind (medians of time, means of calls and tokens)\n");
    out.push_str("kind\tquestions\telapsed_ms\tmodel_ms\tcalls\tinput_tokens\toutput_tokens");
    out.push_str(if price.is_some() {
        "\tcost_usd\n"
    } else {
        "\n"
    });
    let mut kinds: BTreeMap<&str, Vec<&QuestionStat>> = BTreeMap::new();
    for s in stats {
        kinds.entry(s.kind).or_default().push(s);
    }
    for (kind, list) in &kinds {
        let calls: Vec<u64> = list.iter().map(|s| s.calls.len() as u64).collect();
        let input: Vec<u64> = list.iter().map(|s| s.input_tokens).collect();
        let output: Vec<u64> = list.iter().map(|s| s.output_tokens).collect();
        out.push_str(&format!(
            "{kind}\t{}\t{}\t{}\t{:.1}\t{:.0}\t{:.0}",
            list.len(),
            median(list.iter().map(|s| s.elapsed_ms).collect()),
            median(list.iter().map(|s| s.model_ms).collect()),
            mean(&calls),
            mean(&input),
            mean(&output),
        ));
        if let Some(price) = price {
            let costs: f64 = list
                .iter()
                .map(|s| build_cost(s.input_tokens, s.output_tokens, price))
                .sum();
            out.push_str(&format!("\t{:.6}", costs / list.len() as f64));
        }
        out.push('\n');
    }
    out.push_str("\nby model task (medians of time, means of tokens)\n");
    out.push_str("task\tcalls\tduration_ms\tinput_tokens\toutput_tokens\n");
    let mut tasks: BTreeMap<&str, Vec<&CallStat>> = BTreeMap::new();
    for call in stats.iter().flat_map(|s| &s.calls) {
        tasks.entry(call.task.as_str()).or_default().push(call);
    }
    for (task, list) in &tasks {
        let input: Vec<u64> = list.iter().filter_map(|c| c.input_tokens).collect();
        let output: Vec<u64> = list.iter().filter_map(|c| c.output_tokens).collect();
        out.push_str(&format!(
            "{task}\t{}\t{}\t{:.0}\t{:.0}\n",
            list.len(),
            median(list.iter().map(|c| c.duration_ms).collect()),
            mean(&input),
            mean(&output),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(question: &str, timings: serde_json::Value) -> QueryTimings {
        QueryTimings {
            question: question.to_string(),
            status: "ok".to_string(),
            timings,
        }
    }

    #[test]
    fn reads_calls_and_skips_old_rows() {
        let new = row(
            "Vì sao doanh thu giảm",
            serde_json::json!({
                "steps": [],
                "elapsed_ms": 5000,
                "model_calls": [
                    {"task": "classify", "duration_ms": 800, "ok": true, "input_tokens": 400, "output_tokens": 20},
                    {"task": "why", "duration_ms": 1500, "ok": true, "input_tokens": 3000, "output_tokens": 200},
                    {"task": "narrate", "duration_ms": 1000, "ok": false, "input_tokens": null, "output_tokens": null}
                ]
            }),
        );
        let stat = build_question_stat(&new).unwrap();
        assert_eq!(stat.kind, "why");
        assert_eq!((stat.model_ms, stat.calls.len()), (3300, 3));
        assert_eq!((stat.input_tokens, stat.output_tokens), (3400, 220));

        let old = row(
            "Doanh thu",
            serde_json::json!([{"name": "llm", "duration_ms": 10}]),
        );
        assert!(build_question_stat(&old).is_none());
    }

    #[test]
    fn report_has_cost_when_priced() {
        let plain = row(
            "Doanh thu theo thị trường",
            serde_json::json!({
                "elapsed_ms": 3000,
                "model_calls": [
                    {"task": "sql", "duration_ms": 1200, "ok": true, "input_tokens": 1_000_000, "output_tokens": 0}
                ]
            }),
        );
        let stats = vec![build_question_stat(&plain).unwrap()];
        let price = TokenPrice {
            input: 0.1,
            output: 0.4,
        };
        let report = build_report(&stats, Some(price));
        assert!(
            report.contains("plain\t1\t3000\t1200\t1.0\t1000000\t0\t0.100000"),
            "{report}"
        );
        assert!(report.contains("sql\t1\t1200\t1000000\t0"), "{report}");
    }
}
