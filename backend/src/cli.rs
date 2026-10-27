// SPDX-License-Identifier: Apache-2.0
/* cli.rs - lệnh quản trị chạy trên máy chủ, không qua web: `xdi-backend <lệnh>`.

Ai chạy được binary trên máy chủ (và đọc được DATABASE_URL) là chủ bản cài, nên các lệnh này không hỏi đăng nhập.
Đây là đường cứu khi quản trị hệ thống duy nhất quên mật khẩu. Không có tham số thì main chạy server như thường. */

use crate::error::AppError;
use crate::service::{eval, users};
use crate::state::AppState;

const USAGE: &str = "usage:
  xdi-backend                                 start the server
  xdi-backend reset-password --email <email>  print a temporary password and sign the account out everywhere
  xdi-backend list-admins                     list system administrators
  xdi-backend openapi                         print the OpenAPI spec (JSON) to stdout, without a database
  xdi-backend ask-examples                    print sample ask responses (JSON) to stdout, without a database
  xdi-backend eval --data <csv> --cases <json> [--baseline <json>]
                                              run the sample question evaluation; exit 1 when accuracy drops
                                              below the baseline, 2 on infrastructure errors";

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
        _ => {
            eprintln!("{USAGE}");
            2
        }
    }
}

/// Lệnh eval (#37): chạy bộ câu đánh giá trên một file dữ liệu mẫu. In từng câu sai kèm lý do, dòng accuracy cuối,
/// ghi eval/result.json. Có --baseline thì accuracy thấp hơn baseline thì thoát mã 1; lỗi hạ tầng (thiếu file, JSON
/// hỏng, không nạp được dữ liệu) thoát mã 2.
async fn run_eval(state: &AppState, args: &[String]) -> i32 {
    let mut data = None;
    let mut cases = None;
    let mut baseline = None;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--data" => data = it.next(),
            "--cases" => cases = it.next(),
            "--baseline" => baseline = it.next(),
            _ => {
                eprintln!("{USAGE}");
                return 2;
            }
        }
    }
    let (Some(data), Some(cases)) = (data, cases) else {
        eprintln!("{USAGE}");
        return 2;
    };
    let cases = match eval::load_cases(std::path::Path::new(cases)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("eval: {e:?}");
            return 2;
        }
    };
    let report = match eval::run_eval(state, std::path::Path::new(data), &cases).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("eval: {e:?}");
            return 2;
        }
    };
    for case in &report.cases {
        if case.ok {
            println!("ok   {} {}", case.id, case.question);
        } else {
            println!("SAI  {} {}\n     {}", case.id, case.question, case.detail);
        }
    }
    println!(
        "accuracy: {}/{} ({:.1}%)",
        report.passed,
        report.total,
        report.accuracy * 100.0
    );
    if let Err(e) = std::fs::write(
        "eval/result.json",
        serde_json::to_string_pretty(&report).expect("report serializes"),
    ) {
        eprintln!("eval: cannot write eval/result.json: {e}");
        return 2;
    }
    if let Some(path) = baseline {
        let Ok(text) = std::fs::read_to_string(path) else {
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
