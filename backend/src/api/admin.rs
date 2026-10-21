// SPDX-License-Identifier: Apache-2.0
/* api/admin.rs - quản lý tài khoản cho quản trị hệ thống: liệt kê, tạo, khóa, mở, đặt lại mật khẩu.
Logic và kiểm quyền ở service/users.rs; handler chỉ lấy người đang đăng nhập rồi chuyển tiếp. */

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::extract::{CurrentUser, JsonBody, PathParams};
use crate::engine::store::{User, UserSummary, WorkspaceOverview};
use crate::service::{users, workspaces};
use crate::{error::AppError, state::AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/users", get(list).post(create))
        .route("/admin/users/{id}/deactivate", post(deactivate))
        .route("/admin/users/{id}/activate", post(activate))
        .route("/admin/users/{id}/reset-password", post(reset_password))
        .route("/admin/workspaces", get(workspaces_overview))
        .route(
            "/admin/workspaces/{id}/admins",
            post(assign_workspace_admin),
        )
}

#[derive(Deserialize)]
struct AssignAdminRequest {
    email: String,
}

/// Mọi workspace và thành viên, không có dữ liệu bên trong.
async fn workspaces_overview(
    State(state): State<AppState>,
    CurrentUser(actor): CurrentUser,
) -> Result<Json<Vec<WorkspaceOverview>>, AppError> {
    Ok(Json(workspaces::overview(&state, &actor).await?))
}

/// Chỉ định admin cho workspace (ví dụ khi admin cuối không còn). Mỗi lần được ghi log.
async fn assign_workspace_admin(
    State(state): State<AppState>,
    CurrentUser(actor): CurrentUser,
    PathParams(id): PathParams<String>,
    JsonBody(req): JsonBody<AssignAdminRequest>,
) -> Result<StatusCode, AppError> {
    let workspace_id = Uuid::parse_str(&id)
        .map_err(|_| AppError::BadRequest("Mã workspace không hợp lệ".to_string()))?;
    workspaces::assign_admin(&state, &actor, workspace_id, &req.email).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct CreateUserRequest {
    email: String,
    #[serde(default)]
    display_name: String,
    password: String,
}

#[derive(Serialize)]
struct ResetPasswordResponse {
    /// Hiện cho quản trị một lần để chuyển cho người dùng; không lưu ở đâu dạng chữ.
    temporary_password: String,
}

async fn list(
    State(state): State<AppState>,
    CurrentUser(actor): CurrentUser,
) -> Result<Json<Vec<UserSummary>>, AppError> {
    Ok(Json(users::list(&state, &actor).await?))
}

async fn create(
    State(state): State<AppState>,
    CurrentUser(actor): CurrentUser,
    JsonBody(req): JsonBody<CreateUserRequest>,
) -> Result<(StatusCode, Json<User>), AppError> {
    let user = users::create(&state, &actor, &req.email, &req.display_name, req.password).await?;
    Ok((StatusCode::CREATED, Json(user)))
}

async fn deactivate(
    State(state): State<AppState>,
    CurrentUser(actor): CurrentUser,
    PathParams(id): PathParams<String>,
) -> Result<StatusCode, AppError> {
    users::deactivate(&state, &actor, parse_id(&id)?).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn activate(
    State(state): State<AppState>,
    CurrentUser(actor): CurrentUser,
    PathParams(id): PathParams<String>,
) -> Result<StatusCode, AppError> {
    users::activate(&state, &actor, parse_id(&id)?).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn reset_password(
    State(state): State<AppState>,
    CurrentUser(actor): CurrentUser,
    PathParams(id): PathParams<String>,
) -> Result<Json<ResetPasswordResponse>, AppError> {
    let temporary_password = users::reset_password(&state, &actor, parse_id(&id)?).await?;
    Ok(Json(ResetPasswordResponse { temporary_password }))
}

// Đọc id dạng chuỗi rồi tự đổi sang Uuid, để id sai báo 400 với lời rõ ràng thay vì 404 chung.
fn parse_id(raw: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(raw).map_err(|_| AppError::BadRequest("Mã tài khoản không hợp lệ".to_string()))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::{json, Value};

    use crate::api::test_support::{app, call, CODE};
    use crate::engine::store::test_support::fresh_database;

    const PASSWORD: &str = "a long password";

    async fn login(
        app: &axum::Router,
        email: &str,
        password: &str,
    ) -> (StatusCode, Option<String>) {
        let body = json!({"email": email, "password": password});
        let (status, session, _) = call(app, "POST", "/api/auth/login", None, Some(body)).await;
        (status, session)
    }

    /// Tạo quản trị hệ thống, rồi để quản trị tạo một tài khoản thường. Trả phiên quản trị, phiên người thường, id người thường.
    async fn admin_and_member(app: &axum::Router) -> (String, String, String) {
        let setup = json!({"setup_code": CODE, "email": "admin@example.com", "password": PASSWORD});
        let (_, admin, _) = call(app, "POST", "/api/auth/setup", None, Some(setup)).await;
        let admin = admin.expect("admin session");
        let new_user =
            json!({"email": "minh@example.com", "display_name": "Minh", "password": PASSWORD});
        let (status, _, body) = call(
            app,
            "POST",
            "/api/admin/users",
            Some(&admin),
            Some(new_user),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        assert_eq!(body["is_system_admin"], false);
        let id = body["id"].as_str().unwrap().to_string();
        let (status, member) = login(app, "minh@example.com", PASSWORD).await;
        assert_eq!(status, StatusCode::OK);
        (admin, member.unwrap(), id)
    }

    #[tokio::test]
    async fn only_system_admin_manages_accounts() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let app = app(db.store.clone(), Some(CODE));
        let (admin, member, member_id) = admin_and_member(&app).await;

        let (status, _, body) = call(&app, "GET", "/api/admin/users", Some(&admin), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().map(Vec::len), Some(2));
        assert!(body[0].get("password_hash").is_none());

        // Người thường gọi mọi API quản trị đều 403; chưa đăng nhập thì 401.
        let deactivate = format!("/api/admin/users/{member_id}/deactivate");
        for (method, path) in [("GET", "/api/admin/users"), ("POST", deactivate.as_str())] {
            let (status, _, _) = call(&app, method, path, Some(&member), None).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}");
            let (status, _, _) = call(&app, method, path, None, None).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path}");
        }

        for bad in ["two@@example.com", "an @example.com", "no-at-sign"] {
            let body = json!({"email": bad, "password": PASSWORD});
            let (status, _, _) =
                call(&app, "POST", "/api/admin/users", Some(&admin), Some(body)).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
        }

        let duplicate = json!({"email": "MINH@example.com", "password": PASSWORD});
        let (status, _, _) = call(
            &app,
            "POST",
            "/api/admin/users",
            Some(&admin),
            Some(duplicate),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, _, body) = call(
            &app,
            "POST",
            "/api/admin/users/not-a-uuid/activate",
            Some(&admin),
            None,
        )
        .await;
        assert_eq!(
            (status, body["code"].clone()),
            (StatusCode::BAD_REQUEST, Value::from("bad_request"))
        );

        db.drop().await;
    }

    #[tokio::test]
    async fn deactivate_and_reset_take_effect_immediately() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let app = app(db.store.clone(), Some(CODE));
        let (admin, member, member_id) = admin_and_member(&app).await;

        let path = format!("/api/admin/users/{member_id}/deactivate");
        let (status, _, _) = call(&app, "POST", &path, Some(&admin), None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        // Phiên đang mở bị cắt ngay, đăng nhập lại cũng không được.
        let (status, _, _) = call(&app, "GET", "/api/auth/me", Some(&member), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            login(&app, "minh@example.com", PASSWORD).await.0,
            StatusCode::UNAUTHORIZED
        );

        let path = format!("/api/admin/users/{member_id}/activate");
        let (status, _, _) = call(&app, "POST", &path, Some(&admin), None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (status, member) = login(&app, "minh@example.com", PASSWORD).await;
        assert_eq!(status, StatusCode::OK);

        let path = format!("/api/admin/users/{member_id}/reset-password");
        let (status, _, body) = call(&app, "POST", &path, Some(&admin), None).await;
        assert_eq!(status, StatusCode::OK);
        let temporary = body["temporary_password"].as_str().unwrap().to_string();
        let (status, _, _) = call(&app, "GET", "/api/auth/me", member.as_deref(), None).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "old session must be signed out"
        );
        assert_eq!(
            login(&app, "minh@example.com", PASSWORD).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            login(&app, "minh@example.com", &temporary).await.0,
            StatusCode::OK
        );

        db.drop().await;
    }

    #[tokio::test]
    async fn last_system_admin_cannot_be_locked_out() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let app = app(db.store.clone(), Some(CODE));
        let (admin, _, _) = admin_and_member(&app).await;
        let (_, _, me) = call(&app, "GET", "/api/auth/me", Some(&admin), None).await;
        let admin_id = me["user"]["id"].as_str().unwrap();

        let path = format!("/api/admin/users/{admin_id}/deactivate");
        let (status, _, _) = call(&app, "POST", &path, Some(&admin), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, _, _) = call(&app, "GET", "/api/auth/me", Some(&admin), None).await;
        assert_eq!(status, StatusCode::OK);

        db.drop().await;
    }

    #[tokio::test]
    async fn change_password_keeps_current_session_only() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let app = app(db.store.clone(), Some(CODE));
        let (_, first, _) = admin_and_member(&app).await;
        let (_, second) = login(&app, "minh@example.com", PASSWORD).await;

        let wrong =
            json!({"current_password": "not it at all", "new_password": "another long one"});
        let (status, _, _) = call(
            &app,
            "POST",
            "/api/auth/password",
            Some(&first),
            Some(wrong),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let short = json!({"current_password": PASSWORD, "new_password": "short"});
        let (status, _, _) = call(
            &app,
            "POST",
            "/api/auth/password",
            Some(&first),
            Some(short),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let ok = json!({"current_password": PASSWORD, "new_password": "another long one"});
        let (status, _, _) = call(&app, "POST", "/api/auth/password", Some(&first), Some(ok)).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (status, _, _) = call(&app, "GET", "/api/auth/me", Some(&first), None).await;
        assert_eq!(status, StatusCode::OK, "current session stays");
        let (status, _, _) = call(&app, "GET", "/api/auth/me", second.as_deref(), None).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "other sessions are signed out"
        );
        assert_eq!(
            login(&app, "minh@example.com", "another long one").await.0,
            StatusCode::OK
        );

        db.drop().await;
    }

    /// Tự đổi mật khẩu chỉ ghi khi mật khẩu trong database vẫn là chuỗi vừa kiểm: nếu quản trị vừa đặt lại mật khẩu
    /// thì không ghi đè mật khẩu tạm của quản trị.
    #[tokio::test]
    async fn password_write_requires_the_verified_hash() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let app = app(db.store.clone(), Some(CODE));
        let (_, _, member_id) = admin_and_member(&app).await;
        let member_id = uuid::Uuid::parse_str(&member_id).unwrap();
        let current = db
            .store
            .find_credentials("minh@example.com")
            .await
            .unwrap()
            .and_then(|c| c.password_hash)
            .unwrap();
        let new_hash = crate::auth::password::hash("chosen by member").unwrap();

        let written = db
            .store
            .set_password_and_revoke(member_id, &new_hash, Some("$argon2id$stale"), None)
            .await
            .unwrap();
        assert!(!written);
        assert_eq!(
            login(&app, "minh@example.com", PASSWORD).await.0,
            StatusCode::OK
        );

        let written = db
            .store
            .set_password_and_revoke(member_id, &new_hash, Some(&current), None)
            .await
            .unwrap();
        assert!(written);
        assert_eq!(
            login(&app, "minh@example.com", "chosen by member").await.0,
            StatusCode::OK
        );

        db.drop().await;
    }
}
