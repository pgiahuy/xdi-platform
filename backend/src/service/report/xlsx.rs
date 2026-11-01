// SPDX-License-Identifier: Apache-2.0
/* service/report/xlsx.rs - file Excel của một báo cáo đã dựng lại (render.rs), bằng rust_xlsxwriter. Thứ tự sheet:
"Tóm tắt" (tiêu đề, đoạn mở đầu, mỗi mục một dòng: câu hỏi, lời trả lời, kiểm số, kiểm nghĩa, số dòng, sheet chứa bảng),
mỗi mục một sheet "Mục N" (bảng kết quả, phân rã nếu có), rồi "Cách tính" (mỗi mục một dòng: độ đo, bộ lọc, nhóm, kỳ,
SQL đã chạy, bảng và phiên bản, lúc hỏi). Số trên mọi sheet có cột "Mục" hay nằm trong sheet "Mục N", nên truy được về
đúng mục và câu SQL của nó.

Ô số đo (kiểu decimal, chuỗi chữ số) ghi thành số của Excel để người nhận tính tiếp được; Excel giữ 15 chữ số có nghĩa,
nên số dài hơn mất chữ số cuối trong file, còn trên màn hình báo cáo vẫn đúng chuỗi backend tính. */

use chrono::{DateTime, Utc};
use rust_xlsxwriter::{Format, FormatAlign, Workbook, Worksheet, XlsxError};

use crate::error::AppError;
use crate::service::ask::contract::{
    AnswerVerdict, CellValue, QuestionType, ResultTable, Understanding, ValueKind,
};
use crate::service::report::contract::{NumberCheck, ReportBreakdown, ReportItem, ReportResponse};

/// Tên sheet tóm tắt và sheet cách tính. Tên sheet của Excel tối đa 31 ký tự, không có []:*?/\.
const SUMMARY_SHEET: &str = "Tóm tắt";
const METHOD_SHEET: &str = "Cách tính";

/// Độ rộng cột chữ dài (câu hỏi, lời trả lời, SQL), theo đơn vị ký tự của Excel.
const WIDE: f64 = 60.0;
const MEDIUM: f64 = 24.0;

pub(super) fn build_report_xlsx(report: &ReportResponse) -> Result<Vec<u8>, AppError> {
    build_workbook(report).map_err(|e| AppError::Internal(format!("build report xlsx: {e}")))
}

fn build_workbook(report: &ReportResponse) -> Result<Vec<u8>, XlsxError> {
    let mut book = Workbook::new();
    let bold = Format::new().set_bold();
    let wrap = Format::new().set_text_wrap().set_align(FormatAlign::Top);

    let summary = book.add_worksheet();
    write_summary(summary, report, &bold, &wrap)?;
    for (index, item) in report.items.iter().enumerate() {
        let sheet = book.add_worksheet();
        write_item(sheet, index + 1, item, &bold)?;
    }
    let method = book.add_worksheet();
    write_method(method, report, &bold, &wrap)?;
    book.save_to_buffer()
}

fn sheet_name(number: usize) -> String {
    format!("Mục {number}")
}

fn write_summary(
    sheet: &mut Worksheet,
    report: &ReportResponse,
    bold: &Format,
    wrap: &Format,
) -> Result<(), XlsxError> {
    sheet.set_name(SUMMARY_SHEET)?;
    sheet.write_string_with_format(0, 0, &report.report.title, bold)?;
    sheet.write_string(1, 0, &report.report.intro)?;
    sheet.write_string(
        2,
        0,
        format!("Số liệu tính lúc {}", build_time(report.rendered_at)),
    )?;
    let header = [
        "Mục",
        "Câu hỏi",
        "Loại câu hỏi",
        "Lời trả lời",
        "Kiểm số",
        "Kiểm nghĩa",
        "Số dòng kết quả",
        "Bảng ở sheet",
        "Ghi chú",
    ];
    write_header(sheet, 4, &header, bold)?;
    for (index, item) in report.items.iter().enumerate() {
        let row = 5 + index as u32;
        let number = index + 1;
        sheet.write_number(row, 0, number as f64)?;
        sheet.write_string_with_format(row, 1, &item.question, wrap)?;
        sheet.write_string(row, 2, build_question_type(item.question_type))?;
        let answer = item.answer.as_ref().map_or("", |a| a.text.as_str());
        sheet.write_string_with_format(row, 3, answer, wrap)?;
        sheet.write_string(row, 4, build_number_check_text(item.number_check))?;
        sheet.write_string(row, 5, build_meaning_check_text(item.meaning_check))?;
        if let Some(result) = &item.result {
            sheet.write_number(row, 6, result.total_rows as f64)?;
        }
        sheet.write_string(row, 7, sheet_name(number))?;
        let mut notes = item.notes.clone();
        notes.extend(item.error.clone());
        sheet.write_string_with_format(row, 8, notes.join(" "), wrap)?;
    }
    sheet.set_column_width(1, WIDE)?;
    sheet.set_column_width(2, 18.0)?;
    sheet.set_column_width(3, WIDE)?;
    sheet.set_column_width(4, MEDIUM)?;
    sheet.set_column_width(5, MEDIUM)?;
    sheet.set_column_width(6, 16.0)?;
    sheet.set_column_width(7, 12.0)?;
    sheet.set_column_width(8, WIDE)?;
    Ok(())
}

fn write_item(
    sheet: &mut Worksheet,
    number: usize,
    item: &ReportItem,
    bold: &Format,
) -> Result<(), XlsxError> {
    sheet.set_name(sheet_name(number))?;
    sheet.write_string_with_format(0, 0, format!("Mục {number}: {}", item.question), bold)?;
    let mut row = 2;
    if let Some(error) = &item.error {
        sheet.write_string(row, 0, error)?;
        return Ok(());
    }
    if let Some(result) = &item.result {
        let shown = result.rows.len() as u64;
        let note = if shown < result.total_rows {
            format!(
                "Bảng có {} dòng; file có {shown} dòng đầu, như câu trả lời gửi lên giao diện.",
                result.total_rows
            )
        } else {
            format!("Bảng có {} dòng.", result.total_rows)
        };
        sheet.write_string(1, 0, note)?;
        row = write_result(sheet, row, result, bold)?;
    }
    if let Some(breakdown) = &item.breakdown {
        write_breakdown(sheet, row + 1, breakdown, bold)?;
    }
    Ok(())
}

/// Bảng kết quả từ dòng `start`; trả dòng ngay sau bảng.
fn write_result(
    sheet: &mut Worksheet,
    start: u32,
    result: &ResultTable,
    bold: &Format,
) -> Result<u32, XlsxError> {
    for (col, column) in result.columns.iter().enumerate() {
        sheet.write_string_with_format(start, col as u16, &column.name, bold)?;
        sheet.set_column_width(col as u16, MEDIUM)?;
    }
    for (offset, cells) in result.rows.iter().enumerate() {
        let row = start + 1 + offset as u32;
        for (col, cell) in cells.iter().enumerate() {
            let kind = result.columns.get(col).map(|c| c.kind);
            write_cell(sheet, row, col as u16, cell.as_ref(), kind)?;
        }
    }
    Ok(start + 1 + result.rows.len() as u32)
}

fn write_cell(
    sheet: &mut Worksheet,
    row: u32,
    col: u16,
    cell: Option<&CellValue>,
    kind: Option<ValueKind>,
) -> Result<(), XlsxError> {
    match cell {
        None => {}
        Some(CellValue::Number(n)) => {
            sheet.write_number(row, col, *n)?;
        }
        Some(CellValue::Boolean(b)) => {
            sheet.write_boolean(row, col, *b)?;
        }
        Some(CellValue::String(text)) => {
            let numeric = matches!(kind, Some(ValueKind::Decimal | ValueKind::Number));
            match text.parse::<f64>() {
                Ok(n) if numeric && n.is_finite() => sheet.write_number(row, col, n)?,
                _ => sheet.write_string(row, col, text)?,
            };
        }
    }
    Ok(())
}

fn write_breakdown(
    sheet: &mut Worksheet,
    start: u32,
    breakdown: &ReportBreakdown,
    bold: &Format,
) -> Result<(), XlsxError> {
    let title = format!(
        "Phân rã {}: {} {} so với {} {}, chênh lệch {}",
        breakdown.measure,
        breakdown.compare_label,
        breakdown.compare_value,
        breakdown.base_label,
        breakdown.base_value,
        breakdown.change
    );
    sheet.write_string_with_format(start, 0, title, bold)?;
    let header = [
        breakdown.dimension.as_str(),
        breakdown.base_label.as_str(),
        breakdown.compare_label.as_str(),
        "Đóng góp vào chênh lệch",
        "Phần của chênh lệch",
    ];
    write_header(sheet, start + 1, &header, bold)?;
    for (offset, group) in breakdown.groups.iter().enumerate() {
        let row = start + 2 + offset as u32;
        sheet.write_string(row, 0, &group.name)?;
        write_decimal(sheet, row, 1, &group.base_value)?;
        write_decimal(sheet, row, 2, &group.compare_value)?;
        write_decimal(sheet, row, 3, &group.contribution)?;
        if let Some(share) = &group.share_of_change {
            write_decimal(sheet, row, 4, share)?;
        }
    }
    Ok(())
}

fn write_decimal(sheet: &mut Worksheet, row: u32, col: u16, text: &str) -> Result<(), XlsxError> {
    match text.parse::<f64>() {
        Ok(n) if n.is_finite() => sheet.write_number(row, col, n)?,
        _ => sheet.write_string(row, col, text)?,
    };
    Ok(())
}

fn write_method(
    sheet: &mut Worksheet,
    report: &ReportResponse,
    bold: &Format,
    wrap: &Format,
) -> Result<(), XlsxError> {
    sheet.set_name(METHOD_SHEET)?;
    let header = [
        "Mục",
        "Câu hỏi",
        "Độ đo",
        "Bộ lọc",
        "Nhóm theo",
        "Kỳ",
        "Sắp xếp và giới hạn",
        "SQL đã chạy",
        "Bảng và phiên bản",
        "Hỏi lúc",
        "Minh chứng do hệ thống tính",
    ];
    write_header(sheet, 0, &header, bold)?;
    for (index, item) in report.items.iter().enumerate() {
        let row = 1 + index as u32;
        sheet.write_number(row, 0, (index + 1) as f64)?;
        sheet.write_string_with_format(row, 1, &item.question, wrap)?;
        let parts = build_method_parts(item.understanding.as_ref());
        for (offset, text) in parts.iter().enumerate() {
            sheet.write_string_with_format(row, 2 + offset as u16, text, wrap)?;
        }
        sheet.write_string_with_format(row, 7, item.sql.as_deref().unwrap_or(""), wrap)?;
        let tables: Vec<String> = item
            .tables
            .iter()
            .map(|t| {
                format!(
                    "{} phiên bản {} (chụp {})",
                    t.name,
                    t.version,
                    build_time(t.captured_at)
                )
            })
            .collect();
        sheet.write_string_with_format(row, 8, tables.join("; "), wrap)?;
        sheet.write_string(row, 9, build_time(item.asked_at))?;
        let facts = item.facts.as_ref().map_or(String::new(), |f| {
            let mut lines = vec![f.title.clone()];
            lines.extend(f.points.iter().cloned());
            lines.join("\n")
        });
        sheet.write_string_with_format(row, 10, facts, wrap)?;
    }
    sheet.set_column_width(1, WIDE)?;
    for col in 2..=6 {
        sheet.set_column_width(col, MEDIUM)?;
    }
    sheet.set_column_width(7, WIDE)?;
    sheet.set_column_width(8, WIDE)?;
    sheet.set_column_width(9, 20.0)?;
    sheet.set_column_width(10, WIDE)?;
    Ok(())
}

/// Năm cột của "Cách tính": độ đo, bộ lọc (gồm điều kiện phân quyền), nhóm, kỳ, sắp xếp và giới hạn.
pub(super) fn build_method_parts(understanding: Option<&Understanding>) -> [String; 5] {
    let Some(u) = understanding else {
        return Default::default();
    };
    let join = |items: &[crate::service::ask::contract::UnderstoodItem]| {
        items
            .iter()
            .map(|i| i.label.as_str())
            .collect::<Vec<_>>()
            .join("; ")
    };
    let mut order: Vec<&str> = u.order_by.iter().map(|i| i.label.as_str()).collect();
    if let Some(limit) = &u.limit {
        order.push(&limit.label);
    }
    [
        join(&u.measures),
        join(&u.filters),
        join(&u.group_by),
        u.time_range
            .as_ref()
            .map_or(String::new(), |t| t.label.clone()),
        order.join("; "),
    ]
}

fn write_header(
    sheet: &mut Worksheet,
    row: u32,
    names: &[&str],
    bold: &Format,
) -> Result<(), XlsxError> {
    for (col, name) in names.iter().enumerate() {
        sheet.write_string_with_format(row, col as u16, *name, bold)?;
    }
    Ok(())
}

fn build_time(at: DateTime<Utc>) -> String {
    at.format("%Y-%m-%d %H:%M UTC").to_string()
}

fn build_question_type(kind: QuestionType) -> &'static str {
    match kind {
        QuestionType::Quantitative => "Câu hỏi số liệu",
        QuestionType::Causal => "Câu hỏi nguyên nhân",
    }
}

fn build_number_check_text(check: Option<NumberCheck>) -> &'static str {
    match check {
        Some(NumberCheck::Passed) => "Đạt",
        Some(NumberCheck::Replaced) => "Không đạt, đã thay bằng câu hệ thống tính",
        Some(NumberCheck::NotNeeded) => "Không cần: câu do hệ thống viết từ số",
        None => "",
    }
}

fn build_meaning_check_text(verdict: Option<AnswerVerdict>) -> &'static str {
    match verdict {
        Some(AnswerVerdict::Match) => "Khớp",
        Some(AnswerVerdict::Warning) => "Có chỗ chưa rõ",
        // Mismatch của báo cáo luôn là lời của mô hình đã bị thay (report/render.rs, build_checks).
        Some(AnswerVerdict::Mismatch) => "Không đạt, đã thay bằng câu hệ thống tính",
        None => "Không chạy",
    }
}
