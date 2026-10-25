// SPDX-License-Identifier: Apache-2.0
/* api/datasets.rs - liệt kê, xem, nạp dataset trong một workspace. Logic nạp ở service/ingest.rs.
Mọi route nằm dưới /workspaces/{ws}/ và nhận Member (extract.rs): chưa đăng nhập 401, không thuộc workspace 404.

Nạp file: multipart/form-data, các trường
- file: một hoặc nhiều file CSV hoặc Parquet (lặp lại trường file), hoặc một file Excel .xlsx,
- name: tên dataset mới (không bắt buộc, mặc định lấy tên file đầu),
- dataset_id: có thì thêm phiên bản cho dataset này thay vì tạo dataset mới,
- sheet: sheet cần nạp của file Excel (lặp lại để chọn nhiều sheet cùng cột; file chỉ một sheet thì bỏ trống được).
Liệt kê sheet: gửi một file Excel (trường file) tới /datasets/sheets, nhận tên sheet, cột, số dòng; file không lưu lại.
File ghi thẳng xuống đĩa từng đoạn trong lúc nhận, không giữ cả file trong bộ nhớ; SHA-256 tính cùng lúc. */

use axum::{
    extract::{multipart::MultipartError, DefaultBodyLimit, Multipart, State},
    http::StatusCode,
    Json,
};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

use super::extract::PathParams;
use crate::auth::Member;
use crate::engine::store::Dataset;
use crate::error::ErrorBody;
use crate::service::excel::{self, SheetSummary};
use crate::service::ingest::{self, StagedFile, Staging, Upload, UploadResult};
use crate::{error::AppError, state::AppState};

/// Trần số trường multipart trong một request: chặn kiểu gửi hàng vạn trường rỗng để giữ kết nối.
const MAX_FIELDS: usize = 1000;
/// Tên file dài hơn thì từ chối: tên đi vào cột _source_file của mọi dòng.
const MAX_FILE_NAME: usize = 255;

/// max_upload_bytes: MAX_UPLOAD_MB. Giới hạn body cả request đặt rộng hơn một chút để chứa phần đầu và ranh giới của
/// multipart; nội dung các trường thì handler đếm chính xác và trả 413 JSON. Giới hạn đặt cho cả nhóm route này (layer
/// của OpenApiRouter áp cho mọi route đã thêm); hai route GET không có body nên không bị ảnh hưởng.
pub fn routes(max_upload_bytes: u64) -> OpenApiRouter<AppState> {
    let body_limit = usize::try_from(max_upload_bytes)
        .unwrap_or(usize::MAX)
        .saturating_add(1024 * 1024);
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(sheets))
        .routes(routes!(detail))
        .layer(DefaultBodyLimit::max(body_limit))
}

// Hai struct dưới chỉ để mô tả form multipart trong đặc tả OpenAPI; handler đọc từng trường bằng Multipart (receive),
// không dựng struct này, nên tắt cảnh báo trường không được đọc.
/// Multipart form for uploading a dataset. Total size is limited by MAX_UPLOAD_MB (413 above it).
#[derive(ToSchema)]
#[allow(dead_code)]
struct UploadDatasetRequest {
    /// One or more CSV or Parquet files (repeat the field), or one Excel `.xlsx` file. Files with the same columns are
    /// stacked into one table.
    #[schema(value_type = Vec<String>, format = Binary)]
    file: Vec<Vec<u8>>,
    /// Name of the new dataset; defaults to the first file name.
    name: Option<String>,
    /// Add a new version to this dataset instead of creating a new one.
    dataset_id: Option<String>,
    /// Sheet of the Excel file to upload; repeat to stack several sheets with the same columns. Not needed when the
    /// file has one sheet.
    sheet: Option<Vec<String>>,
}

/// Multipart form for listing the sheets of an Excel file. The file is not stored.
#[derive(ToSchema)]
#[allow(dead_code)]
struct ListSheetsRequest {
    /// One Excel `.xlsx` file.
    #[schema(value_type = String, format = Binary)]
    file: Vec<u8>,
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/datasets",
    tag = "datasets",
    operation_id = "listDatasets",
    summary = "Datasets of the workspace",
    params(("ws" = String, Path, description = "Workspace slug")),
    responses(
        (status = 200, body = Vec<Dataset>, description = "OK"),
        (status = "4XX", body = ErrorBody, description = "401, 404 not a member")
    )
)]
async fn list(
    State(state): State<AppState>,
    member: Member,
) -> Result<Json<Vec<Dataset>>, AppError> {
    Ok(Json(ingest::list_datasets(&state, &member).await?))
}

#[utoipa::path(
    post,
    path = "/workspaces/{ws}/datasets",
    tag = "datasets",
    operation_id = "uploadDataset",
    summary = "Upload files as a new dataset or a new version of one (admin only)",
    description = "Each upload is a version. Uploading exactly the files of an existing version creates nothing and \
                   returns 200 with `created` false.",
    params(("ws" = String, Path, description = "Workspace slug")),
    request_body(content = UploadDatasetRequest, content_type = "multipart/form-data"),
    responses(
        (status = 201, body = UploadResult, description = "New dataset or version created"),
        (status = 200, body = UploadResult, description = "Same files as an existing version: nothing created"),
        (status = "4XX", body = ErrorBody, description = "400 unreadable or mismatched files, 401, 403 not admin, \
                                                         404 not a member or unknown dataset_id, 413 too large")
    )
)]
async fn create(
    State(state): State<AppState>,
    member: Member,
    multipart: Result<Multipart, axum::extract::multipart::MultipartRejection>,
) -> Result<(StatusCode, Json<UploadResult>), AppError> {
    // Kiểm vai trước khi nhận file: member không phải chờ gửi xong cả tệp mới biết mình không có quyền.
    member.require_admin()?;
    let (staging, upload) = receive(&state, multipart).await?;
    let result = ingest::upload(&state, &member, staging, upload).await?;
    let status = if result.created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(result)))
}

// Liệt kê sheet của một file Excel để người dùng chọn trước khi nạp. File không được lưu lại.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/datasets/sheets",
    tag = "datasets",
    operation_id = "listExcelSheets",
    summary = "Sheets of an Excel file, to choose which to upload (admin only)",
    description = "The file is read and discarded.",
    params(("ws" = String, Path, description = "Workspace slug")),
    request_body(content = ListSheetsRequest, content_type = "multipart/form-data"),
    responses(
        (status = 200, body = Vec<SheetSummary>, description = "OK"),
        (status = "4XX", body = ErrorBody, description = "400 not a readable Excel file, 401, 403 not admin, \
                                                         404 not a member, 413 too large")
    )
)]
async fn sheets(
    State(state): State<AppState>,
    member: Member,
    multipart: Result<Multipart, axum::extract::multipart::MultipartRejection>,
) -> Result<Json<Vec<SheetSummary>>, AppError> {
    member.require_admin()?;
    let (staging, upload) = receive(&state, multipart).await?;
    Ok(Json(excel::list_sheets(&state, staging, upload).await?))
}

/// Nhận multipart: ghi từng file xuống thư mục tạm, đọc các trường chữ, đếm tổng dung lượng.
async fn receive(
    state: &AppState,
    multipart: Result<Multipart, axum::extract::multipart::MultipartRejection>,
) -> Result<(Staging, Upload), AppError> {
    let mut multipart = multipart.map_err(|_| {
        AppError::BadRequest("Cần gửi file dạng multipart/form-data, trường file".to_string())
    })?;
    let staging = Staging::new(&state.config.data_dir)?;
    let mut upload = Upload {
        name: None,
        dataset_id: None,
        files: Vec::new(),
        sheets: Vec::new(),
    };
    let limit = state.config.max_upload_bytes;
    let mut received: u64 = 0;

    let mut fields = 0;
    while let Some(mut field) = multipart.next_field().await.map_err(multipart_error)? {
        fields += 1;
        if fields > MAX_FIELDS {
            return Err(AppError::BadRequest(format!(
                "Quá {MAX_FIELDS} trường trong một lần nạp"
            )));
        }
        let field_name = field.name().unwrap_or_default().to_string();
        match field_name.as_str() {
            "file" => {
                let name = file_name_only(field.file_name().unwrap_or_default());
                if name.is_empty() {
                    return Err(AppError::BadRequest(
                        "Trường file thiếu tên file".to_string(),
                    ));
                }
                if name.chars().count() > MAX_FILE_NAME {
                    return Err(AppError::BadRequest(format!(
                        "Tên file dài tối đa {MAX_FILE_NAME} ký tự"
                    )));
                }
                let path = staging.upload_path(upload.files.len());
                let mut out = tokio::fs::File::create(&path)
                    .await
                    .map_err(|e| AppError::Internal(format!("create {}: {e}", path.display())))?;
                let mut hasher = Sha256::new();
                while let Some(chunk) = field.chunk().await.map_err(multipart_error)? {
                    received += chunk.len() as u64;
                    check_limit(received, limit)?;
                    hasher.update(&chunk);
                    out.write_all(&chunk).await.map_err(|e| {
                        AppError::Internal(format!("write {}: {e}", path.display()))
                    })?;
                }
                out.flush()
                    .await
                    .map_err(|e| AppError::Internal(format!("flush {}: {e}", path.display())))?;
                let sha256 = hasher
                    .finalize()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect();
                upload.files.push(StagedFile { path, name, sha256 });
            }
            "name" | "dataset_id" | "sheet" => {
                // Trường chữ cũng đếm vào giới hạn, để không ai gửi một "tên" dài hàng GB.
                let mut bytes = Vec::new();
                while let Some(chunk) = field.chunk().await.map_err(multipart_error)? {
                    received += chunk.len() as u64;
                    check_limit(received, limit)?;
                    bytes.extend_from_slice(&chunk);
                }
                let text = String::from_utf8(bytes).map_err(|_| {
                    AppError::BadRequest(format!("Trường {field_name} không phải chữ UTF-8"))
                })?;
                match field_name.as_str() {
                    "name" => upload.name = Some(text),
                    "dataset_id" => upload.dataset_id = Some(text),
                    _ => upload.sheets.push(text),
                }
            }
            // Trường lạ: bỏ qua nhưng vẫn đọc hết, vẫn đếm dung lượng.
            _ => {
                while let Some(chunk) = field.chunk().await.map_err(multipart_error)? {
                    received += chunk.len() as u64;
                    check_limit(received, limit)?;
                }
            }
        }
    }

    Ok((staging, upload))
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/datasets/{id}",
    tag = "datasets",
    operation_id = "getDataset",
    summary = "One dataset of the workspace",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Dataset id (UUID)")
    ),
    responses(
        (status = 200, body = Dataset, description = "OK"),
        (status = "4XX", body = ErrorBody, description = "401, 404 not a member or no such dataset")
    )
)]
async fn detail(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
) -> Result<Json<Dataset>, AppError> {
    Ok(Json(ingest::dataset_detail(&state, &member, &id).await?))
}

fn check_limit(received: u64, limit: u64) -> Result<(), AppError> {
    if received > limit {
        return Err(AppError::TooLarge(format!(
            "Tổng dung lượng vượt giới hạn {} MB của một lần nạp (MAX_UPLOAD_MB)",
            limit / 1024 / 1024
        )));
    }
    Ok(())
}

/// Bỏ phần thư mục nếu trình duyệt hay công cụ gửi kèm ("../../a.csv" hay đường dẫn Windows thành "a.csv"). Tên này chỉ
/// để hiển thị và ghi vào _source_file; đường dẫn trên đĩa do máy chủ đặt.
fn file_name_only(raw: &str) -> String {
    raw.rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn multipart_error(e: MultipartError) -> AppError {
    tracing::debug!(error = %e, "multipart rejected");
    // Vượt giới hạn body cả request (DefaultBodyLimit) cũng đi qua đây.
    if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
        return AppError::TooLarge(
            "Tổng dung lượng vượt giới hạn của một lần nạp (MAX_UPLOAD_MB)".to_string(),
        );
    }
    AppError::BadRequest("Dữ liệu multipart bị hỏng hoặc gửi không trọn".to_string())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use axum::http::StatusCode;
    use serde_json::{json, Value};

    use crate::api::test_support::{app_with, call, multipart, send_raw, CODE};
    use crate::engine::store::test_support::{fetch_count, fresh_database};

    const URL: &str = "/api/workspaces/cua-hang/datasets";

    /// Thư mục dữ liệu riêng cho một test, tự xóa khi ra khỏi phạm vi.
    struct DataDir(PathBuf);

    impl DataDir {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("xdi-upload-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            DataDir(dir)
        }
    }

    impl Drop for DataDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Đếm file .parquet dưới thư mục dữ liệu, bỏ qua thư mục tạm.
    fn parquet_files(dir: &Path) -> usize {
        let mut count = 0;
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for entry in std::fs::read_dir(&d).unwrap().flatten() {
                let p = entry.path();
                if p.is_dir() && p.file_name().is_some_and(|n| n != "staging") {
                    stack.push(p);
                } else if p.extension().is_some_and(|e| e == "parquet") {
                    count += 1;
                }
            }
        }
        count
    }

    fn staging_is_empty(dir: &Path) -> bool {
        std::fs::read_dir(dir.join("staging")).map_or(true, |mut it| it.next().is_none())
    }

    async fn setup(app: &axum::Router) -> String {
        let body = json!({"setup_code": CODE, "email": "lan@example.com", "password": "a long password",
                          "workspace_name": "Cửa hàng"});
        let (status, session, _) = call(app, "POST", "/api/auth/setup", None, Some(body)).await;
        assert_eq!(status, StatusCode::OK);
        session.unwrap()
    }

    async fn upload(
        app: &axum::Router,
        session: &str,
        fields: &[(&str, &str)],
        files: &[(&str, &[u8])],
    ) -> (StatusCode, Value) {
        let (content_type, body) = multipart(fields, files);
        send_raw(app, URL, session, &content_type, body).await
    }

    const T1: &[u8] = b"region,amount\nSouth,10\nNorth,20\n";
    const T2: &[u8] = b"amount,region\n30,South\n";

    #[tokio::test]
    async fn upload_creates_dataset_versions_and_skips_duplicates() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let data = DataDir::new();
        let app = app_with(db.store.clone(), Some(CODE), |c| {
            c.data_dir = data.0.clone()
        });
        let lan = setup(&app).await;

        // Hai file cùng cột thành một dataset, phiên bản 1.
        let (status, body) = upload(
            &app,
            &lan,
            &[("name", "Bán hàng")],
            &[("thang1.csv", T1), ("thang2.csv", T2)],
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        assert_eq!(body["created"], true);
        assert_eq!(body["dataset"]["logical_table"], "ban_hang");
        assert_eq!(body["version"]["version"], 1);
        assert_eq!(body["version"]["row_count"], 3);
        assert_eq!(body["version"]["source_kind"], "csv");
        assert_eq!(body["dataset"]["current_version_id"], body["version"]["id"]);
        let columns: Vec<&str> = body["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap())
            .collect();
        assert_eq!(columns, ["region", "amount", "_source_file"]);
        // Hồ sơ cột có trước khi trả 201: đủ ba cột, cột ít giá trị có tần suất (region 2, amount 3, _source_file 2).
        assert_eq!(body["version"]["profile_status"], "structure");
        let version_id = body["version"]["id"].as_str().unwrap().to_string();
        let count = |table: &str| {
            let sql = match table {
                "columns" => {
                    format!("SELECT count(*) FROM columns WHERE version_id = '{version_id}'")
                }
                _ => format!(
                    "SELECT count(*) FROM column_values v JOIN columns c ON c.id = v.column_id \
                     WHERE c.version_id = '{version_id}'"
                ),
            };
            let store = db.store.clone();
            async move { fetch_count(&store, &sql).await }
        };
        assert_eq!(count("columns").await, 3);
        assert_eq!(count("column_values").await, 7);
        let dataset_id = body["dataset"]["id"].as_str().unwrap().to_string();
        assert_eq!(parquet_files(&data.0), 2);
        assert!(staging_is_empty(&data.0));

        // Đúng bộ file đó, thứ tự khác: bỏ qua, trả phiên bản cũ.
        let (status, again) = upload(
            &app,
            &lan,
            &[("dataset_id", &dataset_id)],
            &[("thang2.csv", T2), ("thang1.csv", T1)],
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            (again["created"].clone(), again["version"]["id"].clone()),
            (json!(false), body["version"]["id"].clone())
        );
        // Nạp lại như dataset mới cũng không tạo bản trùng.
        let (status, again) =
            upload(&app, &lan, &[], &[("thang1.csv", T1), ("thang2.csv", T2)]).await;
        assert_eq!(
            (status, again["dataset"]["id"].as_str()),
            (StatusCode::OK, Some(dataset_id.as_str()))
        );

        // Bộ file khác: phiên bản 2, thành phiên bản hiện hành.
        let t3: &[u8] = b"region,amount\nEast,5\n";
        let (status, v2) = upload(
            &app,
            &lan,
            &[("dataset_id", &dataset_id)],
            &[("thang1.csv", T1), ("thang3.csv", t3)],
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{v2}");
        assert_eq!(v2["version"]["version"], 2);
        assert_eq!(v2["dataset"]["current_version_id"], v2["version"]["id"]);

        // Dataset khác cùng tên: tên bảng logic không trùng.
        let (_, other) = upload(&app, &lan, &[("name", "Bán hàng")], &[("x.csv", b"a\n1\n")]).await;
        assert_eq!(other["dataset"]["logical_table"], "ban_hang_2");

        let (_, _, list) = call(&app, "GET", URL, Some(&lan), None).await;
        assert_eq!(list.as_array().map(Vec::len), Some(2));

        db.drop().await;
    }

    #[tokio::test]
    async fn bad_uploads_leave_nothing_behind() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let data = DataDir::new();
        let app = app_with(db.store.clone(), Some(CODE), |c| {
            c.data_dir = data.0.clone();
            c.max_upload_bytes = 64;
        });
        let lan = setup(&app).await;

        let (status, body) = upload(
            &app,
            &lan,
            &[],
            &[("a.csv", T1), ("b.csv", b"region,qty\nSouth,1\n")],
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            body["message"].as_str().unwrap().contains("b.csv"),
            "{body}"
        );

        let (status, _) = upload(&app, &lan, &[], &[("data.xlsx", b"x")]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, _) = upload(&app, &lan, &[], &[("a.csv", T1), ("b.parquet", b"x")]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, _) = upload(&app, &lan, &[("name", "x")], &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let big = vec![b'a'; 100];
        let (status, body) = upload(&app, &lan, &[], &[("big.csv", &big)]).await;
        assert_eq!(
            (status, body["code"].as_str()),
            (StatusCode::PAYLOAD_TOO_LARGE, Some("too_large"))
        );

        // Giới hạn trong test là 64 byte: trường dataset_id đã 36 byte, nên dùng file nhỏ.
        let missing = uuid::Uuid::new_v4().to_string();
        let (status, _) = upload(
            &app,
            &lan,
            &[("dataset_id", &missing)],
            &[("a.csv", b"a\n1\n")],
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        let (status, body) = send_raw(&app, URL, &lan, "application/json", b"{}".to_vec()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

        let (_, _, list) = call(&app, "GET", URL, Some(&lan), None).await;
        assert_eq!(list.as_array().map(Vec::len), Some(0));
        assert_eq!(parquet_files(&data.0), 0);
        assert!(staging_is_empty(&data.0));

        db.drop().await;
    }

    #[tokio::test]
    async fn concurrent_duplicate_uploads_do_not_fail() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let data = DataDir::new();
        let app = app_with(db.store.clone(), Some(CODE), |c| {
            c.data_dir = data.0.clone()
        });
        let lan = setup(&app).await;
        let (_, first) = upload(&app, &lan, &[("name", "Bán hàng")], &[("thang1.csv", T1)]).await;
        let id = first["dataset"]["id"].as_str().unwrap().to_string();

        // Cùng bộ file mới, gửi hai lần cùng lúc vào cùng dataset: một bên tạo phiên bản, bên kia nhận phiên bản đó.
        let fields = [("dataset_id", id.as_str())];
        let files: [(&str, &[u8]); 1] = [("thang2.csv", T2)];
        let a = upload(&app, &lan, &fields, &files);
        let b = upload(&app, &lan, &fields, &files);
        let ((sa, ba), (sb, bb)) = tokio::join!(a, b);
        let mut statuses = [sa, sb];
        statuses.sort();
        assert_eq!(statuses, [StatusCode::OK, StatusCode::CREATED], "{ba} {bb}");
        assert_eq!(ba["version"]["id"], bb["version"]["id"]);
        assert!(staging_is_empty(&data.0));

        db.drop().await;
    }

    #[tokio::test]
    async fn field_flood_and_file_names_are_bounded() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let data = DataDir::new();
        let app = app_with(db.store.clone(), Some(CODE), |c| {
            c.data_dir = data.0.clone()
        });
        let lan = setup(&app).await;

        // Hàng nghìn trường rỗng: nội dung 0 byte nhưng vẫn bị chặn.
        let empty: Vec<(&str, &str)> = (0..1500).map(|_| ("x", "")).collect();
        let (status, _) = upload(&app, &lan, &empty, &[("a.csv", T1)]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let long = format!("{}.csv", "a".repeat(300));
        let (status, _) = upload(&app, &lan, &[], &[(long.as_str(), T1)]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        // Phần thư mục trong tên file bị bỏ: chỉ còn tên, cả ở tên dataset lẫn cột _source_file.
        let (status, body) = upload(&app, &lan, &[], &[("../../secret/orders.csv", T1)]).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        assert_eq!(body["dataset"]["name"], "orders");

        // Tên mặc định dài thì cắt còn 100 ký tự, không báo lỗi.
        let stem = "c".repeat(150);
        let name = format!("{stem}.csv");
        let (status, body) = upload(&app, &lan, &[], &[(name.as_str(), b"z\n9\n")]).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        assert_eq!(
            body["dataset"]["name"].as_str().map(|n| n.chars().count()),
            Some(100)
        );

        db.drop().await;
    }

    /// Nhiều lần nạp KHÁC nội dung vào cùng dataset cùng lúc: hai lần có thể tính ra cùng số phiên bản; bên vấp
    /// UNIQUE thử lại, không trả 500.
    #[tokio::test]
    async fn concurrent_different_uploads_all_succeed() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let data = DataDir::new();
        let app = app_with(db.store.clone(), Some(CODE), |c| {
            c.data_dir = data.0.clone()
        });
        let lan = setup(&app).await;
        let (_, first) = upload(&app, &lan, &[("name", "Bán hàng")], &[("thang1.csv", T1)]).await;
        let id = first["dataset"]["id"].as_str().unwrap().to_string();
        // Bốn lần nạp chạy cùng lúc, mỗi lần một nội dung khác nhau.
        let mut uploads = tokio::task::JoinSet::new();
        for i in 0..4 {
            let (app, lan, id) = (app.clone(), lan.clone(), id.clone());
            uploads.spawn(async move {
                let content = format!("region,amount\nSouth,{i}\n").into_bytes();
                upload(
                    &app,
                    &lan,
                    &[("dataset_id", id.as_str())],
                    &[("t.csv", content.as_slice())],
                )
                .await
            });
        }
        let mut versions: Vec<i64> = uploads
            .join_all()
            .await
            .iter()
            .map(|(status, body)| {
                assert_eq!(*status, StatusCode::CREATED, "{body}");
                body["version"]["version"].as_i64().unwrap()
            })
            .collect();
        versions.sort();
        assert_eq!(versions, [2, 3, 4, 5]);

        db.drop().await;
    }

    #[tokio::test]
    async fn odd_file_selections_are_handled() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let data = DataDir::new();
        let app = app_with(db.store.clone(), Some(CODE), |c| {
            c.data_dir = data.0.clone()
        });
        let lan = setup(&app).await;

        // Cùng một file chọn hai lần: báo, không nhân đôi số dòng.
        let (status, body) = upload(&app, &lan, &[], &[("a.csv", T1), ("b.csv", T1)]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        // Hai file khác thư mục, trùng tên gốc: báo để cột _source_file phân biệt được.
        let (status, _) = upload(
            &app,
            &lan,
            &[],
            &[("hanoi/thang1.csv", T1), ("hcm/thang1.csv", T2)],
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // Tên file chỉ có đuôi: tên dataset mặc định, không 500.
        let (status, body) = upload(&app, &lan, &[], &[(".csv", T1)]).await;
        assert_eq!(
            (status, body["dataset"]["name"].as_str()),
            (StatusCode::CREATED, Some("Dataset")),
            "{body}"
        );

        db.drop().await;
    }

    #[test]
    fn leftover_staging_is_cleared() {
        let data = DataDir::new();
        let leftover = data.0.join("staging").join("dead-upload").join("upload");
        std::fs::create_dir_all(&leftover).unwrap();
        std::fs::write(leftover.join("0000"), b"half").unwrap();
        crate::service::ingest::clear_staging(&data.0);
        assert!(!data.0.join("staging").exists());
        // Không có thư mục staging cũng không lỗi.
        crate::service::ingest::clear_staging(&data.0);
    }

    const DATACO_CSV: &[u8] = b"Type,Market,Order Id,Product Image,Order Item Total\n\
        DEBIT,Europe,1,img/1.jpg,100.5\n\
        CASH,LATAM,1,img/2.jpg,200.0\n\
        DEBIT,Europe,2,img/1.jpg,150.25\n";

    /// Nạp xong, hồ sơ ngữ nghĩa chạy nền tới xong: dataset tên DataCo (bảng logic dataco) nhận hồ sơ mock — mô tả,
    /// bí danh, vai trò sửa, 5 câu gợi ý, current_profile_status thành semantic. Xóa dòng spawn trong ingest thì test
    /// này đỏ.
    #[tokio::test]
    async fn upload_spawns_the_semantic_profile() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let data = DataDir::new();
        let app = app_with(db.store.clone(), Some(CODE), |c| {
            c.data_dir = data.0.clone();
            // Tường minh mock, không theo biến môi trường của máy chạy test.
            c.llm_provider = "mock".to_string();
        });
        let lan = setup(&app).await;

        let (status, body) =
            upload(&app, &lan, &[("name", "DataCo")], &[("d.csv", DATACO_CSV)]).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        assert_eq!(body["version"]["profile_status"], "structure");
        let dataset_id = body["dataset"]["id"].as_str().unwrap().to_string();
        let url = format!("{URL}/{dataset_id}");

        let mut detail = serde_json::Value::Null;
        for _ in 0..100 {
            let (_, _, body) = call(&app, "GET", &url, Some(&lan), None).await;
            if body["current_profile_status"] == "semantic" {
                detail = body;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert_eq!(detail["current_profile_status"], "semantic", "{detail}");
        assert_eq!(
            detail["suggested_questions"].as_array().map(Vec::len),
            Some(5),
            "{detail}"
        );

        db.drop().await;
    }

    /// Mô hình lỗi (provider ollama trỏ vào cổng không nghe): lần nạp vẫn 201, phiên bản giữ structure và được đánh
    /// dấu đã thử; nạp lại đúng bộ file đó trả 200 created = false và gọi lại mô hình lần nữa (vẫn lỗi, vẫn 200).
    #[tokio::test]
    async fn llm_failure_does_not_block_the_upload() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let data = DataDir::new();
        let app = app_with(db.store.clone(), Some(CODE), |c| {
            c.data_dir = data.0.clone();
            c.llm_provider = "ollama".to_string();
            // Cổng 9 (discard) hầu như luôn không có gì nghe: kết nối bị từ chối ngay, không phải chờ timeout.
            c.ollama_url = "http://127.0.0.1:9".to_string();
        });
        let lan = setup(&app).await;

        let (status, body) =
            upload(&app, &lan, &[("name", "DataCo")], &[("d.csv", DATACO_CSV)]).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        let version_id = body["version"]["id"].as_str().unwrap().to_string();
        let attempted = format!(
            "SELECT count(*) FROM dataset_versions WHERE id = '{version_id}' AND semantic_attempted_at IS NOT NULL"
        );
        for _ in 0..100 {
            if fetch_count(&db.store, &attempted).await == 1 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert_eq!(fetch_count(&db.store, &attempted).await, 1);
        let status_of = format!(
            "SELECT count(*) FROM dataset_versions WHERE id = '{version_id}' AND profile_status = 'structure'"
        );
        assert_eq!(fetch_count(&db.store, &status_of).await, 1);

        // Nạp lại đúng bộ file: không tạo gì mới, nhưng là đường thử lại tường minh của lớp ngữ nghĩa.
        let (status, body) =
            upload(&app, &lan, &[("name", "DataCo")], &[("d.csv", DATACO_CSV)]).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["created"], false);

        db.drop().await;
    }
}
