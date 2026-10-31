// SPDX-License-Identifier: Apache-2.0
/* service/ask/value_retry_tests.rs - test phần thuần của việc gửi lại kèm đủ giá trị (ask/value_retry.rs): cột nào được
gửi lại, và danh sách đủ thay cho danh sách thiếu. Test qua route (gửi lại đúng một lần) ở api/ask_tests.rs. */

use uuid::Uuid;

use crate::engine::store::{ColumnAggregation, ValueRow};
use crate::service::ask::context::ColumnProfile;
use crate::service::ask::filter_values::MissingValue;
use crate::service::ask::value_retry::{apply_full_values, build_resend_columns};
use crate::service::ask::values::{ValueEntry, ValueIndex};

fn dimension(name: &str) -> ColumnProfile {
    ColumnProfile {
        id: Uuid::new_v4(),
        name: name.to_string(),
        data_type: "VARCHAR".to_string(),
        role: "dimension".to_string(),
        time_format: None,
        description: None,
        aliases: Vec::new(),
        is_sensitive: false,
        values: Vec::new(),
        values_complete: false,
        matched: false,
        unit: None,
        aggregation: ColumnAggregation::Sum,
        join_key: false,
        metric_column: false,
        picked: false,
        brief: false,
    }
}

fn missing(column: &str) -> MissingValue {
    MissingValue {
        column: column.to_string(),
        value: "Nowhere".to_string(),
        suggestions: Vec::new(),
    }
}

/// Chỉ cột chiều không nhạy cảm chưa gửi đủ: cột đã đủ danh sách giữ cảnh báo như cũ, cột nhạy cảm không bao giờ gửi.
#[test]
fn only_columns_without_a_complete_list_are_resent() {
    let mut complete = dimension("Market");
    complete.values_complete = true;
    let mut sensitive = dimension("Email");
    sensitive.is_sensitive = true;
    let columns = vec![dimension("Region"), complete, sensitive];
    let found = build_resend_columns(
        &[
            missing("Region"),
            missing("Region"),
            missing("Market"),
            missing("Email"),
            missing("Other"),
        ],
        &columns,
    );
    assert_eq!(found, ["Region"]);
}

/// Cột đang gửi 2 giá trị đầu nhận đủ 3 giá trị của chỉ mục; gọi lại thì không còn gì để gửi thêm.
#[test]
fn full_values_replace_the_partial_list() {
    let mut region = dimension("Region");
    let rows: Vec<ValueRow> = ["North", "South", "West"]
        .iter()
        .map(|v| ValueRow {
            column_id: region.id,
            column_name: "Region".to_string(),
            value: v.to_string(),
            aliases: Vec::new(),
        })
        .collect();
    let index = ValueIndex::build(&rows);
    region.values = vec![
        ValueEntry {
            value: "North".to_string(),
            aliases: Vec::new(),
        },
        ValueEntry {
            value: "South".to_string(),
            aliases: Vec::new(),
        },
    ];
    let mut columns = vec![region];
    assert!(apply_full_values(&mut columns, &index, "Region"));
    assert_eq!(columns[0].values.len(), 3);
    assert!(columns[0].values_complete);
    assert!(!apply_full_values(&mut columns, &index, "Region"));
    assert!(!apply_full_values(&mut columns, &index, "Nowhere"));
}
