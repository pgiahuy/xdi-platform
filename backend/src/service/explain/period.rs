// SPDX-License-Identifier: Apache-2.0
/* service/explain/period.rs - kỳ của câu "tại sao" (#38): đọc kỳ mô hình ghi trong form ("2017", "2017-Q3", "2017-05",
tuần "2017-W22", ngày "2017-05-31"), đổi thành khoảng ngày [đầu, cuối) cho điều kiện SQL, tìm kỳ liền trước và cùng kỳ
năm trước, in nhãn cho người dùng. explain/check.rs gọi khi kiểm form, explain/sql.rs in khoảng ngày thành điều kiện,
explain/filter.rs đọc kỳ trong bộ lọc ngày. Hàm thuần, test ở explain/check_tests.rs và explain/frames_tests.rs.

Chỉ nhận đúng năm dạng viết, khớp với grain: mô hình viết dạng khác ("Q3 2017", "2017-7") thì kiểm form báo lỗi trường
và mô hình sửa ở vòng sau, thay vì code đoán. Tuần theo ISO 8601 (bắt đầu thứ Hai), đúng như date_trunc('week') của
DuckDB, để kỳ của điều kiện lọc và kỳ của bảng tổng là một. */

use chrono::{Datelike, Months, NaiveDate, TimeDelta, Weekday};
use serde::{Deserialize, Serialize};

/// Độ dài một kỳ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Grain {
    Day,
    Week,
    Month,
    Quarter,
    Year,
}

impl Grain {
    /// Tên đơn vị của date_trunc trong DuckDB, cũng là tên cột kỳ của bảng tổng theo kỳ.
    pub fn as_str(self) -> &'static str {
        match self {
            Grain::Day => "day",
            Grain::Week => "week",
            Grain::Month => "month",
            Grain::Quarter => "quarter",
            Grain::Year => "year",
        }
    }

    /// Danh từ cho người dùng, viết hoa chữ đầu: "Quý" trong "Quý liền trước".
    pub fn noun(self) -> &'static str {
        match self {
            Grain::Day => "Ngày",
            Grain::Week => "Tuần",
            Grain::Month => "Tháng",
            Grain::Quarter => "Quý",
            Grain::Year => "Năm",
        }
    }

    /// Số tháng của một kỳ với tháng, quý, năm; None với tuần và ngày (đếm bằng ngày).
    fn months(self) -> Option<u32> {
        match self {
            Grain::Month => Some(1),
            Grain::Quarter => Some(3),
            Grain::Year => Some(12),
            Grain::Week | Grain::Day => None,
        }
    }

    /// Số ngày của một kỳ với tuần và ngày.
    fn days(self) -> i64 {
        match self {
            Grain::Week => 7,
            _ => 1,
        }
    }
}

/// Một kỳ: ngày, tuần, tháng, quý hoặc năm, giữ bằng ngày đầu kỳ. Ngày đầu luôn đúng mốc của grain (ngày 1 của tháng
/// đầu quý, thứ Hai của tuần), vì mọi Period chỉ dựng qua parse và containing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Period {
    pub grain: Grain,
    start: NaiveDate,
}

// Năm của kỳ mô hình ghi phải nằm trong khoảng này: ngoài đó NaiveDate vẫn dựng được nhưng không phải dữ liệu thật,
// và nhãn "Năm 0" khó hiểu.
const MIN_YEAR: i32 = 1;
const MAX_YEAR: i32 = 9999;

impl Period {
    /// Đọc kỳ theo grain: năm "2017", quý "2017-Q3", tháng "2017-05", tuần "2017-W22", ngày "2017-05-31". Err là lời
    /// tiếng Anh gửi lại cho mô hình.
    pub fn parse(grain: Grain, text: &str) -> Result<Period, String> {
        let text = text.trim();
        let wanted = match grain {
            Grain::Year => "a year such as 2017",
            Grain::Quarter => "a quarter such as 2017-Q3",
            Grain::Month => "a month such as 2017-05",
            Grain::Week => "an ISO week such as 2017-W22",
            Grain::Day => "a day such as 2017-05-31",
        };
        let wrong = || format!("\"{text}\" is not {wanted}");
        let parts: Vec<&str> = text.split('-').collect();
        let year = parts
            .first()
            .and_then(|y| parse_digits(y, 4))
            .filter(|y| (MIN_YEAR..=MAX_YEAR).contains(y))
            .ok_or_else(wrong)?;
        let start = match (grain, &parts[1..]) {
            (Grain::Year, []) => NaiveDate::from_ymd_opt(year, 1, 1),
            (Grain::Quarter, [quarter]) => quarter
                .strip_prefix('Q')
                .and_then(|n| parse_digits(n, 1))
                .filter(|n| (1..=4).contains(n))
                .and_then(|n| NaiveDate::from_ymd_opt(year, (n as u32 - 1) * 3 + 1, 1)),
            (Grain::Month, [month]) => {
                parse_digits(month, 2).and_then(|m| NaiveDate::from_ymd_opt(year, m as u32, 1))
            }
            (Grain::Week, [week]) => week
                .strip_prefix('W')
                .and_then(|n| parse_digits(n, 2))
                .and_then(|n| NaiveDate::from_isoywd_opt(year, n as u32, Weekday::Mon)),
            (Grain::Day, [month, day]) => match (parse_digits(month, 2), parse_digits(day, 2)) {
                (Some(m), Some(d)) => NaiveDate::from_ymd_opt(year, m as u32, d as u32),
                _ => None,
            },
            _ => None,
        };
        let start = start.ok_or_else(wrong)?;
        Ok(Period { grain, start })
    }

    /// Kỳ chứa một ngày: ngày 15/11/2017 theo quý là quý 4/2017, theo tuần là tuần bắt đầu thứ Hai 13/11/2017.
    pub fn containing(grain: Grain, date: NaiveDate) -> Period {
        let first_of_month = |month: u32| date.with_day(1).and_then(|d| d.with_month(month));
        let start = match grain {
            Grain::Day => Some(date),
            Grain::Week => {
                Some(date - TimeDelta::days(i64::from(date.weekday().num_days_from_monday())))
            }
            Grain::Month => first_of_month(date.month()),
            // Tháng đầu quý là 1, 4, 7, 10.
            Grain::Quarter => first_of_month(date.month0() / 3 * 3 + 1),
            Grain::Year => first_of_month(1),
        };
        Period {
            grain,
            // with_day(1), with_month của ngày 1 luôn dựng được; giữ ngày gốc chỉ để không phải trả Option.
            start: start.unwrap_or(date),
        }
    }

    /// Kỳ liền trước: quý 3/2017 thành quý 2/2017, tháng 1/2018 thành tháng 12/2017, tuần thành tuần trước.
    pub fn previous(self) -> Period {
        self.shifted(false)
    }

    /// Cùng kỳ năm trước: quý 3/2017 thành quý 3/2016. Với năm thì trùng kỳ liền trước. Tuần lùi 52 tuần để vẫn bắt
    /// đầu thứ Hai; ngày 29/2 lùi về 28/2.
    pub fn same_last_year(self) -> Period {
        let start = match self.grain {
            Grain::Week => self.start.checked_sub_signed(TimeDelta::weeks(52)),
            _ => self.start.checked_sub_months(Months::new(12)),
        };
        self.with_start(start)
    }

    /// Kỳ sau (forward true) hay kỳ trước.
    fn shifted(self, forward: bool) -> Period {
        let start = match self.grain.months() {
            Some(months) if forward => self.start.checked_add_months(Months::new(months)),
            Some(months) => self.start.checked_sub_months(Months::new(months)),
            None => {
                let days = TimeDelta::days(self.grain.days());
                if forward {
                    self.start.checked_add_signed(days)
                } else {
                    self.start.checked_sub_signed(days)
                }
            }
        };
        self.with_start(start)
    }

    // Ra ngoài khoảng ngày chrono biểu diễn được thì giữ nguyên kỳ: explain/check.rs báo base trùng compare.
    fn with_start(self, start: Option<NaiveDate>) -> Period {
        Period {
            grain: self.grain,
            start: start.unwrap_or(self.start),
        }
    }

    /// Ngày đầu kỳ và ngày đầu kỳ sau: dòng thuộc kỳ khi đầu <= ngày < cuối, đúng cả với cột có giờ phút. None khi kỳ
    /// sau nằm ngoài khoảng ngày chrono biểu diễn được (không xảy ra với năm từ MIN_YEAR tới MAX_YEAR).
    pub fn range(self) -> Option<(NaiveDate, NaiveDate)> {
        let end = self.shifted(true).start;
        (end > self.start).then_some((self.start, end))
    }

    /// Cách viết trong form: "2017", "2017-Q3", "2017-05", "2017-W22", "2017-05-31".
    pub fn text(self) -> String {
        let (year, month) = (self.start.year(), self.start.month());
        match self.grain {
            Grain::Year => format!("{year}"),
            Grain::Quarter => format!("{year}-Q{}", (month - 1) / 3 + 1),
            Grain::Month => format!("{year}-{month:02}"),
            Grain::Week => {
                let week = self.start.iso_week();
                format!("{}-W{:02}", week.year(), week.week())
            }
            Grain::Day => self.start.format("%Y-%m-%d").to_string(),
        }
    }

    /// Nhãn cho người dùng: "Năm 2017", "Quý 3/2017", "Tháng 5/2017", "Tuần 29/5/2017 - 4/6/2017", "Ngày 31/5/2017".
    /// Tuần ghi ngày đầu và ngày cuối thay cho số tuần ISO, vì người đọc ít khi biết tuần 22 là tuần nào.
    pub fn label(self) -> String {
        let (year, month) = (self.start.year(), self.start.month());
        let day = |date: NaiveDate| format!("{}/{}/{}", date.day(), date.month(), date.year());
        match self.grain {
            Grain::Year => format!("Năm {year}"),
            Grain::Quarter => format!("Quý {}/{year}", (month - 1) / 3 + 1),
            Grain::Month => format!("Tháng {month}/{year}"),
            Grain::Week => {
                let last = self.start + TimeDelta::days(6);
                format!("Tuần {} - {}", day(self.start), day(last))
            }
            Grain::Day => format!("Ngày {}", day(self.start)),
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
