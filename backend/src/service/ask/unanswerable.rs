// SPDX-License-Identifier: Apache-2.0
/* service/ask/unanswerable.rs - lời cho người dùng khi mô hình báo câu hỏi không trả lời được từ dữ liệu (#164,
trường unanswerable của ask/reply.rs). ask/mod.rs gọi cho câu thường, explain/run.rs cho câu "tại sao"; cả hai trả
rejected với lời này, không chạy SQL, và ghi lịch sử như câu bị từ chối. Hàm thuần, test ở cuối file.

Cột gần nhất do mô hình nêu nên lọc như phương án của câu hỏi lại (ask/choices.rs): chỉ giữ tên có thật trong các cột
đã gửi mô hình, để lời không gợi ý một cột không có. */

use crate::service::ask::reply::Unanswerable;

/// Số cột gợi ý tối đa trong lời: đủ để người hỏi thấy hướng hỏi khác, không thành danh sách dài.
const MAX_RELATED: usize = 5;

/// "Dữ liệu không có thông tin để trả lời câu này: <lý do>. Có thể hỏi về các cột gần nhất: A, B." columns là tên
/// các cột của ngữ cảnh; cột mô hình nêu mà không có trong đó thì bỏ.
pub fn build_unanswerable_message(unanswerable: &Unanswerable, columns: &[&str]) -> String {
    let reason = unanswerable.reason.trim().trim_end_matches('.');
    let mut related: Vec<&str> = Vec::new();
    for name in &unanswerable.related_columns {
        let name = name.trim();
        if columns.contains(&name) && !related.contains(&name) && related.len() < MAX_RELATED {
            related.push(name);
        }
    }
    let hint = if related.is_empty() {
        "Thử hỏi về các cột có trong dataset.".to_string()
    } else {
        format!("Có thể hỏi về các cột gần nhất: {}.", related.join(", "))
    };
    format!("Dữ liệu không có thông tin để trả lời câu này: {reason}. {hint}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unanswerable(reason: &str, related: &[&str]) -> Unanswerable {
        Unanswerable {
            reason: reason.to_string(),
            related_columns: related.iter().map(|c| c.to_string()).collect(),
        }
    }

    #[test]
    fn message_keeps_only_real_columns() {
        let columns = ["Order Status", "Delivery Status", "Customer Segment"];
        let given = unanswerable(
            "dataset không có cột đánh giá hay mức hài lòng của khách hàng.",
            &[
                "Delivery Status",
                "Rating",
                "Order Status",
                "Delivery Status",
            ],
        );
        assert_eq!(
            build_unanswerable_message(&given, &columns),
            "Dữ liệu không có thông tin để trả lời câu này: dataset không có cột đánh giá hay mức hài lòng của khách \
             hàng. Có thể hỏi về các cột gần nhất: Delivery Status, Order Status."
        );
    }

    #[test]
    fn message_without_real_columns_asks_for_another_question() {
        let given = unanswerable("dữ liệu chỉ có các năm đã qua", &["Forecast"]);
        assert_eq!(
            build_unanswerable_message(&given, &["Order Status"]),
            "Dữ liệu không có thông tin để trả lời câu này: dữ liệu chỉ có các năm đã qua. Thử hỏi về các cột có trong \
             dataset."
        );
    }

    #[test]
    fn at_most_five_columns_are_suggested() {
        let columns = ["a", "b", "c", "d", "e", "f"];
        let given = unanswerable("không có", &columns);
        let message = build_unanswerable_message(&given, &columns);
        assert!(message.ends_with("a, b, c, d, e."), "{message}");
    }
}
