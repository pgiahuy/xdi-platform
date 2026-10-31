// SPDX-License-Identifier: Apache-2.0
/* service/sources/plan.rs - chọn bảng nào của một lần chụp được chép dữ liệu, bảng nào chỉ lưu metadata (#247). Hàm
thuần, snapshot.rs gọi trong giao dịch chụp sau khi đếm dòng, test ở cuối file.

Hai trần:
- SOURCE_MAX_ROWS cho từng bảng: chép thiếu dòng thì mọi phép cộng, đếm sai mà không ai thấy, nên bảng quá trần không
  chép dòng nào.
- SOURCE_SNAPSHOT_MAX_MB cho tổng dung lượng Postgres báo (pg_table_size) của các bảng chép trong một lần: mọi file tạm
  của lần chụp cùng nằm trên đĩa tới khi nạp xong. Bảng nhỏ được xếp trước, nên khi quá trần thì các bảng lớn nhất chỉ
  lưu metadata, còn nhiều bảng nhất có dữ liệu. View không có dung lượng (tính 0).

Bảng chỉ lưu metadata vẫn có cột, kiểu, khóa và mô tả; admin chụp riêng nó sau (lời ở reason nói cách). */

use serde::Serialize;

use super::postgres::SourceTable;

/// Why a table keeps metadata only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SkipCause {
    /// More rows than SOURCE_MAX_ROWS.
    RowLimit,
    /// Adding it would take the snapshot over SOURCE_SNAPSHOT_MAX_MB.
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

/// Số dòng của một bảng trong kế hoạch: đếm thật, hay ước lượng khi bảng quá lớn để đếm.
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

/// Một bảng đã đếm, chờ quyết.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub table: SourceTable,
    pub rows: RowCount,
    pub size_bytes: Option<i64>,
}

/// Một bảng chỉ lưu metadata, kèm lý do cho admin.
#[derive(Debug, Clone)]
pub struct Skipped {
    pub candidate: Candidate,
    pub cause: SkipCause,
    pub reason: String,
}

/// Ước lượng của Postgres quá gấp đôi trần thì không đếm thật: count(*) trên bảng trăm triệu dòng tốn cả phút trong
/// giao dịch chụp, mà kết quả chắc chắn vẫn quá trần. Ước lượng lệch tới gấp đôi là hiếm (thống kê cũ sau khi xóa nhiều).
pub fn needs_exact_count(estimated_rows: Option<i64>, max_rows: u64) -> bool {
    match estimated_rows {
        Some(n) => u64::try_from(n).unwrap_or(0) <= max_rows.saturating_mul(2),
        None => true,
    }
}

/// Chia các bảng thành bảng chép (giữ thứ tự gửi vào) và bảng chỉ lưu metadata.
pub fn build_plan(
    candidates: Vec<Candidate>,
    max_rows: u64,
    max_bytes: u64,
) -> (Vec<Candidate>, Vec<Skipped>) {
    let mut skipped = Vec::new();
    let mut fits: Vec<(usize, Candidate)> = Vec::new();
    for (index, c) in candidates.into_iter().enumerate() {
        if u64::try_from(c.rows.value()).unwrap_or(0) > max_rows {
            let reason = build_row_reason(&c, max_rows);
            skipped.push(Skipped {
                candidate: c,
                cause: SkipCause::RowLimit,
                reason,
            });
        } else {
            fits.push((index, c));
        }
    }
    // Bảng nhỏ trước; cùng dung lượng thì theo thứ tự gửi vào, để kết quả không đổi giữa hai lần chụp.
    fits.sort_by_key(|(index, c)| (size_of(c), *index));
    let mut total: u64 = 0;
    let mut taken: Vec<(usize, Candidate)> = Vec::new();
    for (index, c) in fits {
        let size = size_of(&c);
        if total.saturating_add(size) > max_bytes {
            let reason = build_size_reason(&c, max_bytes);
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

fn build_row_reason(c: &Candidate, max_rows: u64) -> String {
    let rows = match c.rows {
        RowCount::Exact(n) => format!("{n} dòng"),
        RowCount::Estimate(n) => format!("khoảng {n} dòng (ước lượng của Postgres)"),
    };
    format!(
        "Bảng {} có {rows}, quá giới hạn {max_rows} dòng của một bảng (SOURCE_MAX_ROWS): mới lưu cột, kiểu, khóa và mô \
         tả, chưa có dữ liệu. Tăng SOURCE_MAX_ROWS rồi chụp riêng bảng này",
        c.table.full_name()
    )
}

fn build_size_reason(c: &Candidate, max_bytes: u64) -> String {
    let size_mb = size_of(c).div_ceil(1024 * 1024);
    let max_mb = max_bytes / 1024 / 1024;
    let advice = if size_of(c) > max_bytes {
        "Tăng SOURCE_SNAPSHOT_MAX_MB rồi chụp riêng bảng này"
    } else {
        "Chụp riêng bảng này, hoặc tăng SOURCE_SNAPSHOT_MAX_MB"
    };
    format!(
        "Bảng {} khoảng {size_mb} MB (theo Postgres); cộng thêm nó thì lần chụp quá {max_mb} MB \
         (SOURCE_SNAPSHOT_MAX_MB): mới lưu cột, kiểu, khóa và mô tả, chưa có dữ liệu. {advice}",
        c.table.full_name()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: i64 = 1024 * 1024;

    fn candidate(name: &str, rows: RowCount, size_mb: Option<i64>) -> Candidate {
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

    fn names(list: &[Candidate]) -> Vec<&str> {
        list.iter().map(|c| c.table.name.as_str()).collect()
    }

    #[test]
    fn rows_over_the_limit_keep_metadata_only() {
        let (taken, skipped) = build_plan(
            vec![
                candidate("courses", RowCount::Exact(22), Some(1)),
                candidate("clicks", RowCount::Exact(1_808_119), Some(104)),
                candidate("guess", RowCount::Estimate(9_000_000), Some(500)),
            ],
            1_000_000,
            2048 * MB as u64,
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

    /// Trần dung lượng giữ bảng nhỏ, bỏ bảng lớn nhất; thứ tự bảng chép giữ như gửi vào; view (không dung lượng) luôn vừa.
    #[test]
    fn size_limit_drops_the_biggest_tables() {
        let (taken, skipped) = build_plan(
            vec![
                candidate("big", RowCount::Exact(10), Some(60)),
                candidate("small", RowCount::Exact(10), Some(10)),
                candidate("view", RowCount::Exact(10), None),
                candidate("mid", RowCount::Exact(10), Some(30)),
            ],
            1_000_000,
            50 * MB as u64,
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
    fn exact_count_is_skipped_only_far_over_the_limit() {
        assert!(needs_exact_count(None, 100));
        assert!(needs_exact_count(Some(150), 100));
        assert!(needs_exact_count(Some(200), 100));
        assert!(!needs_exact_count(Some(201), 100));
    }
}
