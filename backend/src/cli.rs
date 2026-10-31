// SPDX-License-Identifier: Apache-2.0
/* cli.rs - lệnh quản trị chạy trên máy chủ, không qua web: `xdi-backend <lệnh>`.

Ai chạy được binary trên máy chủ (và đọc được DATABASE_URL) là chủ bản cài, nên các lệnh này không hỏi đăng nhập.
Đây là đường cứu khi quản trị hệ thống duy nhất quên mật khẩu. Không có tham số thì main chạy server như thường. */

use crate::config::Config;
use crate::engine::llm;
use crate::error::AppError;
use crate::service::eval_pace::{build_paced_state, Pace};
use crate::service::eval_report::{
    build_regressions, build_subset, build_variant_cases, load_previous, KindRow,
};
use crate::service::llm_stats::{build_question_stat, build_report, TokenPrice};
use crate::service::{eval, eval_classify, eval_judge, users};
use crate::state::AppState;

const USAGE: &str = "usage:
  xdi-backend                                 start the server
  xdi-backend reset-password --email <email>  print a temporary password and sign the account out everywhere
  xdi-backend list-admins                     list system administrators
  xdi-backend openapi                         print the OpenAPI spec (JSON) to stdout, without a database
  xdi-backend ask-examples                    print sample ask responses (JSON) to stdout, without a database
  xdi-backend eval (--data <csv> | --sample-database) --cases <json> [--cases <json>...]
                  [--baseline <json>] [--out <json>] [--lessons <json>]
                  [--pause-ms <ms>] [--quota-wait-ms <ms>]
                  [--subset <n>] [--variants] [--repeat <n>] [--previous <json>]
                                              run the sample question evaluation on a file or on the
                                              snapshot of every SAMPLE_DATABASE_URL entry; write the report to --out
                                              (default eval/result.json); exit 1 when accuracy drops
                                              below the baseline, 2 on infrastructure errors; with
                                              --lessons, run once without and once with those lessons;
                                              --pause-ms spaces every model call, --quota-wait-ms (default
                                              60000) is the wait before calling again when every key is
                                              out of quota; --subset picks n cases spread over the kinds,
                                              --variants also asks every alternative wording, --repeat runs
                                              the set n times on one snapshot, --previous lists the cases
                                              that passed in that earlier report and fail now
  xdi-backend eval-classify --data <csv> --cases <json> [--pause-ms <ms>]
                                              compare the keyword rule and the model on question kinds;
                                              exit 2 on infrastructure errors
  xdi-backend eval-judge --cases <json> [--pause-ms <ms>]
                                              measure the judge model of gate 2 on labelled explanations,
                                              without a database; exit 2 on infrastructure errors
  xdi-backend llm-stats [--since <rfc3339>] [--price-in <usd> --price-out <usd>]
                                              time, model calls and tokens of recorded questions, by question
                                              kind and by model task; prices are USD per million tokens";

/// Chạy lệnh và trả mã thoát cho tiến trình. Kết quả in ra stdout, lỗi ra stderr.
pub async fn run(state: &AppState, args: &[String]) -> i32 {
    match args {
        [cmd, flag, email] if cmd == "reset-password" && flag == "--email" => {
            match users::reset_password_by_email(state, email).await {
                Ok(temporary) => {
                    println!("temporary password for {email}: {temporary}");
                    println!("all sessions of this account were signed out; change the password after signing in");
                    0
                }
                Err(AppError::NotFound(_)) => {
                    eprintln!("reset-password failed: no account with email {email}");
                    1
                }
                Err(e) => {
                    eprintln!("reset-password failed: {e:?}");
                    1
                }
            }
        }
        [cmd] if cmd == "list-admins" => match state.store.list_system_admins().await {
            Ok(admins) => {
                for a in admins {
                    let status = if a.is_active { "active" } else { "deactivated" };
                    println!("{}\t{}\t{status}", a.email, a.display_name);
                }
                0
            }
            Err(e) => {
                eprintln!("list-admins failed: {e:?}");
                1
            }
        },
        [cmd, rest @ ..] if cmd == "eval" => run_eval(state, rest).await,
        [cmd, rest @ ..] if cmd == "eval-classify" => run_eval_classify(state, rest).await,
        [cmd, rest @ ..] if cmd == "llm-stats" => run_llm_stats(state, rest).await,
        _ => {
            eprintln!("{USAGE}");
            2
        }
    }
}

/// Lệnh eval (#37): chạy bộ câu đánh giá trên một file dữ liệu mẫu (--data) hay trên mọi database mẫu của
/// SAMPLE_DATABASE_URL (--sample-database, #230). --cases lặp được, các file nối lại thành một bộ. In từng câu sai kèm
/// lý do, dòng accuracy cuối, ghi báo cáo ra --out. Có --baseline thì accuracy thấp hơn baseline thì thoát mã 1; lỗi hạ
/// tầng (thiếu file, JSON hỏng, không nạp được dữ liệu) thoát mã 2. Có --lessons thì chạy hai lượt, không và có bài học
/// (#228), in thêm tỷ lệ đúng của lượt không bài học và các câu đổi kết quả; baseline so với lượt có bài học.
/// --pause-ms, --quota-wait-ms (#202): mọi lần gọi mô hình của lượt chạy đi qua service/eval_pace.rs; in số lần gọi dự
/// kiến trước khi chạy, số lần gọi thật và số lần chờ hết hạn mức sau khi chạy. --subset, --variants, --repeat,
/// --previous (#202): chọn câu, hỏi cả cách hỏi khác, chạy lặp, so với báo cáo cũ (service/eval_report.rs); in thêm
/// bảng theo loại câu kèm khoảng tin cậy.
async fn run_eval(state: &AppState, args: &[String]) -> i32 {
    let mut data = None;
    let mut sample_database = false;
    let mut case_files = Vec::new();
    let mut baseline = None;
    let mut out = "eval/result.json".to_string();
    let mut lessons_path = None;
    let mut pause_ms = "0".to_string();
    let mut quota_wait_ms = "60000".to_string();
    let mut subset = None;
    let mut variants = false;
    let mut repeat = "1".to_string();
    let mut previous = None;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        if flag == "--sample-database" {
            sample_database = true;
            continue;
        }
        if flag == "--variants" {
            variants = true;
            continue;
        }
        // Các cờ còn lại đều có một giá trị đi sau.
        let Some(value) = it.next() else {
            eprintln!("{USAGE}");
            return 2;
        };
        match flag.as_str() {
            "--data" => data = Some(value.clone()),
            "--cases" => case_files.push(value.clone()),
            "--baseline" => baseline = Some(value.clone()),
            "--out" => out = value.clone(),
            "--lessons" => lessons_path = Some(value.clone()),
            "--pause-ms" => pause_ms = value.clone(),
            "--quota-wait-ms" => quota_wait_ms = value.clone(),
            "--subset" => subset = Some(value.clone()),
            "--repeat" => repeat = value.clone(),
            "--previous" => previous = Some(value.clone()),
            _ => {
                eprintln!("{USAGE}");
                return 2;
            }
        }
    }
    let (Ok(pause_ms), Ok(quota_wait_ms)) = (pause_ms.parse::<u64>(), quota_wait_ms.parse::<u64>())
    else {
        eprintln!("{USAGE}");
        return 2;
    };
    let (Ok(repeat), Ok(subset)) = (
        repeat.parse::<u32>(),
        subset.map(|s| s.parse::<usize>()).transpose(),
    ) else {
        eprintln!("{USAGE}");
        return 2;
    };
    // Lượt có bài học so với lượt không bài học theo từng câu; gộp thêm nhiều lượt thì không còn biết so lượt nào.
    if repeat == 0 || (repeat > 1 && lessons_path.is_some()) {
        eprintln!("eval: --repeat must be at least 1 and cannot be used with --lessons");
        return 2;
    }
    let pace = Pace {
        pause: std::time::Duration::from_millis(pause_ms),
        quota_wait: std::time::Duration::from_millis(quota_wait_ms),
        quota_retries: QUOTA_RETRIES,
    };
    let databases = &state.config.sample_databases;
    let source = match (&data, sample_database) {
        (Some(path), false) => eval::EvalData::File(std::path::Path::new(path)),
        (None, true) if !databases.is_empty() => eval::EvalData::SampleDatabases(databases),
        (None, true) => {
            eprintln!("eval: --sample-database needs SAMPLE_DATABASE_URL");
            return 2;
        }
        _ => {
            eprintln!("{USAGE}");
            return 2;
        }
    };
    if case_files.is_empty() {
        eprintln!("{USAGE}");
        return 2;
    }
    let mut cases = Vec::new();
    for file in &case_files {
        match eval::load_cases(std::path::Path::new(file)) {
            Ok(c) => cases.extend(c),
            Err(e) => {
                eprintln!("eval: {e:?}");
                return 2;
            }
        }
    }
    if let Some(size) = subset {
        cases = build_subset(cases, size);
    }
    if variants {
        cases = build_variant_cases(cases);
    }
    let previous = match previous.map(|p| load_previous(std::path::Path::new(&p))) {
        None => None,
        Some(Ok(previous)) => Some(previous),
        Some(Err(e)) => {
            eprintln!("eval: {e:?}");
            return 2;
        }
    };
    let lessons = match lessons_path.map(|p| eval::load_lessons(std::path::Path::new(&p))) {
        None => Vec::new(),
        Some(Ok(lessons)) => lessons,
        Some(Err(e)) => {
            eprintln!("eval: {e:?}");
            return 2;
        }
    };
    print_call_estimate(cases.len() * repeat as usize, pace);
    let (paced, stats) = build_paced_state(state, pace);
    let mut report = match eval::run_eval(&paced, source, &cases, &lessons, repeat).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("eval: {e:?}");
            return 2;
        }
    };
    if let Some(previous) = &previous {
        report.regressions = build_regressions(previous, &report.cases);
    }
    for case in &report.cases {
        if repeat > 1 {
            print!("[{}] ", case.run);
        }
        match case.verdict {
            eval::Verdict::Pass => println!("ok   {} {}", case.id, case.question),
            eval::Verdict::Fail => {
                println!("SAI  {} {}\n     {}", case.id, case.question, case.detail)
            }
            eval::Verdict::Review => {
                println!("XEM  {} {}\n     {}", case.id, case.question, case.detail)
            }
        }
    }
    println!(
        "accuracy: {}/{} ({:.1}%), needs review: {}",
        report.passed,
        report.passed + report.failed,
        report.accuracy * 100.0,
        report.review
    );
    print_kind_rows(&report.by_kind);
    if !report.runs.is_empty() {
        let runs: Vec<String> = report
            .runs
            .iter()
            .map(|r| format!("{:.1}%", r.accuracy * 100.0))
            .collect();
        println!(
            "runs: {}; cases that changed between runs: {}",
            runs.join(", "),
            build_id_list(&report.unstable)
        );
    }
    if previous.is_some() {
        println!(
            "regressions since the previous report: {}",
            build_id_list(&report.regressions)
        );
    }
    if let Some(comparison) = &report.lessons {
        println!(
            "lessons: {}; without lessons {}/{} ({:.1}%); cases that changed: {}",
            comparison.lessons,
            comparison.passed_without,
            report.total,
            comparison.accuracy_without * 100.0,
            if comparison.changed.is_empty() {
                "none".to_string()
            } else {
                comparison.changed.join(", ")
            }
        );
    }
    println!(
        "model calls: {} (waited {} times for quota)",
        stats.calls(),
        stats.quota_waits()
    );
    if let Err(e) = std::fs::write(
        &out,
        serde_json::to_string_pretty(&report).expect("report serializes"),
    ) {
        eprintln!("eval: cannot write {out}: {e}");
        return 2;
    }
    if let Some(path) = baseline {
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!("eval: cannot read baseline {path}");
            return 2;
        };
        let Ok(base) = serde_json::from_str::<serde_json::Value>(&text) else {
            eprintln!("eval: cannot parse baseline {path}");
            return 2;
        };
        let Some(floor) = base["accuracy"].as_f64() else {
            eprintln!("eval: baseline {path} has no numeric accuracy");
            return 2;
        };
        if report.accuracy < floor {
            eprintln!(
                "eval: accuracy {:.1}% below baseline {:.1}%",
                report.accuracy * 100.0,
                floor * 100.0
            );
            return 1;
        }
    }
    0
}

/// Bảng theo loại câu: đạt trên số câu chấm được, tỷ lệ, khoảng tin cậy Wilson 95%, số câu cần xem lại.
fn print_kind_rows(rows: &[KindRow]) {
    println!("by kind (accuracy, 95% Wilson interval):");
    for row in rows {
        println!(
            "  {:<22} {:>3}/{:<3} {:>5.1}%  [{:.1}%, {:.1}%]  needs review: {}",
            row.kind,
            row.passed,
            row.passed + row.failed,
            row.accuracy * 100.0,
            row.low * 100.0,
            row.high * 100.0,
            row.review
        );
    }
}

fn build_id_list(ids: &[String]) -> String {
    if ids.is_empty() {
        "none".to_string()
    } else {
        ids.join(", ")
    }
}

/// Số lần gọi lại tối đa khi mọi khóa hết hạn mức: hạn mức theo phút hồi sau một lần chờ; hết hạn mức theo ngày thì
/// gọi lại mãi cũng không được, nên dừng sau vài lần.
const QUOTA_RETRIES: u32 = 3;

/// Số lần gọi của câu hỏi đo ở docs/PERFORMANCE.md: câu thường khoảng 3, câu "vì sao" khoảng 4. Lập hồ sơ ngữ nghĩa
/// thêm 3 lần mỗi bảng, chưa tính ở đây vì số bảng chỉ biết sau khi nạp hay chụp.
const CALLS_PER_QUESTION: (usize, usize) = (2, 4);

/// In trước số lần gọi dự kiến và thời gian tối thiểu, để biết hạn mức theo ngày có đủ không trước khi chạy.
fn print_call_estimate(cases: usize, pace: Pace) {
    let (low, high) = (cases * CALLS_PER_QUESTION.0, cases * CALLS_PER_QUESTION.1);
    let minutes = |calls: usize| pace.pause.as_secs_f64() * calls as f64 / 60.0;
    println!(
        "estimate: {low} to {high} model calls for {cases} questions, plus 3 per table while profiling; \
         at least {:.0} to {:.0} minutes with --pause-ms {}",
        minutes(low),
        minutes(high),
        pace.pause.as_millis()
    );
}

/// Lệnh llm-stats (#184): đọc lịch sử câu hỏi từ --since (mặc định mọi câu) rồi in báo cáo của
/// service/llm_stats.rs. Giá token đưa qua tham số, vì giá đổi theo mô hình và theo thời gian.
async fn run_llm_stats(state: &AppState, args: &[String]) -> i32 {
    let mut since = chrono::DateTime::<chrono::Utc>::UNIX_EPOCH;
    let (mut price_in, mut price_out) = (None, None);
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next();
        let parsed = match (flag.as_str(), value) {
            ("--since", Some(v)) => chrono::DateTime::parse_from_rfc3339(v)
                .map(|t| since = t.with_timezone(&chrono::Utc))
                .is_ok(),
            ("--price-in", Some(v)) => v.parse::<f64>().map(|p| price_in = Some(p)).is_ok(),
            ("--price-out", Some(v)) => v.parse::<f64>().map(|p| price_out = Some(p)).is_ok(),
            _ => false,
        };
        if !parsed {
            eprintln!("{USAGE}");
            return 2;
        }
    }
    let price = match (price_in, price_out) {
        (Some(input), Some(output)) => Some(TokenPrice { input, output }),
        (None, None) => None,
        _ => {
            eprintln!("llm-stats: give both --price-in and --price-out");
            return 2;
        }
    };
    let rows = match state.store.list_query_timings(since).await {
        Ok(rows) => rows,
        Err(e) => {
            eprintln!("llm-stats: {e:?}");
            return 2;
        }
    };
    let stats: Vec<_> = rows.iter().filter_map(build_question_stat).collect();
    print!("{}", build_report(&stats, price));
    0
}

/// Lệnh eval-classify (#197): bộ câu thử phân loại câu hỏi. In từng câu kèm loại đúng, loại của luật từ khóa, loại
/// của bước classify và thời gian, rồi tỷ lệ đúng của hai cách và trung vị thời gian. Chỉ báo số, không so với
/// baseline: với mock, phần lớn câu rơi về luật từ khóa nên hai tỷ lệ gần bằng nhau.
async fn run_eval_classify(state: &AppState, args: &[String]) -> i32 {
    let mut data = None;
    let mut cases = None;
    let mut pause_ms = Some("0");
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--data" => data = it.next(),
            "--cases" => cases = it.next(),
            "--pause-ms" => pause_ms = it.next().map(String::as_str),
            _ => {
                eprintln!("{USAGE}");
                return 2;
            }
        }
    }
    let (Some(data), Some(cases), Some(Ok(pause_ms))) = (data, cases, pause_ms.map(str::parse))
    else {
        eprintln!("{USAGE}");
        return 2;
    };
    let cases = match eval_classify::load_cases(std::path::Path::new(cases)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("eval-classify: {e:?}");
            return 2;
        }
    };
    let pause = std::time::Duration::from_millis(pause_ms);
    let path = std::path::Path::new(data);
    let report = match eval_classify::run_eval_classify(state, path, &cases, pause).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("eval-classify: {e:?}");
            return 2;
        }
    };
    // Mỗi câu một dòng: id, loại đúng, loại của luật từ khóa và của bước classify (SAI khi lệch), thời gian, câu hỏi;
    // dòng sau là chi tiết của bước classify.
    for case in &report.cases {
        let verdict = |kind: eval_classify::CaseKind| {
            let mark = if kind == case.expected { "" } else { " SAI" };
            format!("{}{mark}", kind.as_str())
        };
        println!(
            "{} expected={} keyword={} model={} {} ms  {}\n     {}",
            case.id,
            case.expected.as_str(),
            verdict(case.keyword),
            verdict(case.model),
            case.duration_ms,
            case.question,
            case.detail
        );
    }
    let percent = |n: usize| n as f64 * 100.0 / report.total.max(1) as f64;
    println!(
        "keyword rule: {}/{} ({:.1}%)",
        report.keyword_correct,
        report.total,
        percent(report.keyword_correct)
    );
    println!(
        "classify step: {}/{} ({:.1}%), fell back to keywords on {} questions",
        report.model_correct,
        report.total,
        percent(report.model_correct),
        report.fallbacks
    );
    println!("classify step median: {} ms", report.median_ms);
    0
}

/// Lệnh eval-judge (#201): bộ đo mô hình chấm của cổng 2. In từng ca kèm kết luận đúng, kết luận của mô hình (SAI khi
/// lệch) và chỗ mô hình nêu, rồi tỷ lệ đúng, số bỏ lọt và số chỉ cảnh báo trên số ca sai, số chặn nhầm và số cảnh báo
/// thừa trên số ca đúng, số ca không chấm được và trung vị thời gian. Chỉ báo số, không so với baseline. main.rs gọi thẳng lệnh này trước khi kết nối database.
pub async fn run_eval_judge(config: &Config, args: &[String]) -> i32 {
    let mut cases = None;
    let mut pause_ms = Some("0");
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--cases" => cases = it.next(),
            "--pause-ms" => pause_ms = it.next().map(String::as_str),
            _ => {
                eprintln!("{USAGE}");
                return 2;
            }
        }
    }
    let (Some(cases), Some(Ok(pause_ms))) = (cases, pause_ms.map(str::parse)) else {
        eprintln!("{USAGE}");
        return 2;
    };
    let cases = match eval_judge::load_judge_cases(std::path::Path::new(cases)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("eval-judge: {e:?}");
            return 2;
        }
    };
    let pause = std::time::Duration::from_millis(pause_ms);
    let provider = llm::from_config(config);
    let report = eval_judge::run_eval_judge(provider.as_ref(), config, &cases, pause).await;
    // Mỗi ca một dòng: id, kết luận đúng, loại lỗi cài vào, kết luận của mô hình (SAI khi lệch), thời gian; dòng sau là
    // lỗi mô hình nêu hay lý do không chấm được.
    for case in &report.cases {
        let got = case.got.as_deref().unwrap_or("none");
        let mark = if case.got.as_deref() == Some(case.expected.as_str()) {
            ""
        } else {
            " SAI"
        };
        println!(
            "{} expected={} error={} judge={got}{mark} {} ms\n     {}",
            case.id, case.expected, case.error, case.duration_ms, case.detail
        );
    }
    let percent = |n: usize, of: usize| n as f64 * 100.0 / of.max(1) as f64;
    println!(
        "correct: {}/{} ({:.1}%)",
        report.correct,
        report.total,
        percent(report.correct, report.total)
    );
    println!(
        "missed (wrong text judged match): {}/{} ({:.1}%)",
        report.missed,
        report.mismatch_total,
        percent(report.missed, report.mismatch_total)
    );
    println!(
        "warned only (wrong text judged warning): {}/{} ({:.1}%)",
        report.warned_wrong,
        report.mismatch_total,
        percent(report.warned_wrong, report.mismatch_total)
    );
    println!(
        "false alarms (right text judged mismatch): {}/{} ({:.1}%)",
        report.false_alarms,
        report.match_total,
        percent(report.false_alarms, report.match_total)
    );
    println!(
        "extra warnings (right text judged warning): {}/{} ({:.1}%)",
        report.warned_right,
        report.match_total,
        percent(report.warned_right, report.match_total)
    );
    println!("not judged: {}", report.unavailable);
    println!("judge median: {} ms", report.median_ms);
    0
}
