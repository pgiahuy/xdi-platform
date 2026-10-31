// SPDX-License-Identifier: Apache-2.0
/* service/sources/plan.rs - chọn bảng nào của một lần chụp được chép dữ liệu, bảng nào chỉ lưu metadata (#247). Hàm
thuần, copy.rs gọi trong giao dịch chụp, catalog.rs gọi cho nhãn của màn kết nối; test ở cuối file.

Trần tính bằng dung lượng Postgres báo (pg_table_size, chưa nén), không bằng số dòng: một triệu dòng ba cột và một
triệu dòng hai trăm cột khác nhau cả chục lần về thời gian chép và đĩa. Dung lượng là số thật của file trên đĩa, có
trước khi chép, nên bảng quá trần bị loại trước mà không ghi file nào. Ba trần:
- SOURCE_TABLE_MAX_MB cho từng bảng.
- SOURCE_SNAPSHOT_MAX_MB cho tổng các bảng chép trong một lần: mọi file tạm của lần chụp cùng nằm trên đĩa tới khi nạp
  xong. Bảng nhỏ được xếp trước, nên khi quá trần thì các bảng lớn nhất chỉ lưu metadata, còn nhiều bảng nhất có dữ
  liệu. View không có dung lượng (tính 0); file của view kiểm sau khi ghi (direct.rs, postgres.rs).
- SOURCE_MAX_ROWS cho từng bảng, chỉ khi admin đặt (mặc định không có). Chép thiếu dòng thì mọi phép cộng, đếm sai mà
  không ai thấy, nên bảng quá trần không chép dòng nào.

Bảng chỉ lưu metadata vẫn có cột, kiểu, khóa và mô tả; admin chụp riêng nó sau (lời ở reason nói cách). */

use serde::Serialize;

use super::postgres::SourceTable;
use crate::config::Config;

/// Why a table keeps metadata only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SkipCause {
    /// More rows than SOURCE_MAX_ROWS (only when that limit is set).
    RowLimit,
    /// Larger than SOURCE_TABLE_MAX_MB, or adding it would take the snapshot over SOURCE_SNAPSHOT_MAX_MB.
    SizeLimit,
}

impl SkipCause {
    /// Chữ ghi vào source_metadata_tables.cause (CHECK của migration 0018).
    pub fn as_str(self) -> &'static str {
        match self {
            SkipCause::RowLimit => "row_limit",
            SkipCause::SizeLimit => "size_limit",
        }
    }

    /// Ngược của as_str, cho dòng đọc lại từ source_metadata_tables. CHECK của migration chỉ cho hai giá trị, nên giá
    /// trị lạ (sửa tay bằng SQL) coi là row_limit thay vì làm hỏng cả danh sách.
    pub fn from_stored(text: &str) -> Self {
        match text {
            "size_limit" => SkipCause::SizeLimit,
            _ => SkipCause::RowLimit,
        }
    }
}

/// Các trần của một lần chụp, lấy từ cấu hình.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// SOURCE_MAX_ROWS; None là không có trần dòng.
    pub max_rows: Option<u64>,
    /// SOURCE_TABLE_MAX_MB, tính bằng byte.
    pub table_max_bytes: u64,
    /// SOURCE_SNAPSHOT_MAX_MB, tính bằng byte.
    pub total_max_bytes: u64,
}

impl Limits {
    pub fn from_config(config: &Config) -> Self {
        Limits {
            max_rows: config.source_max_rows,
            table_max_bytes: config.source_table_max_bytes,
            total_max_bytes: config.source_snapshot_max_bytes,
        }
    }
}

/// Số dòng của một bảng trong kế hoạch: đếm thật, hay ước lượng của Postgres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowCount {
    Exact(i64),
    Estimate(i64),
}

impl RowCount {
    pub fn value(self) -> i64 {
        match self {
            RowCount::Exact(n) | RowCount::Estimate(n) => n,
        }
    }
}

/// Một bảng đã thống kê, chờ quyết.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub table: SourceTable,
    /// None khi không đếm (không có trần dòng) và Postgres chưa có thống kê.
    pub rows: Option<RowCount>,
    pub size_bytes: Option<i64>,
}

/// Một bảng chỉ lưu metadata, kèm lý do cho admin.
#[derive(Debug, Clone)]
pub struct Skipped {
    pub candidate: Candidate,
    pub cause: SkipCause,
    pub reason: String,
}

/// Có cần count(*) không. Không có trần dòng thì không đếm: số dòng không quyết gì, mà count(*) trên bảng chục triệu dòng
/// tốn vài giây trong giao dịch chụp. Có trần mà ước lượng của Postgres đã quá gấp đôi trần thì cũng không đếm: kết quả
/// chắc chắn vẫn quá trần. Ước lượng lệch tới gấp đôi là hiếm (thống kê cũ sau khi xóa nhiều).
pub fn needs_exact_count(estimated_rows: Option<i64>, max_rows: Option<u64>) -> bool {
    let Some(max_rows) = max_rows else {
        return false;
    };
    match estimated_rows {
        Some(n) => u64::try_from(n).unwrap_or(0) <= max_rows.saturating_mul(2),
        None => true,
    }
}

/// Trần mà riêng một bảng đã vượt (chưa tính tổng của lần chụp); None khi bảng vừa. Dùng cả cho nhãn ở màn kết nối
/// (catalog.rs), với số dòng ước lượng.
pub fn build_table_cause(
    rows: Option<i64>,
    size_bytes: Option<i64>,
    limits: &Limits,
) -> Option<SkipCause> {
    let over_rows = match (rows, limits.max_rows) {
        (Some(n), Some(max)) => u64::try_from(n).unwrap_or(0) > max,
        _ => false,
    };
    if over_rows {
        return Some(SkipCause::RowLimit);
    }
    let size = size_bytes.and_then(|s| u64::try_from(s).ok()).unwrap_or(0);
    (size > limits.table_max_bytes).then_some(SkipCause::SizeLimit)
}

/// Chia các bảng thành bảng chép (giữ thứ tự gửi vào) và bảng chỉ lưu metadata.
pub fn build_plan(candidates: Vec<Candidate>, limits: &Limits) -> (Vec<Candidate>, Vec<Skipped>) {
    let mut skipped = Vec::new();
    let mut fits: Vec<(usize, Candidate)> = Vec::new();
    for (index, c) in candidates.into_iter().enumerate() {
        let rows = c.rows.map(RowCount::value);
        match build_table_cause(rows, c.size_bytes, limits) {
            Some(cause) => {
                let reason = match cause {
                    SkipCause::RowLimit => build_row_reason(&c, limits),
                    SkipCause::SizeLimit => build_table_size_reason(&c, limits),
                };
                skipped.push(Skipped {
                    candidate: c,
                    cause,
                    reason,
                });
            }
            None => fits.push((index, c)),
        }
    }
    // Bảng nhỏ trước; cùng dung lượng thì theo thứ tự gửi vào, để kết quả không đổi giữa hai lần chụp.
    fits.sort_by_key(|(index, c)| (size_of(c), *index));
    let mut total: u64 = 0;
    let mut taken: Vec<(usize, Candidate)> = Vec::new();
    for (index, c) in fits {
        let size = size_of(&c);
        if total.saturating_add(size) > limits.total_max_bytes {
            let reason = build_total_reason(&c, limits);
            skipped.push(Skipped {
                candidate: c,
                cause: SkipCause::SizeLimit,
                reason,
            });
        } else {
            total += size;
            taken.push((index, c));
        }
    }
    taken.sort_by_key(|(index, _)| *index);
    (taken.into_iter().map(|(_, c)| c).collect(), skipped)
}

fn size_of(c: &Candidate) -> u64 {
    c.size_bytes
        .and_then(|s| u64::try_from(s).ok())
        .unwrap_or(0)
}

fn to_mb(bytes: u64) -> u64 {
    bytes.div_ceil(1024 * 1024)
}

fn build_row_reason(c: &Candidate, limits: &Limits) -> String {
    let rows = match c.rows {
        Some(RowCount::Exact(n)) => format!("{n} dòng"),
        Some(RowCount::Estimate(n)) => format!("khoảng {n} dòng (ước lượng của Postgres)"),
        None => "quá nhiều dòng".to_string(),
    };
    format!(
        "Bảng {} có {rows}, quá giới hạn {} dòng của một bảng (SOURCE_MAX_ROWS): mới lưu cột, kiểu, khóa và mô \
         tả, chưa có dữ liệu. Tăng hay bỏ SOURCE_MAX_ROWS rồi chụp riêng bảng này",
        c.table.full_name(),
        limits.max_rows.unwrap_or(0)
    )
}

fn build_table_size_reason(c: &Candidate, limits: &Limits) -> String {
    format!(
        "Bảng {} khoảng {} MB (theo Postgres), quá giới hạn {} MB của một bảng (SOURCE_TABLE_MAX_MB): mới lưu cột, \
         kiểu, khóa và mô tả, chưa có dữ liệu. Tăng SOURCE_TABLE_MAX_MB rồi chụp riêng bảng này",
        c.table.full_name(),
        to_mb(size_of(c)),
        limits.table_max_bytes / 1024 / 1024
    )
}

fn build_total_reason(c: &Candidate, limits: &Limits) -> String {
    let advice = if size_of(c) > limits.total_max_bytes {
        "Tăng SOURCE_SNAPSHOT_MAX_MB rồi chụp riêng bảng này"
    } else {
        "Chụp riêng bảng này, hoặc tăng SOURCE_SNAPSHOT_MAX_MB"
    };
    format!(
        "Bảng {} khoảng {} MB (theo Postgres); cộng thêm nó thì lần chụp quá {} MB \
         (SOURCE_SNAPSHOT_MAX_MB): mới lưu cột, kiểu, khóa và mô tả, chưa có dữ liệu. {advice}",
        c.table.full_name(),
        to_mb(size_of(c)),
        limits.total_max_bytes / 1024 / 1024
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: i64 = 1024 * 1024;

    fn candidate(name: &str, rows: Option<RowCount>, size_mb: Option<i64>) -> Candidate {
        Candidate {
            table: SourceTable {
                schema: "edu".to_string(),
                name: name.to_string(),
                kind: "table".to_string(),
            },
            rows,
            size_bytes: size_mb.map(|m| m * MB),
        }
    }

    fn limits(max_rows: Option<u64>, table_mb: u64, total_mb: u64) -> Limits {
        Limits {
            max_rows,
            table_max_bytes: table_mb * MB as u64,
            total_max_bytes: total_mb * MB as u64,
        }
    }

    fn names(list: &[Candidate]) -> Vec<&str> {
        list.iter().map(|c| c.table.name.as_str()).collect()
    }

    /// Không đặt SOURCE_MAX_ROWS: số dòng không quyết gì, bảng vừa trần dung lượng thì chép dù nhiều dòng.
    #[test]
    fn no_row_limit_by_default() {
        let (taken, skipped) = build_plan(
            vec![
                candidate("clicks", Some(RowCount::Estimate(90_000_000)), Some(900)),
                candidate("fresh", None, Some(5)),
            ],
            &limits(None, 2048, 4096),
        );
        assert_eq!(names(&taken), ["clicks", "fresh"]);
        assert!(skipped.is_empty());
    }

    #[test]
    fn rows_over_the_limit_keep_metadata_only() {
        let (taken, skipped) = build_plan(
            vec![
                candidate("courses", Some(RowCount::Exact(22)), Some(1)),
                candidate("clicks", Some(RowCount::Exact(1_808_119)), Some(104)),
                candidate("guess", Some(RowCount::Estimate(9_000_000)), Some(500)),
            ],
            &limits(Some(1_000_000), 2048, 4096),
        );
        assert_eq!(names(&taken), ["courses"]);
        assert_eq!(skipped.len(), 2);
        assert!(skipped.iter().all(|s| s.cause == SkipCause::RowLimit));
        assert!(
            skipped[0].reason.contains("edu.clicks có 1808119 dòng"),
            "{}",
            skipped[0].reason
        );
        assert!(
            skipped[1].reason.contains("khoảng 9000000 dòng"),
            "{}",
            skipped[1].reason
        );
    }

    /// Bảng một mình đã quá SOURCE_TABLE_MAX_MB chỉ lưu metadata trước khi chép, dù tổng còn chỗ; bảng khác vẫn chép.
    #[test]
    fn table_over_its_own_limit_keeps_metadata_only() {
        let (taken, skipped) = build_plan(
            vec![
                candidate("huge", Some(RowCount::Estimate(10)), Some(300)),
                candidate("small", Some(RowCount::Estimate(10)), Some(10)),
            ],
            &limits(None, 100, 4096),
        );
        assert_eq!(names(&taken), ["small"]);
        assert_eq!(skipped[0].cause, SkipCause::SizeLimit);
        let reason = &skipped[0].reason;
        assert!(reason.contains("edu.huge khoảng 300 MB"), "{reason}");
        assert!(reason.contains("SOURCE_TABLE_MAX_MB"), "{reason}");
    }

    /// Trần tổng giữ bảng nhỏ, bỏ bảng lớn nhất; thứ tự bảng chép giữ như gửi vào; view (không dung lượng) luôn vừa.
    #[test]
    fn total_limit_drops_the_biggest_tables() {
        let rows = Some(RowCount::Exact(10));
        let (taken, skipped) = build_plan(
            vec![
                candidate("big", rows, Some(60)),
                candidate("small", rows, Some(10)),
                candidate("view", rows, None),
                candidate("mid", rows, Some(30)),
            ],
            &limits(Some(1_000_000), 2048, 50),
        );
        assert_eq!(names(&taken), ["small", "view", "mid"]);
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].cause, SkipCause::SizeLimit);
        // Một mình bảng đã quá trần: chụp riêng không đủ, lời khuyên tăng trần.
        assert!(
            skipped[0].reason.contains("edu.big khoảng 60 MB"),
            "{}",
            skipped[0].reason
        );
        assert!(
            skipped[0].reason.contains("Tăng SOURCE_SNAPSHOT_MAX_MB"),
            "{}",
            skipped[0].reason
        );
    }

    #[test]
    fn exact_count_only_with_a_row_limit() {
        assert!(!needs_exact_count(None, None));
        assert!(!needs_exact_count(Some(150), None));
        assert!(needs_exact_count(None, Some(100)));
        assert!(needs_exact_count(Some(150), Some(100)));
        assert!(needs_exact_count(Some(200), Some(100)));
        assert!(!needs_exact_count(Some(201), Some(100)));
    }
}
