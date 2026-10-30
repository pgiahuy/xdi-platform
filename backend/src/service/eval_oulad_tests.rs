// SPDX-License-Identifier: Apache-2.0
/* service/eval_oulad_tests.rs - kiểm bộ câu OULAD (eval/oulad.json, #202).

Hai phần:
- Định dạng (luôn chạy): trường bắt buộc, loại câu, nhóm, số cách hỏi, câu phải từ chối không có SQL, mỗi expected_sql
  có một check_sql đi kèm; và runner (build_phrasings) bỏ qua đúng các ca chưa chạy được.
- SQL đáp án trên dữ liệu thật (chỉ chạy khi đặt OULAD_DIR, thư mục 7 file CSV của data/download.sh oulad; `make
  eval-oulad-check`): nhóm single nạp từng CSV qua đúng hàm nạp của app (analytics::materialize) nên tên cột và kiểu
  cột như khi người dùng nạp file; nhóm multi dựng bảng edu từ cùng các CSV theo cách của
  data/postgres-demo/sql/edu_load.sql, kiểu cột như khi chụp bảng Postgres (sources/pg_types.rs). Mỗi expected_sql và
  check_sql đi kèm (câu viết riêng tính lại cùng đáp án) chạy qua guard rồi DuckDB, kết quả phải bằng nhau sau khi làm
  tròn 4 chữ số có nghĩa, không xét thứ tự dòng trừ ca ordered. Không đặt OULAD_DIR thì test này in lý do và qua. */

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use super::eval::{build_phrasings, EvalCase};
use crate::engine::analytics::{materialize, FileFormat, InputFile};
use crate::engine::guard::{build_guarded_sql, AllowedTable};
use crate::engine::query::run_query;
use crate::service::ask::contract::CellValue;
use crate::service::ask::result::build_result_table;

const CASES: &str = include_str!("../../../eval/oulad.json");

/// Loại câu theo #202. Ba loại cuối là câu thường khó vừa; các loại còn lại trừ plain là câu cần giải thích.
const KINDS: [&str; 12] = [
    "plain",
    "why_periods",
    "why_groups",
    "ratio",
    "why_implicit",
    "false_premise",
    "why_no_period",
    "follow_up",
    "refuse",
    "same_period",
    "top_n_group",
    "no_diacritics",
];
const FANOUT_TRAP: &str = "fanout_trap";

/// File CSV của OULAD và bảng logic app tạo khi nạp từng file không đặt tên (tên file viết thường). studentVle.csv
/// (454 MB) không có câu nào ở nhóm single nên không nạp.
const SINGLE_FILES: [&str; 6] = [
    "courses",
    "studentInfo",
    "studentRegistration",
    "assessments",
    "studentAssessment",
    "vle",
];

fn cases_json() -> Vec<Value> {
    serde_json::from_str(CASES).expect("eval/oulad.json parses")
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn every_case_has_the_fields_of_issue_202() {
    let cases = cases_json();
    let mut ids = BTreeSet::new();
    for case in &cases {
        let id = case["id"].as_str().expect("id");
        assert!(ids.insert(id.to_string()), "{id}: duplicate id");
        let kind = case["kind"].as_str().expect("kind");
        let group = case["group"].as_str().expect("group");
        assert!(
            KINDS.contains(&kind) || kind == FANOUT_TRAP,
            "{id}: unknown kind {kind}"
        );
        assert!(group == "single" || group == "multi", "{id}: group");
        let question = case["question"].as_str().expect("question");
        let variants = strings(&case["variants"]);
        assert_eq!(variants.len(), 2, "{id}: two variants");
        assert!(
            variants.iter().all(|v| !v.is_empty() && v != question),
            "{id}: variants"
        );
        let tables = strings(&case["tables"]);
        assert!(!tables.is_empty(), "{id}: tables");
        if group == "single" {
            assert_eq!(tables.len(), 1, "{id}: a single case reads one table");
        }
        assert!(
            !case["author"].as_str().unwrap_or("").is_empty(),
            "{id}: author"
        );
        assert!(case["reviewers"].is_array(), "{id}: reviewers");
        assert!(case["ordered"].is_boolean(), "{id}: ordered");
        let expected = strings(&case["expected_sql"]);
        let check = strings(&case["check_sql"]);
        let refuse = case["must_refuse"].as_bool().expect("must_refuse");
        assert_eq!(refuse, kind == "refuse", "{id}: must_refuse matches kind");
        assert_eq!(refuse, expected.is_empty(), "{id}: refuse has no SQL");
        assert_eq!(expected.len(), check.len(), "{id}: one check per answer");
        assert_eq!(
            refuse,
            strings(&case["required_columns"]).is_empty(),
            "{id}: required_columns"
        );
    }
}

#[test]
fn runner_skips_multi_and_follow_up_cases() {
    let cases: Vec<EvalCase> = serde_json::from_str(CASES).expect("cases");
    let (phrasings, skipped) = build_phrasings(&cases);
    let json = cases_json();
    let runnable = json
        .iter()
        .filter(|c| c["group"] == "single" && c.get("earlier").is_none())
        .count();
    assert_eq!(phrasings.len(), runnable * 3);
    assert_eq!(skipped.len(), cases.len() - runnable);
    assert!(phrasings
        .iter()
        .all(|p| p.case.tables.len() == 1 && !p.id.is_empty()));
}

/// Ô kết quả đưa về dạng so được: số làm tròn 4 chữ số có nghĩa, như cách chấm của #202.
fn build_cell_key(cell: &Option<CellValue>) -> String {
    match cell {
        None => "null".to_string(),
        Some(CellValue::Number(n)) => {
            let rounded: f64 = format!("{n:.3e}").parse().expect("float");
            format!("n{rounded}")
        }
        Some(CellValue::String(s)) => format!("s{s}"),
        Some(CellValue::Boolean(b)) => format!("b{b}"),
    }
}

fn build_row_keys(rows: &[Vec<Option<CellValue>>], ordered: bool) -> Vec<String> {
    let mut keys: Vec<String> = rows
        .iter()
        .map(|r| r.iter().map(build_cell_key).collect::<Vec<_>>().join("|"))
        .collect();
    if !ordered {
        keys.sort();
    }
    keys
}

fn run_sql(sql: &str, tables: &[AllowedTable]) -> Result<Vec<Vec<Option<CellValue>>>, String> {
    let guarded = build_guarded_sql(sql, tables, 100_000).map_err(|e| format!("guard: {e:?}"))?;
    let output = run_query(&guarded.runnable, 100_000, Duration::from_secs(120))
        .map_err(|e| format!("duckdb: {e:?}"))?;
    Ok(build_result_table(output, 100_000).rows)
}

/// Nạp một CSV như app nạp: Parquet trong thư mục riêng, cột lấy từ kết quả nạp (kể cả cột _source_file).
fn build_single_table(csv: &Path, name: &str, out: &Path) -> AllowedTable {
    let dir = out.join(name);
    std::fs::create_dir_all(&dir).expect("create dir");
    let file = InputFile {
        path: csv.to_path_buf(),
        name: csv.file_name().expect("name").to_string_lossy().to_string(),
        format: FileFormat::Csv,
        column_types: Vec::new(),
    };
    let materialized = materialize(&[file], &dir).expect("materialize");
    AllowedTable {
        name: name.to_string(),
        columns: materialized.columns.into_iter().map(|c| c.name).collect(),
        parquet_dir: dir,
        row_filters: Vec::new(),
    }
}

/// Bảng edu dựng từ CSV như sql/edu_load.sql: "?" thành NULL, imd_band "10-20" thêm %, disability và is_banked thành
/// boolean, studentVle gộp theo (sinh viên, ngày). Kiểu cột như bản chụp từ Postgres: integer thành INTEGER, numeric
/// trơn thành DOUBLE (sources/pg_types.rs).
const EDU_TABLES: [(&str, &str); 7] = [
    (
        "courses",
        "SELECT code_module, code_presentation, CAST(left(code_presentation, 4) AS INTEGER) AS presentation_year,
                CAST(CASE right(code_presentation, 1) WHEN 'B' THEN 2 WHEN 'J' THEN 10 END AS INTEGER)
                  AS presentation_start_month,
                CAST(module_presentation_length AS INTEGER) AS module_presentation_length
         FROM read_csv('{dir}/courses.csv', header = true, all_varchar = true)",
    ),
    (
        "students",
        "SELECT code_module, code_presentation, CAST(id_student AS INTEGER) AS id_student, gender, region,
                highest_education,
                CASE WHEN imd_band IN ('?', '') THEN NULL WHEN imd_band = '10-20' THEN '10-20%' ELSE imd_band END
                  AS imd_band,
                age_band, CAST(num_of_prev_attempts AS INTEGER) AS num_of_prev_attempts,
                CAST(studied_credits AS INTEGER) AS studied_credits, disability = 'Y' AS disability, final_result
         FROM read_csv('{dir}/studentInfo.csv', header = true, all_varchar = true)",
    ),
    (
        "registrations",
        "SELECT code_module, code_presentation, CAST(id_student AS INTEGER) AS id_student,
                TRY_CAST(date_registration AS INTEGER) AS date_registration,
                TRY_CAST(date_unregistration AS INTEGER) AS date_unregistration
         FROM read_csv('{dir}/studentRegistration.csv', header = true, all_varchar = true)",
    ),
    (
        "assessments",
        "SELECT CAST(id_assessment AS INTEGER) AS id_assessment, code_module, code_presentation, assessment_type,
                TRY_CAST(date AS INTEGER) AS date, CAST(weight AS DOUBLE) AS weight
         FROM read_csv('{dir}/assessments.csv', header = true, all_varchar = true)",
    ),
    (
        "student_assessments",
        "SELECT CAST(sa.id_assessment AS INTEGER) AS id_assessment, CAST(sa.id_student AS INTEGER) AS id_student,
                a.code_module, a.code_presentation, CAST(sa.date_submitted AS INTEGER) AS date_submitted,
                sa.is_banked = '1' AS is_banked, TRY_CAST(sa.score AS DOUBLE) AS score
         FROM read_csv('{dir}/studentAssessment.csv', header = true, all_varchar = true) sa
         JOIN read_csv('{dir}/assessments.csv', header = true, all_varchar = true) a
           ON a.id_assessment = sa.id_assessment",
    ),
    (
        "vle_materials",
        "SELECT CAST(id_site AS INTEGER) AS id_site, code_module, code_presentation, activity_type,
                TRY_CAST(week_from AS INTEGER) AS week_from, TRY_CAST(week_to AS INTEGER) AS week_to
         FROM read_csv('{dir}/vle.csv', header = true, all_varchar = true)",
    ),
    (
        "student_daily_clicks",
        "SELECT code_module, code_presentation, CAST(id_student AS INTEGER) AS id_student, CAST(date AS INTEGER) AS date,
                CAST(SUM(CAST(sum_click AS INTEGER)) AS INTEGER) AS sum_click,
                CAST(COUNT(DISTINCT id_site) AS INTEGER) AS num_sites
         FROM read_csv('{dir}/studentVle.csv', header = true, all_varchar = true)
         GROUP BY 1, 2, 3, 4",
    ),
];

fn build_edu_tables(csv_dir: &Path, out: &Path) -> Vec<AllowedTable> {
    let conn = duckdb::Connection::open_in_memory().expect("duckdb");
    let dir_text = csv_dir.to_string_lossy().replace('\'', "''");
    let mut tables = Vec::new();
    for (name, select) in EDU_TABLES {
        let dir = out.join(format!("edu_{name}"));
        std::fs::create_dir_all(&dir).expect("create dir");
        let select = select.replace("{dir}", &dir_text);
        let target = dir.join("part-0001.parquet");
        let target = target.to_string_lossy().replace('\'', "''");
        conn.execute_batch(&format!("COPY ({select}) TO '{target}' (FORMAT parquet)"))
            .unwrap_or_else(|e| panic!("build edu.{name}: {e}"));
        let mut stmt = conn
            .prepare(&format!("DESCRIBE {select}"))
            .expect("describe");
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .expect("columns")
            .collect::<Result<Vec<_>, _>>()
            .expect("column names");
        tables.push(AllowedTable {
            name: name.to_string(),
            columns,
            parquet_dir: dir,
            row_filters: Vec::new(),
        });
    }
    tables
}

/// Thư mục tạm, xóa khi test xong kể cả khi panic.
struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn gold_sql_matches_the_independent_check_on_real_oulad() {
    let Some(csv_dir) = std::env::var_os("OULAD_DIR").map(PathBuf::from) else {
        eprintln!("OULAD_DIR not set: skip the gold SQL check on OULAD");
        return;
    };
    let out = TempDir(std::env::temp_dir().join(format!("xdi-oulad-{}", uuid::Uuid::new_v4())));
    let single: Vec<AllowedTable> = SINGLE_FILES
        .iter()
        .map(|f| build_single_table(&csv_dir.join(format!("{f}.csv")), &f.to_lowercase(), &out.0))
        .collect();
    let multi = build_edu_tables(&csv_dir, &out.0);

    let mut failures = Vec::new();
    let mut checked = 0;
    for case in cases_json() {
        let id = case["id"].as_str().expect("id");
        let ordered = case["ordered"].as_bool().unwrap_or(false);
        // Nhóm single: guard chỉ thấy đúng bảng của ca, như runner hỏi trên một dataset.
        let tables: Vec<AllowedTable> = if case["group"] == "single" {
            let name = strings(&case["tables"]).remove(0);
            single
                .iter()
                .filter(|t| t.name == name)
                .map(clone_table)
                .collect()
        } else {
            multi.iter().map(clone_table).collect()
        };
        let pairs = strings(&case["expected_sql"])
            .into_iter()
            .zip(strings(&case["check_sql"]));
        for (index, (expected, check)) in pairs.enumerate() {
            checked += 1;
            let result =
                run_sql(&expected, &tables).and_then(|e| run_sql(&check, &tables).map(|c| (e, c)));
            match result {
                Ok((e, c)) if build_row_keys(&e, ordered) == build_row_keys(&c, ordered) => {
                    eprintln!("{id}[{index}] ok, {} rows", e.len());
                }
                Ok((e, c)) => failures.push(format!(
                    "{id}[{index}] mismatch: {:?} vs {:?}",
                    build_row_keys(&e, ordered).first(),
                    build_row_keys(&c, ordered).first()
                )),
                Err(why) => failures.push(format!("{id}[{index}] {why}")),
            }
        }
    }
    assert!(checked > 0);
    assert!(failures.is_empty(), "{failures:#?}");
}

fn clone_table(t: &AllowedTable) -> AllowedTable {
    AllowedTable {
        name: t.name.clone(),
        columns: t.columns.clone(),
        parquet_dir: t.parquet_dir.clone(),
        row_filters: Vec::new(),
    }
}
