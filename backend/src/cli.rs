// SPDX-License-Identifier: Apache-2.0
/* cli.rs - lệnh quản trị chạy trên máy chủ, không qua web: `xdi-backend <lệnh>`.

Ai chạy được binary trên máy chủ (và đọc được DATABASE_URL) là chủ bản cài, nên các lệnh này không hỏi đăng nhập.
Đây là đường cứu khi quản trị hệ thống duy nhất quên mật khẩu. Không có tham số thì main chạy server như thường. */

use crate::error::AppError;
use crate::service::users;
use crate::state::AppState;

const USAGE: &str = "usage:
  xdi-backend                                 start the server
  xdi-backend reset-password --email <email>  print a temporary password and sign the account out everywhere
  xdi-backend list-admins                     list system administrators
  xdi-backend openapi                         print the OpenAPI spec (JSON) to stdout, without a database
  xdi-backend ask-examples                    print sample ask responses (JSON) to stdout, without a database";

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
        _ => {
            eprintln!("{USAGE}");
            2
        }
    }
}
