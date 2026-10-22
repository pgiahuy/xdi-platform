// SPDX-License-Identifier: Apache-2.0
/* api/auth.rs - thiết lập lần đầu, đăng nhập, đăng xuất, người dùng hiện tại. Logic ở service/accounts.rs.

Mã phiên đi trong cookie httpOnly: JavaScript của trang không đọc được, nên một lỗi XSS không lấy cắp được phiên.
SameSite=Lax chặn trang khác gửi request POST kèm cookie này. Cờ Secure bật khi PUBLIC_URL bắt đầu bằng https://
(Config::secure_cookies). */

use axum::{extract::State, http::StatusCode, Json};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

use super::extract::{CurrentUser, JsonBody};
use crate::engine::store::{Membership, User};
use crate::error::ErrorBody;
use crate::service::accounts::{self, SetupInput, SESSION_DAYS};
use crate::service::users;
use crate::{error::AppError, state::AppState};

pub const SESSION_COOKIE: &str = "xdi_session";

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(setup_status, setup))
        .routes(routes!(login))
        .routes(routes!(logout))
        .routes(routes!(me))
        .routes(routes!(change_password))
        .routes(routes!(demo_login))
}

#[derive(Serialize, ToSchema)]
struct SetupStatusResponse {
    /// True while the installation has no account yet: show the first-run setup screen instead of sign-in.
    needed: bool,
}

#[derive(Deserialize, ToSchema)]
struct SetupRequest {
    /// Code printed in the backend log at startup; case and surrounding spaces are ignored.
    setup_code: String,
    email: String,
    #[serde(default)]
    display_name: String,
    /// 8 to 128 characters.
    password: String,
    /// Name of the first workspace; empty gives a default name.
    #[serde(default)]
    workspace_name: String,
}

#[derive(Deserialize, ToSchema)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Deserialize, ToSchema)]
struct ChangePasswordRequest {
    current_password: String,
    /// 8 to 128 characters.
    new_password: String,
}

#[derive(Serialize, ToSchema)]
struct MeResponse {
    user: User,
    /// Workspaces the user belongs to, with their role in each, sorted by name.
    workspaces: Vec<Membership>,
}

// Giao diện gọi khi mở trang: needed = true thì hiện màn tạo tài khoản quản trị thay cho màn đăng nhập.
#[utoipa::path(
    get,
    path = "/auth/setup",
    tag = "auth",
    operation_id = "setupStatus",
    summary = "Whether the first-run administrator setup is still needed",
    security(()),
    responses((status = 200, body = SetupStatusResponse, description = "OK"))
)]
async fn setup_status(State(state): State<AppState>) -> Json<SetupStatusResponse> {
    Json(SetupStatusResponse {
        needed: accounts::setup_needed(&state),
    })
}

#[utoipa::path(
    post,
    path = "/auth/setup",
    tag = "auth",
    operation_id = "setup",
    summary = "Create the first administrator account and workspace, then sign in",
    description = "Only works while no account exists and with the setup code from the backend log. Sets the \
                   session cookie.",
    security(()),
    request_body = SetupRequest,
    responses(
        (status = 200, body = MeResponse, description = "Signed in as the new administrator"),
        (status = "4XX", body = ErrorBody, description = "400 invalid input, 403 wrong code or already set up")
    )
)]
async fn setup(
    State(state): State<AppState>,
    jar: CookieJar,
    JsonBody(req): JsonBody<SetupRequest>,
) -> Result<(CookieJar, Json<MeResponse>), AppError> {
    let input = SetupInput {
        setup_code: req.setup_code,
        email: req.email,
        display_name: req.display_name,
        password: req.password,
        workspace_name: req.workspace_name,
    };
    let (user, session) = accounts::setup(&state, input).await?;
    let body = me_response(&state, user).await?;
    Ok((jar.add(session_cookie(&state, session)), body))
}

#[utoipa::path(
    post,
    path = "/auth/login",
    tag = "auth",
    operation_id = "login",
    summary = "Sign in with email and password",
    description = "Sets the session cookie. A wrong email and a wrong password give the same 401 message.",
    security(()),
    request_body = LoginRequest,
    responses(
        (status = 200, body = MeResponse, description = "Signed in"),
        (status = "4XX", body = ErrorBody, description = "400 invalid input, 401 wrong email or password")
    )
)]
async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    JsonBody(req): JsonBody<LoginRequest>,
) -> Result<(CookieJar, Json<MeResponse>), AppError> {
    let (user, session) = accounts::login(&state, &req.email, req.password).await?;
    let body = me_response(&state, user).await?;
    Ok((jar.add(session_cookie(&state, session)), body))
}

// Xóa phiên ở database rồi xóa cookie. Gọi khi chưa đăng nhập cũng trả thành công.
#[utoipa::path(
    post,
    path = "/auth/logout",
    tag = "auth",
    operation_id = "logout",
    summary = "Sign out: delete the session and clear the cookie",
    description = "Succeeds even when not signed in.",
    security(()),
    responses((status = 204, description = "Signed out"))
)]
async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<(CookieJar, StatusCode), AppError> {
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        accounts::logout(&state, cookie.value()).await?;
    }
    // Cookie xóa mang cùng thuộc tính với cookie lúc đặt, để trình duyệt chắc chắn thay đúng cookie đó.
    let removal = Cookie::build(SESSION_COOKIE)
        .path("/")
        .secure(state.config.secure_cookies());
    Ok((jar.remove(removal), StatusCode::NO_CONTENT))
}

#[utoipa::path(
    get,
    path = "/auth/me",
    tag = "auth",
    operation_id = "me",
    summary = "The signed-in user and their workspaces",
    responses(
        (status = 200, body = MeResponse, description = "OK"),
        (status = 401, body = ErrorBody, description = "Not signed in or session expired")
    )
)]
async fn me(State(state): State<AppState>, jar: CookieJar) -> Result<Json<MeResponse>, AppError> {
    let user = session_user(&state, &jar).await?;
    me_response(&state, user).await
}

// Đổi mật khẩu của chính mình. Phiên đang dùng giữ lại, các phiên khác bị đăng xuất.
#[utoipa::path(
    post,
    path = "/auth/password",
    tag = "auth",
    operation_id = "changePassword",
    summary = "Change your own password",
    description = "The current session stays signed in; every other session of the account is signed out.",
    request_body = ChangePasswordRequest,
    responses(
        (status = 204, description = "Password changed"),
        (status = "4XX", body = ErrorBody, description = "400 wrong current password or invalid new password, \
                                                         401 not signed in")
    )
)]
// CurrentUser đứng trước JsonBody: chưa đăng nhập thì trả 401 trước khi đọc body, như mọi endpoint cần đăng nhập khác
// (đặc tả ghi thao tác này cần cookie phiên; test every_session_operation_requires_login kiểm điều đó).
async fn change_password(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    jar: CookieJar,
    JsonBody(req): JsonBody<ChangePasswordRequest>,
) -> Result<StatusCode, AppError> {
    // CurrentUser đã kiểm có cookie phiên hợp lệ, nên ở đây luôn có giá trị.
    let session = jar
        .get(SESSION_COOKIE)
        .map(|c| c.value().to_string())
        .unwrap_or_default();
    users::change_password(
        &state,
        &user,
        &session,
        req.current_password,
        req.new_password,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/auth/demo-login",
    tag = "auth",
    operation_id = "demoLogin",
    summary = "Sign in to the public demo with a role (not implemented yet)",
    security(()),
    responses((status = 501, body = ErrorBody, description = "Not implemented yet"))
)]
async fn demo_login() -> Result<(), AppError> {
    Err(AppError::NotImplemented("đăng nhập demo theo vai"))
}

/// Người dùng của cookie phiên. Extractor CurrentUser và Member (extract.rs) gọi hàm này cho mọi API cần đăng nhập.
pub async fn session_user(state: &AppState, jar: &CookieJar) -> Result<User, AppError> {
    let session = jar.get(SESSION_COOKIE).map(|c| c.value().to_string());
    accounts::current_user(state, session.as_deref()).await
}

async fn me_response(state: &AppState, user: User) -> Result<Json<MeResponse>, AppError> {
    let workspaces = accounts::memberships(state, &user).await?;
    Ok(Json(MeResponse { user, workspaces }))
}

fn session_cookie(state: &AppState, session: String) -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE, session))
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(state.config.secure_cookies())
        .path("/")
        .max_age(time::Duration::days(SESSION_DAYS))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    use crate::api::test_support::{app, call, CODE};
    use crate::engine::store::test_support::{exec, fresh_database};

    fn setup_body(code: &str) -> Value {
        json!({
            "setup_code": code,
            "email": "admin@example.com",
            "display_name": "Admin",
            "password": "a long password",
            "workspace_name": "Cửa hàng"
        })
    }

    #[tokio::test]
    async fn setup_login_me_logout() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let app = app(db.store.clone(), Some(CODE));

        let (status, _, body) = call(&app, "GET", "/api/auth/setup", None, None).await;
        assert_eq!(
            (status, body["needed"].as_bool()),
            (StatusCode::OK, Some(true))
        );

        let (status, _, _) = call(
            &app,
            "POST",
            "/api/auth/setup",
            None,
            Some(setup_body("WRONG-CODE")),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        // Mã gõ chữ thường, thừa khoảng trắng vẫn nhận.
        let typed = format!("  {}  ", CODE.to_lowercase());
        let (status, session, body) = call(
            &app,
            "POST",
            "/api/auth/setup",
            None,
            Some(setup_body(&typed)),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let session = session.expect("setup sets a session cookie");
        assert_eq!(body["user"]["is_system_admin"], true);
        assert!(body["user"].get("password_hash").is_none());
        assert_eq!(body["workspaces"][0]["role"], "admin");
        assert_eq!(body["workspaces"][0]["slug"], "cua-hang");

        let (_, _, body) = call(&app, "GET", "/api/auth/setup", None, None).await;
        assert_eq!(body["needed"], false);
        let (status, _, _) = call(
            &app,
            "POST",
            "/api/auth/setup",
            None,
            Some(setup_body(CODE)),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        let (status, _, body) = call(&app, "GET", "/api/auth/me", Some(&session), None).await;
        assert_eq!(
            (status, body["user"]["email"].as_str()),
            (StatusCode::OK, Some("admin@example.com"))
        );

        let (status, _, _) = call(&app, "POST", "/api/auth/logout", Some(&session), None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        // Phiên đã xóa ở database: cookie cũ không dùng lại được.
        let (status, _, body) = call(&app, "GET", "/api/auth/me", Some(&session), None).await;
        assert_eq!(
            (status, body["code"].as_str()),
            (StatusCode::UNAUTHORIZED, Some("unauthorized"))
        );

        let wrong = json!({"email": "admin@example.com", "password": "not the password"});
        let (status, _, wrong_body) =
            call(&app, "POST", "/api/auth/login", None, Some(wrong)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let unknown = json!({"email": "nobody@example.com", "password": "a long password"});
        let (status, _, unknown_body) =
            call(&app, "POST", "/api/auth/login", None, Some(unknown)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        // Cùng một thông báo: không lộ email nào có tài khoản.
        assert_eq!(wrong_body["message"], unknown_body["message"]);

        let right = json!({"email": "ADMIN@example.com", "password": "a long password"});
        let (status, session, _) = call(&app, "POST", "/api/auth/login", None, Some(right)).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _, _) = call(&app, "GET", "/api/auth/me", session.as_deref(), None).await;
        assert_eq!(status, StatusCode::OK);

        db.drop().await;
    }

    /// Set-Cookie của request đăng nhập, nguyên văn.
    async fn login_set_cookie(app: &axum::Router) -> String {
        use tower::ServiceExt;
        let body = json!({"email": "admin@example.com", "password": "a long password"});
        let req = axum::http::Request::post("/api/auth/login")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body.to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        res.headers()["set-cookie"].to_str().unwrap().to_string()
    }

    /// Cookie phiên có cờ Secure khi PUBLIC_URL là https://, không có khi chạy HTTP trên máy.
    #[tokio::test]
    async fn session_cookie_is_secure_behind_https() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let local = app(db.store.clone(), Some(CODE));
        let (status, _, _) = call(
            &local,
            "POST",
            "/api/auth/setup",
            None,
            Some(setup_body(CODE)),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let cookie = login_set_cookie(&local).await;
        assert!(!cookie.contains("Secure"), "got {cookie}");

        let public = crate::api::test_support::app_with(db.store.clone(), None, |c| {
            c.public_url = "https://du-lieu.example.com".to_string();
        });
        let cookie = login_set_cookie(&public).await;
        assert!(cookie.contains("; Secure"), "got {cookie}");
        assert!(cookie.contains("HttpOnly"), "got {cookie}");

        db.drop().await;
    }

    #[tokio::test]
    async fn concurrent_setup_creates_one_admin() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let app = app(db.store.clone(), Some(CODE));
        let a = call(
            &app,
            "POST",
            "/api/auth/setup",
            None,
            Some(setup_body(CODE)),
        );
        let b = call(
            &app,
            "POST",
            "/api/auth/setup",
            None,
            Some(setup_body(CODE)),
        );
        let ((sa, _, _), (sb, _, _)) = tokio::join!(a, b);
        let mut statuses = [sa, sb];
        statuses.sort();
        assert_eq!(statuses, [StatusCode::OK, StatusCode::FORBIDDEN]);
        assert!(db.store.has_any_user().await.unwrap());

        db.drop().await;
    }

    #[tokio::test]
    async fn short_password_and_bad_email_are_rejected() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let app = app(db.store.clone(), Some(CODE));
        let mut body = setup_body(CODE);
        body["password"] = json!("short");
        let (status, _, _) = call(&app, "POST", "/api/auth/setup", None, Some(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let mut body = setup_body(CODE);
        body["email"] = json!("not-an-email");
        let (status, _, _) = call(&app, "POST", "/api/auth/setup", None, Some(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // Thất bại không tiêu mã thiết lập.
        let (_, _, body) = call(&app, "GET", "/api/auth/setup", None, None).await;
        assert_eq!(body["needed"], true);

        db.drop().await;
    }

    #[tokio::test]
    async fn expired_session_and_deactivated_account_are_rejected() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let app = app(db.store.clone(), Some(CODE));
        let (_, session, _) = call(
            &app,
            "POST",
            "/api/auth/setup",
            None,
            Some(setup_body(CODE)),
        )
        .await;

        exec(
            &db.store,
            "UPDATE sessions SET expires_at = now() - interval '1 minute'",
        )
        .await;
        let (status, _, _) = call(&app, "GET", "/api/auth/me", session.as_deref(), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "expired session");

        let login = json!({"email": "admin@example.com", "password": "a long password"});
        let (status, session, _) =
            call(&app, "POST", "/api/auth/login", None, Some(login.clone())).await;
        assert_eq!(status, StatusCode::OK);
        exec(&db.store, "UPDATE users SET is_active = false").await;
        let (status, _, _) = call(&app, "GET", "/api/auth/me", session.as_deref(), None).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "open session of a deactivated account"
        );
        let (status, _, _) = call(&app, "POST", "/api/auth/login", None, Some(login)).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "login of a deactivated account"
        );

        db.drop().await;
    }

    /// Tạo phiên kiểm lại tài khoản ngay trong câu INSERT: mật khẩu đã đổi hoặc tài khoản đã khóa thì không tạo.
    /// Đây là chốt chặn cho đăng nhập chen giữa lúc quản trị đặt lại mật khẩu hay khóa tài khoản.
    #[tokio::test]
    async fn session_is_not_created_for_stale_password_or_locked_account() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let app = app(db.store.clone(), Some(CODE));
        let (_, _, body) = call(
            &app,
            "POST",
            "/api/auth/setup",
            None,
            Some(setup_body(CODE)),
        )
        .await;
        let user_id = uuid::Uuid::parse_str(body["user"]["id"].as_str().unwrap()).unwrap();
        let current = db
            .store
            .find_credentials("admin@example.com")
            .await
            .unwrap()
            .and_then(|c| c.password_hash)
            .unwrap();
        let expires = chrono::Utc::now() + chrono::Duration::days(1);
        let token = || {
            crate::auth::token::hash_session_token(
                &crate::auth::token::new_session_token().unwrap(),
            )
        };

        assert!(db
            .store
            .create_session(user_id, &token(), expires, &current)
            .await
            .unwrap());
        assert!(!db
            .store
            .create_session(user_id, &token(), expires, "$argon2id$stale")
            .await
            .unwrap());
        exec(&db.store, "UPDATE users SET is_active = false").await;
        assert!(!db
            .store
            .create_session(user_id, &token(), expires, &current)
            .await
            .unwrap());

        db.drop().await;
    }
}
