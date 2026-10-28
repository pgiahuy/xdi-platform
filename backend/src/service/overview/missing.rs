// SPDX-License-Identifier: Apache-2.0
/* service/overview/missing.rs - lời báo các phần trang tổng quan chưa có hay không hiện được (MissingPart), để người xem
dataset đang lập hồ sơ biết phần nào sẽ có sau, phần nào không bao giờ có với dữ liệu này. Lời là tiếng Việt vì người
đọc là người dùng cuối, như Warning của câu hỏi. */

use crate::service::overview::contract::{MissingCode, MissingPart};

pub fn build_missing(code: MissingCode) -> MissingPart {
    let message = match code {
        MissingCode::NoVersion => "Dataset chưa có dữ liệu; nạp file ở trang Dữ liệu.",
        MissingCode::ProfilePending => {
            "Đang lập hồ sơ cột: danh sách cột, số liệu và biểu đồ sẽ có sau ít phút."
        }
        MissingCode::ProfileFailed => {
            "Lập hồ sơ phiên bản này lỗi nên chưa có danh sách cột và biểu đồ; nạp lại file ở trang Dữ liệu."
        }
        MissingCode::SemanticPending => {
            "Đang viết nhãn tiếng Việt, mô tả và đơn vị của cột; các phần này sẽ có sau ít phút."
        }
        MissingCode::SemanticUnavailable => {
            "Mô hình hiện tại không viết được nhãn tiếng Việt, mô tả và đơn vị cột; cột hiện theo tên trong file."
        }
        MissingCode::Restricted => {
            "Vai dữ liệu của bạn chỉ xem được một phần dòng. Số ô trống, số giá trị khác nhau và số dòng theo giá trị \
             tính trên mọi dòng nên không hiện; số dòng, khoảng thời gian và biểu đồ theo thời gian tính trên phần bạn \
             được xem."
        }
        MissingCode::NoTime => {
            "Dataset không có cột thời gian nên không có khoảng thời gian và biểu đồ theo thời gian."
        }
        MissingCode::NoMeasure => {
            "Chưa có chỉ số đã xác nhận hay cột số đo tiền, số lượng nên chưa có biểu đồ độ đo theo thời gian."
        }
        // Lời của biểu đồ lỗi kèm lý do, dựng ở build_chart_failed.
        MissingCode::ChartFailed => "Không tính được một biểu đồ.",
    };
    MissingPart {
        code,
        message: message.to_string(),
    }
}

/// what: phần không tính được, ví dụ "số dòng theo tháng"; reason: lời của guard hay DuckDB, đã bỏ đường dẫn file.
pub fn build_chart_failed(what: &str, reason: &str) -> MissingPart {
    MissingPart {
        code: MissingCode::ChartFailed,
        message: format!("Không tính được {what}: {reason}"),
    }
}
