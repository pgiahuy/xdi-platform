// SPDX-License-Identifier: Apache-2.0
/* service/column_roles.rs - hai luật vai trò cột của lớp cấu trúc dựa vào thứ ngoài số đo (service/profile.rs giữ các
luật theo kiểu và số đo):
- Cột số nguyên là mốc thời gian đếm bằng số (is_time_position_name): edu.student_daily_clicks.date là số ngày tính từ
  đầu học phần, edu.vle_materials.week_from là tuần thứ mấy. Luật số đo cho chúng thành measure, mà cộng hay lấy tổng
  của "ngày thứ 10" thì vô nghĩa, và mô hình đã lấy chúng làm kỳ của câu gợi ý ("ngày 10 so với ngày 0"). Chúng thành
  dimension (không quá DIMENSION_MAX giá trị) hay text: nhóm theo được, lọc theo số được, trung bình vẫn tính được (đường
  câu hỏi nguyên nhân nhận avg trên mọi cột số không phải key), nhưng không thành kỳ và không bị cộng mặc định.
- Khóa của database nguồn (apply_source_keys): cột của khóa chính hay khóa ngoại khai báo ở database thành key, thay vì
  đoán từ "số nguyên, mỗi dòng một giá trị". Mã sản phẩm, mã khách trong bảng dòng hàng lặp lại nên luật số đo cho chúng
  thành measure, chờ lớp ngữ nghĩa sửa; khóa ngoại nói thẳng chúng là mã. Chỉ cột số nguyên, và cột chữ đứng một mình làm
  khóa chính: cột chữ của khóa nhiều cột hay khóa ngoại như code_module (mã môn, 7 giá trị) là nhóm người dùng hỏi theo
  ("môn AAA"), phải giữ dimension. Cột mốc thời gian trong khóa chính (date của bảng lượt bấm theo ngày) giữ luật trên.
service/ingest.rs gọi apply_source_keys sau khi lập hồ sơ cấu trúc của bảng chụp từ database; khóa đọc ở
service/sources/catalog.rs (read_key_columns), trong cùng giao dịch với lúc chép bảng. */

use crate::engine::store::{NewColumn, ProfileRole};
use crate::service::profile::{is_integer_type, name_tokens};

/// Từ trong tên cột cho biết cột số nguyên là một mốc thời gian: ngày, tuần, tháng, năm thứ mấy. Chỉ dạng số ít:
/// "days for shipping" (số ngày giao) là một lượng, lấy trung bình có nghĩa, nên không khớp.
const TIME_POSITION_WORDS: &[&str] = &[
    "date", "day", "week", "month", "year", "ngay", "tuan", "thang",
];

/// Từ làm cả tên thành một lượng chứ không phải mốc: "số ngày giao", "day count", "length day".
const AMOUNT_WORDS: &[&str] = &[
    "so", "count", "num", "number", "length", "duration", "total", "tong",
];

/// Tên cột số nguyên là mốc thời gian theo TIME_POSITION_WORDS (so theo từ, sau khi bỏ dấu và tách camelCase).
pub fn is_time_position_name(name: &str) -> bool {
    let tokens = name_tokens(name);
    tokens
        .iter()
        .any(|t| TIME_POSITION_WORDS.contains(&t.as_str()))
        && !tokens.iter().any(|t| AMOUNT_WORDS.contains(&t.as_str()))
}

/// Khóa của bảng nguồn đọc từ catalog Postgres. Rỗng với file nạp tay.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceKeys {
    /// Cột của khóa chính, theo thứ tự của khóa.
    pub primary_key: Vec<String>,
    /// Cột thuộc ít nhất một khóa ngoại của bảng.
    pub foreign_key_columns: Vec<String>,
}

/// Đặt vai trò key cho cột khóa của database nguồn (xem đầu file). Cột ignore, time giữ nguyên.
pub fn apply_source_keys(columns: &mut [NewColumn], keys: &SourceKeys) {
    for column in columns.iter_mut() {
        if is_source_key(column, keys) {
            column.role = ProfileRole::Key;
        }
    }
}

fn is_source_key(column: &NewColumn, keys: &SourceKeys) -> bool {
    if matches!(column.role, ProfileRole::Ignore | ProfileRole::Time) {
        return false;
    }
    if keys.primary_key.len() == 1 && keys.primary_key[0] == column.name {
        return true;
    }
    let in_key =
        keys.primary_key.contains(&column.name) || keys.foreign_key_columns.contains(&column.name);
    in_key && is_integer_type(&column.data_type) && !is_time_position_name(&column.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_column(name: &str, data_type: &str, role: ProfileRole) -> NewColumn {
        NewColumn {
            position: 0,
            name: name.to_string(),
            data_type: data_type.to_string(),
            role,
            null_count: 0,
            distinct_count: 10,
            min_value: None,
            max_value: None,
            time_format: None,
            is_sensitive: false,
            description: None,
            values: Vec::new(),
        }
    }

    #[test]
    fn time_position_names() {
        for name in [
            "date",
            "date_registration",
            "date_submitted",
            "week_from",
            "presentation_year",
            "Ngày",
            "OrderDay",
        ] {
            assert!(is_time_position_name(name), "{name}");
        }
        for name in [
            "days_for_shipping_real",
            "Days for shipment (scheduled)",
            "Số ngày giao",
            "module_presentation_length",
            "sum_click",
            "Update count",
        ] {
            assert!(!is_time_position_name(name), "{name}");
        }
    }

    /// edu.student_daily_clicks: khóa chính (code_module, code_presentation, id_student, date), khóa ngoại tới students.
    /// Chỉ id_student thành key: hai cột chữ giữ dimension để hỏi theo môn, kỳ; date là mốc thời gian.
    #[test]
    fn source_keys_become_key_for_integer_columns() {
        let mut columns = vec![
            new_column("code_module", "VARCHAR", ProfileRole::Dimension),
            new_column("code_presentation", "VARCHAR", ProfileRole::Dimension),
            new_column("id_student", "INTEGER", ProfileRole::Measure),
            new_column("date", "INTEGER", ProfileRole::Text),
            new_column("sum_click", "INTEGER", ProfileRole::Measure),
        ];
        let keys = SourceKeys {
            primary_key: ["code_module", "code_presentation", "id_student", "date"]
                .map(String::from)
                .to_vec(),
            foreign_key_columns: ["code_module", "code_presentation", "id_student"]
                .map(String::from)
                .to_vec(),
        };
        apply_source_keys(&mut columns, &keys);
        let roles: Vec<ProfileRole> = columns.iter().map(|c| c.role).collect();
        assert_eq!(
            roles,
            [
                ProfileRole::Dimension,
                ProfileRole::Dimension,
                ProfileRole::Key,
                ProfileRole::Text,
                ProfileRole::Measure
            ]
        );
    }

    /// Khóa chính một cột thành key dù là chữ (mã đơn dạng chữ); khóa ngoại số nguyên lặp lại cũng vậy.
    #[test]
    fn single_primary_key_and_foreign_keys() {
        let mut columns = vec![
            new_column("order_code", "VARCHAR", ProfileRole::Text),
            new_column("product_id", "INTEGER", ProfileRole::Measure),
            new_column("order_date", "TIMESTAMP", ProfileRole::Time),
        ];
        let keys = SourceKeys {
            primary_key: vec!["order_code".to_string()],
            foreign_key_columns: vec!["product_id".to_string(), "order_date".to_string()],
        };
        apply_source_keys(&mut columns, &keys);
        assert_eq!(columns[0].role, ProfileRole::Key);
        assert_eq!(columns[1].role, ProfileRole::Key);
        assert_eq!(columns[2].role, ProfileRole::Time);
    }
}
