// SPDX-License-Identifier: Apache-2.0
/* service/explain_period.rs - kỳ của câu "tại sao" (#38): đọc kỳ mô hình ghi trong form ("2017", "2017-Q3", "2017-05"),
đổi thành khoảng ngày [đầu, cuối) cho điều kiện SQL, tìm kỳ liền trước và cùng kỳ năm trước, in nhãn cho người dùng.
explain_check.rs gọi khi kiểm form, explain_sql.rs in khoảng ngày thành điều kiện. Hàm thuần, test ở
explain_check_tests.rs.

Chỉ nhận đúng ba dạng viết, khớp với grain: mô hình viết dạng khác ("Q3 2017", "2017-7") thì kiểm form báo lỗi trường
và mô hình sửa ở vòng sau, thay vì code đoán. Kỳ là khoảng ngày tùy ý thì chưa nhận (ngoài phạm vi của #38). */

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Độ dài một kỳ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Grain {
    Month,
    Quarter,
    Year,
}

impl Grain {
    /// Tên đơn vị của date_trunc trong DuckDB, cũng là tên cột kỳ của bảng tổng theo kỳ.
    pub fn as_str(self) -> &'static str {
        match self {
            Grain::Month => "month",
            Grain::Quarter => "quarter",
            Grain::Year => "year",
        }
    }

    /// Danh từ cho người dùng, viết hoa chữ đầu: "Quý" trong "Quý liền trước".
    pub fn noun(self) -> &'static str {
        match self {
            Grain::Month => "Tháng",
            Grain::Quarter => "Quý",
            Grain::Year => "Năm",
        }
    }

    fn months(self) -> i32 {
        match self {
            Grain::Month => 1,
            Grain::Quarter => 3,
            Grain::Year => 12,
        }
    }
}

/// Một kỳ: tháng, quý hoặc năm. Giữ bằng số tháng tính từ tháng 1 năm 0 của tháng đầu kỳ, để lùi một kỳ hay một năm
/// chỉ là phép trừ, không phải xử lý riêng chuyện qua năm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Period {
    pub grain: Grain,
    first_month: i32,
}

// Năm của kỳ mô hình ghi phải nằm trong khoảng này: ngoài đó NaiveDate vẫn dựng được nhưng không phải dữ liệu thật,
// và nhãn "Năm 0" khó hiểu.
const MIN_YEAR: i32 = 1;
const MAX_YEAR: i32 = 9999;

impl Period {
    /// Đọc kỳ theo grain: năm "2017", quý "2017-Q3", tháng "2017-05". Err là lời tiếng Anh gửi lại cho mô hình.
    pub fn parse(grain: Grain, text: &str) -> Result<Period, String> {
        let text = text.trim();
        let wanted = match grain {
            Grain::Year => "a year such as 2017",
            Grain::Quarter => "a quarter such as 2017-Q3",
            Grain::Month => "a month such as 2017-05",
        };
        let wrong = || format!("\"{text}\" is not {wanted}");
        let (year, rest) = match text.split_once('-') {
            Some((year, rest)) => (year, Some(rest)),
            None => (text, None),
        };
        let year = parse_digits(year, 4).ok_or_else(wrong)?;
        if !(MIN_YEAR..=MAX_YEAR).contains(&year) {
            return Err(wrong());
        }
        let month = match (grain, rest) {
            (Grain::Year, None) => 1,
            (Grain::Quarter, Some(rest)) => {
                let quarter = rest
                    .strip_prefix('Q')
                    .and_then(|n| parse_digits(n, 1))
                    .filter(|n| (1..=4).contains(n))
                    .ok_or_else(wrong)?;
                (quarter - 1) * 3 + 1
            }
            (Grain::Month, Some(rest)) => parse_digits(rest, 2)
                .filter(|n| (1..=12).contains(n))
                .ok_or_else(wrong)?,
            _ => return Err(wrong()),
        };
        Ok(Period {
            grain,
            first_month: year * 12 + month - 1,
        })
    }

    pub fn year(self) -> i32 {
        self.first_month.div_euclid(12)
    }

    // Tháng đầu kỳ, 1 tới 12.
    fn month(self) -> i32 {
        self.first_month.rem_euclid(12) + 1
    }

    /// Kỳ liền trước: quý 3/2017 thành quý 2/2017, tháng 1/2018 thành tháng 12/2017.
    pub fn previous(self) -> Period {
        self.shift(-self.grain.months())
    }

    /// Cùng kỳ năm trước: quý 3/2017 thành quý 3/2016. Với năm thì trùng kỳ liền trước.
    pub fn same_last_year(self) -> Period {
        self.shift(-12)
    }

    fn shift(self, months: i32) -> Period {
        Period {
            grain: self.grain,
            first_month: self.first_month + months,
        }
    }

    /// Ngày đầu kỳ và ngày đầu kỳ sau: dòng thuộc kỳ khi đầu <= ngày < cuối, đúng cả với cột có giờ phút. None khi kỳ
    /// nằm ngoài khoảng ngày chrono biểu diễn được (không xảy ra với năm từ MIN_YEAR tới MAX_YEAR).
    pub fn range(self) -> Option<(NaiveDate, NaiveDate)> {
        let end = self.shift(self.grain.months());
        Some((self.first_day()?, end.first_day()?))
    }

    fn first_day(self) -> Option<NaiveDate> {
        NaiveDate::from_ymd_opt(self.year(), u32::try_from(self.month()).ok()?, 1)
    }

    /// Cách viết trong form: "2017", "2017-Q3", "2017-05".
    pub fn text(self) -> String {
        match self.grain {
            Grain::Year => format!("{}", self.year()),
            Grain::Quarter => format!("{}-Q{}", self.year(), (self.month() - 1) / 3 + 1),
            Grain::Month => format!("{}-{:02}", self.year(), self.month()),
        }
    }

    /// Nhãn cho người dùng: "Năm 2017", "Quý 3/2017", "Tháng 5/2017".
    pub fn label(self) -> String {
        match self.grain {
            Grain::Year => format!("Năm {}", self.year()),
            Grain::Quarter => format!("Quý {}/{}", (self.month() - 1) / 3 + 1, self.year()),
            Grain::Month => format!("Tháng {}/{}", self.month(), self.year()),
        }
    }
}

// Đúng `len` chữ số ASCII; "+7", " 7", "７" (chữ số toàn khổ) đều không nhận, dù str::parse nhận vài dạng trong đó.
fn parse_digits(text: &str, len: usize) -> Option<i32> {
    if text.len() != len || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}
