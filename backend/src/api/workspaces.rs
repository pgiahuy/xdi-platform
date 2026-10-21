// SPDX-License-Identifier: Apache-2.0
/* api/workspaces.rs - workspace của người đang đăng nhập: liệt kê, tạo, đổi tên, xóa; thành viên: xem, thêm, đổi vai,
xóa hoặc tự rời. Logic và kiểm vai ở service/workspaces.rs.
API dữ liệu nằm dưới /api/workspaces/{ws}/ (datasets.rs, query.rs) và đi qua kiểm thành viên ở extract.rs. */

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, patch},
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use super::extract::{CurrentUser, JsonBody, PathParams};
use crate::auth::Member;
use crate::engine::store::{MemberRow, Membership};
use crate::service::workspaces;
use crate::{error::AppError, state::AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/workspaces", get(list).post(create))
        .route("/workspaces/{ws}", patch(rename).delete(delete))
        .route("/workspaces/{ws}/members", get(members).post(add_member))
        .route(
            "/workspaces/{ws}/members/{user_id}",
            patch(set_role).delete(remove_member),
        )
}

#[derive(Deserialize)]
struct CreateWorkspaceRequest {
    name: String,
}

#[derive(Deserialize)]
struct AddMemberRequest {
    email: String,
    role: String,
}

#[derive(Deserialize)]
struct SetRoleRequest {
    role: String,
}

async fn list(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<Vec<Membership>>, AppError> {
    Ok(Json(workspaces::list(&state, &user).await?))
}

async fn create(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    JsonBody(req): JsonBody<CreateWorkspaceRequest>,
) -> Result<(StatusCode, Json<Membership>), AppError> {
    let membership = workspaces::create(&state, &user, &req.name).await?;
    Ok((StatusCode::CREATED, Json(membership)))
}

async fn rename(
    State(state): State<AppState>,
    member: Member,
    JsonBody(req): JsonBody<CreateWorkspaceRequest>,
) -> Result<StatusCode, AppError> {
    workspaces::rename(&state, &member, &req.name).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete(State(state): State<AppState>, member: Member) -> Result<StatusCode, AppError> {
    workspaces::delete(&state, &member).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn members(
    State(state): State<AppState>,
    member: Member,
) -> Result<Json<Vec<MemberRow>>, AppError> {
    Ok(Json(workspaces::members(&state, &member).await?))
}

async fn add_member(
    State(state): State<AppState>,
    member: Member,
    JsonBody(req): JsonBody<AddMemberRequest>,
) -> Result<StatusCode, AppError> {
    workspaces::add_member(&state, &member, &req.email, &req.role).await?;
    Ok(StatusCode::CREATED)
}

async fn set_role(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, user_id)): PathParams<(String, Uuid)>,
    JsonBody(req): JsonBody<SetRoleRequest>,
) -> Result<StatusCode, AppError> {
    workspaces::set_role(&state, &member, user_id, &req.role).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_member(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, user_id)): PathParams<(String, Uuid)>,
) -> Result<StatusCode, AppError> {
    workspaces::remove_member(&state, &member, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::{json, Value};

    use crate::api::test_support::{app, call, CODE};
    use crate::auth::WorkspaceId;
    use crate::engine::store::{test_support::fresh_database, Store};

    const PASSWORD: &str = "a long password";

    async fn setup_admin(app: &axum::Router) -> String {
        let body = json!({"setup_code": CODE, "email": "lan@example.com", "password": PASSWORD,
                          "workspace_name": "Cửa hàng online"});
        let (status, session, _) = call(app, "POST", "/api/auth/setup", None, Some(body)).await;
        assert_eq!(status, StatusCode::OK);
        session.unwrap()
    }

    /// Thêm một dataset thẳng vào database: nạp file qua API là việc của issue nạp dữ liệu.
    async fn add_dataset(store: &Store, workspace: &Value, name: &str) -> String {
        let id = uuid::Uuid::parse_str(workspace["workspace_id"].as_str().unwrap()).unwrap();
        let ws = WorkspaceId::for_test(id);
        let source = store.create_local_source(&ws).await.unwrap();
        let dataset = store
            .create_dataset(&ws, source, name, name, None)
            .await
            .unwrap();
        dataset.id.to_string()
    }

    #[tokio::test]
    async fn each_workspace_sees_only_its_own_datasets() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let app = app(db.store.clone(), Some(CODE));
        let lan = setup_admin(&app).await;

        let (status, _, home) = call(
            &app,
            "POST",
            "/api/workspaces",
            Some(&lan),
            Some(json!({"name": "Chi tiêu gia đình"})),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            (home["slug"].as_str(), home["role"].as_str()),
            (Some("chi-tieu-gia-dinh"), Some("admin"))
        );

        let (_, _, list) = call(&app, "GET", "/api/workspaces", Some(&lan), None).await;
        let slugs: Vec<&str> = list
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["slug"].as_str().unwrap())
            .collect();
        assert_eq!(slugs, ["chi-tieu-gia-dinh", "cua-hang-online"]);
        let shop = list
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["slug"] == "cua-hang-online")
            .unwrap()
            .clone();

        let shop_dataset = add_dataset(&db.store, &shop, "orders").await;
        add_dataset(&db.store, &home, "expenses").await;

        let (status, _, body) = call(
            &app,
            "GET",
            "/api/workspaces/cua-hang-online/datasets",
            Some(&lan),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let names: Vec<&str> = body
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["orders"]);
        let (_, _, body) = call(
            &app,
            "GET",
            "/api/workspaces/chi-tieu-gia-dinh/datasets",
            Some(&lan),
            None,
        )
        .await;
        let names: Vec<&str> = body
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["expenses"]);

        // Dataset của workspace kia, gọi qua đường dẫn workspace này: như không tồn tại.
        let path = format!("/api/workspaces/chi-tieu-gia-dinh/datasets/{shop_dataset}");
        let (status, _, _) = call(&app, "GET", &path, Some(&lan), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let path = format!("/api/workspaces/cua-hang-online/datasets/{shop_dataset}");
        let (status, _, body) = call(&app, "GET", &path, Some(&lan), None).await;
        assert_eq!(
            (status, body["name"].as_str()),
            (StatusCode::OK, Some("orders"))
        );

        db.drop().await;
    }

    #[tokio::test]
    async fn non_members_cannot_enter_a_workspace() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let app = app(db.store.clone(), Some(CODE));
        let lan = setup_admin(&app).await;
        let new_user = json!({"email": "minh@example.com", "password": PASSWORD});
        call(&app, "POST", "/api/admin/users", Some(&lan), Some(new_user)).await;
        let login = json!({"email": "minh@example.com", "password": PASSWORD});
        let (_, minh, _) = call(&app, "POST", "/api/auth/login", None, Some(login)).await;
        let minh = minh.unwrap();

        // Không thuộc workspace và workspace không tồn tại: cùng một lỗi, không dò được tên nào có thật.
        let (status, _, not_member) = call(
            &app,
            "GET",
            "/api/workspaces/cua-hang-online/datasets",
            Some(&minh),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (_, _, missing) = call(
            &app,
            "GET",
            "/api/workspaces/khong-co/datasets",
            Some(&minh),
            None,
        )
        .await;
        assert_eq!(not_member, missing);
        let ask = json!({"question": "doanh thu"});
        let path = "/api/workspaces/cua-hang-online/datasets/x/ask";
        let (status, _, _) = call(&app, "POST", path, Some(&minh), Some(ask)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        // Minh tự tạo workspace của mình thì vào được, Lan thì không.
        let (status, _, _) = call(
            &app,
            "POST",
            "/api/workspaces",
            Some(&minh),
            Some(json!({"name": "Của Minh"})),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let (status, _, _) = call(
            &app,
            "GET",
            "/api/workspaces/cua-minh/datasets",
            Some(&minh),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _, _) = call(
            &app,
            "GET",
            "/api/workspaces/cua-minh/datasets",
            Some(&lan),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        db.drop().await;
    }

    #[tokio::test]
    async fn workspace_names_are_validated_and_slugs_stay_unique() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let app = app(db.store.clone(), Some(CODE));
        let lan = setup_admin(&app).await;
        let (status, _, _) = call(
            &app,
            "POST",
            "/api/workspaces",
            Some(&lan),
            Some(json!({"name": "   "})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let long = "a".repeat(101);
        let (status, _, _) = call(
            &app,
            "POST",
            "/api/workspaces",
            Some(&lan),
            Some(json!({"name": long})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        // Cùng tên, tạo cùng lúc: hai workspace, hai slug khác nhau.
        let a = call(
            &app,
            "POST",
            "/api/workspaces",
            Some(&lan),
            Some(json!({"name": "Kinh doanh"})),
        );
        let b = call(
            &app,
            "POST",
            "/api/workspaces",
            Some(&lan),
            Some(json!({"name": "Kinh doanh"})),
        );
        let ((sa, _, ba), (sb, _, bb)) = tokio::join!(a, b);
        assert_eq!((sa, sb), (StatusCode::CREATED, StatusCode::CREATED));
        let mut slugs = [
            ba["slug"].as_str().unwrap().to_string(),
            bb["slug"].as_str().unwrap().to_string(),
        ];
        slugs.sort();
        assert_eq!(slugs, ["kinh-doanh", "kinh-doanh-2"]);

        db.drop().await;
    }

    /// Lỗi do request sai (đường dẫn không phải UTF-8, body hỏng, thiếu Content-Type) vẫn trả JSON {code, message}.
    #[tokio::test]
    async fn malformed_requests_get_json_errors() {
        use axum::{body::Body, http::Request};
        use http_body_util::BodyExt;
        use tower::ServiceExt;

        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let app = app(db.store.clone(), Some(CODE));
        let lan = setup_admin(&app).await;
        let cookie = format!("{}={lan}", crate::api::auth::SESSION_COOKIE);

        let cases = [
            (
                "GET",
                "/api/workspaces/%FF/datasets",
                None,
                None,
                StatusCode::NOT_FOUND,
            ),
            (
                "GET",
                "/api/workspaces/cua-hang-online/datasets/%FF",
                None,
                None,
                StatusCode::NOT_FOUND,
            ),
            (
                "POST",
                "/api/workspaces",
                Some("application/json"),
                Some("{not json"),
                StatusCode::BAD_REQUEST,
            ),
            (
                "POST",
                "/api/workspaces",
                Some("application/json"),
                Some("{}"),
                StatusCode::BAD_REQUEST,
            ),
            (
                "POST",
                "/api/workspaces",
                None,
                Some(r#"{"name":"x"}"#),
                StatusCode::BAD_REQUEST,
            ),
        ];
        for (method, path, content_type, body, expected) in cases {
            let mut req = Request::builder()
                .method(method)
                .uri(path)
                .header("cookie", &cookie);
            if let Some(ct) = content_type {
                req = req.header("content-type", ct);
            }
            let req = req
                .body(
                    body.map(|b| Body::from(b.to_string()))
                        .unwrap_or_else(Body::empty),
                )
                .unwrap();
            let res = app.clone().oneshot(req).await.unwrap();
            assert_eq!(res.status(), expected, "{method} {path} {body:?}");
            let ct = res
                .headers()
                .get("content-type")
                .map(|v| v.to_str().unwrap().to_string());
            assert!(
                ct.as_deref()
                    .is_some_and(|c| c.starts_with("application/json")),
                "{path}: {ct:?}"
            );
            let bytes = res.into_body().collect().await.unwrap().to_bytes();
            let json: Value = serde_json::from_slice(&bytes).unwrap();
            assert!(
                json["code"].is_string() && json["message"].is_string(),
                "{path}: {json}"
            );
        }

        db.drop().await;
    }
}
