// SPDX-License-Identifier: Apache-2.0
/* service/explain/joined.rs - cột của các bảng nối nhiều-một vào bảng bắt đầu mà form câu hỏi nguyên nhân được dùng
cho kỳ, nhóm và bộ lọc. Câu "Vì sao doanh thu năm 2017 khác nhau giữa Europe và LATAM" đo order_items.item_total, còn thị
trường và ngày đặt nằm ở orders: trước đây form chỉ thấy cột của bảng bắt đầu nên không điền được câu này.

Tên cột là "bảng.cột", đúng tên cột của câu con nối bảng (explain/source.rs, build_from_sql): kiểm form (explain/check.rs)
tìm cột theo tên này, và SQL do code dựng gọi đúng tên này mà không viết lại gì. Chỉ cột vai trò dimension hay time, trừ
cột khóa nối (trùng cột khóa ngoại của bảng bắt đầu) và cột nhạy cảm. Độ đo vẫn phải là cột của bảng bắt đầu: cộng một
cột của bảng cha trên các dòng của bảng con thì đếm lặp (một đơn nhiều dòng hàng).

ask/why.rs dựng JoinedColumns từ nhóm bảng của câu hỏi; explain/run.rs đưa vào prompt form và kiểm form. Hàm thuần,
test ở explain/source_tests.rs. */

use crate::engine::store::{ProfileColumn, ProfileRole};
use crate::service::ask::context::ColumnProfile;
use crate::service::ask::values::ColumnValues;
use crate::service::explain::source::RelatedTables;

/// Cột của bảng liên quan dùng được trong form, mỗi cột đã đổi tên thành "bảng.cột".
#[derive(Debug, Clone, Default)]
pub struct JoinedColumns {
    /// Hồ sơ cột, cho kiểm form.
    pub profile: Vec<ProfileColumn>,
    /// Cột ngữ cảnh kèm danh sách giá trị, cho prompt form.
    pub prompt: Vec<ColumnProfile>,
    /// Giá trị của các cột chiều, để kiểm giá trị nhóm và giá trị lọc.
    pub values: Vec<ColumnValues>,
}

/// Không có bảng liên quan: dataset nạp từ file, phân rã tự động của câu hỏi số liệu, test.
pub static NO_JOINED: JoinedColumns = JoinedColumns {
    profile: Vec::new(),
    prompt: Vec::new(),
    values: Vec::new(),
};

/// Một bảng liên quan như bước dựng cần.
pub struct JoinedTable<'a> {
    pub table: &'a str,
    pub columns: &'a [ProfileColumn],
    /// Cột ngữ cảnh của bảng (ask/group_prompt.rs), đã gắn danh sách giá trị; rỗng thì prompt không có cột của bảng này.
    pub prompt: &'a [ColumnProfile],
    pub values: &'a [ColumnValues],
}

/// Cột dùng được của mọi bảng trong related.source (các bảng nối nhiều-một vào bảng bắt đầu). main là cột của bảng bắt
/// đầu: tên "bảng.cột" trùng một cột của nó (bảng có cột tên "orders.status") thì bỏ, để mỗi tên trong form chỉ chỉ một
/// cột.
pub fn build_joined_columns(
    related: &RelatedTables,
    main: &[ProfileColumn],
    tables: &[JoinedTable],
) -> JoinedColumns {
    let mut joined = JoinedColumns::default();
    for (index, join) in related.source.joins.iter().enumerate() {
        let Some(table) = tables.iter().find(|t| t.table == join.table) else {
            continue;
        };
        let keys: Vec<&str> = join.keys.iter().map(|(_, own)| own.as_str()).collect();
        let usable = related
            .source
            .columns
            .iter()
            .filter(|c| c.join == index && !keys.contains(&c.column.as_str()));
        for related_column in usable {
            let Some(column) = table
                .columns
                .iter()
                .find(|c| c.name == related_column.column)
            else {
                continue;
            };
            if !is_form_column(column) || main.iter().any(|c| c.name == related_column.name) {
                continue;
            }
            let mut renamed = column.clone();
            renamed.name = related_column.name.clone();
            joined.profile.push(renamed);
            if let Some(prompt) = table.prompt.iter().find(|c| c.name == column.name) {
                let mut prompt = prompt.clone();
                prompt.name = related_column.name.clone();
                joined.prompt.push(prompt);
            }
            if let Some(values) = table.values.iter().find(|v| v.column_id == column.id) {
                let mut values = values.clone();
                values.column_name = related_column.name.clone();
                joined.values.push(values);
            }
        }
    }
    joined
}

fn is_form_column(column: &ProfileColumn) -> bool {
    let role = column.role.as_str();
    !column.is_sensitive
        && (role == ProfileRole::Dimension.as_str() || role == ProfileRole::Time.as_str())
}
