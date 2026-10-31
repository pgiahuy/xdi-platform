// SPDX-License-Identifier: Apache-2.0
/* api/sources_tests.rs - test nguồn Postgres qua route thật (#165). Database ngoài là chính database riêng của test
(fresh_database): test tạo một bảng nhỏ trong đó rồi thêm nguồn trỏ vào nó, chụp thành dataset và hỏi. Kiểm thêm mật
khẩu không có trong response và log, máy chủ ngoài danh sách bị từ chối, member nhận 403, thiếu khóa thì tính năng
tắt. Không có DATABASE_URL thì bỏ qua. */

use std::io::Write;
use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use serde_json::{json, Value};

use super::ask_tests::{scripted, sql_reply};
use crate::api::test_support::{app_with_llm, call, CODE};
use crate::config::{Config, Secret};
use crate::engine::store::test_support::{exec, fresh_database, TestDb};

const PASSWORD: &str = "a long password";
pub(super) const SHOP: &str = "/api/workspaces/cua-hang";
pub(super) const KEY: &str = "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=";

/// Máy, cổng, tài khoản, mật khẩu của DATABASE_URL dạng postgres://user:pw@host:port/db.
fn url_parts(url: &str) -> (String, u16, String, String) {
    let rest = url.split_once("://").unwrap().1;
    let authority = rest.split(['/', '?']).next().unwrap();
    let (user_info, host_port) = authority.rsplit_once('@').unwrap();
    let (user, password) = user_info.split_once(':').unwrap_or((user_info, ""));
    let (host, port) = host_port.split_once(':').unwrap_or((host_port, "5432"));
    (
        host.to_string(),
        port.parse().unwrap(),
        user.to_string(),
        password.to_string(),
    )
}

/// Ghi log của tracing vào bộ nhớ, để kiểm mật khẩu không bị ghi ra.
#[derive(Clone, Default)]
struct LogBuffer(Arc<Mutex<Vec<u8>>>);

impl Write for LogBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(super) struct Setup {
    pub db: TestDb,
    pub data: std::path::PathBuf,
    pub app: axum::Router,
    pub lan: String,
    pub minh: String,
    pub form: Value,
    pub user: String,
    password: String,
}

/// Lan admin, Minh member của "Cửa hàng"; bảng public.orders trong database của test. key None: tính năng tắt.
pub(super) async fn start(key: Option<&str>, llm_replies: &[Value]) -> Option<Setup> {
    start_with_max_rows(key, llm_replies, None).await
}

/// Như start, kèm SOURCE_MAX_ROWS khác mặc định.
pub(super) async fn start_with_max_rows(
    key: Option<&str>,
    llm_replies: &[Value],
    max_rows: Option<u64>,
) -> Option<Setup> {
    start_with_llm(key, scripted(llm_replies), max_rows).await
}

/// Như start, với một mô hình giả bất kỳ (ví dụ mô hình giữ lại prompt, ask_tests::recording).
pub(super) async fn start_with_llm(
    key: Option<&str>,
    llm: std::sync::Arc<dyn crate::engine::llm::LlmProvider>,
    max_rows: Option<u64>,
) -> Option<Setup> {
    start_with_snapshot_mode(key, llm, max_rows, None).await
}

/// Như start_with_llm, kèm SOURCE_SNAPSHOT_MODE và đường dẫn extension postgres của DuckDB (source_snapshot_tests.rs).
pub(super) async fn start_with_snapshot_mode(
    key: Option<&str>,
    llm: std::sync::Arc<dyn crate::engine::llm::LlmProvider>,
    max_rows: Option<u64>,
    snapshot_mode: Option<(crate::config::SnapshotMode, std::path::PathBuf)>,
) -> Option<Setup> {
    start_configured(key, llm, move |c: &mut Config| {
        if let Some(max_rows) = max_rows {
            c.source_max_rows = max_rows;
        }
        if let Some((mode, extension)) = snapshot_mode {
            c.source_snapshot_mode = mode;
            c.source_postgres_extension = extension;
        }
    })
    .await
}

/// Như start_with_llm, với một hàm đổi cấu hình bất kỳ (trần dòng, trần dung lượng, cách chép).
pub(super) async fn start_configured(
    key: Option<&str>,
    llm: std::sync::Arc<dyn crate::engine::llm::LlmProvider>,
    extra: impl FnOnce(&mut Config),
) -> Option<Setup> {
    let db = fresh_database().await?;
    let url = crate::engine::store::test_support::database_url().unwrap();
    let (host, port, _, _) = url_parts(&url);
    // Tài khoản riêng chỉ có quyền SELECT, như tài liệu khuyên; mật khẩu đủ đặc trưng để tìm trong response và log.
    let tag = &uuid::Uuid::new_v4().simple().to_string()[..12];
    let user = format!("xdi_src_{tag}");
    let password = format!("pw-{tag}-secret");
    exec(
        &db.store,
        "CREATE TABLE orders (region text, amount numeric(10,2), ordered_at date)",
    )
    .await;
    exec(
        &db.store,
        "INSERT INTO orders VALUES ('Bắc', 100.50, '2024-01-05'), ('Nam', 200, '2024-01-06'), ('Bắc', 50, '2024-02-01')",
    )
    .await;
    exec(
        &db.store,
        &format!("CREATE ROLE {user} LOGIN PASSWORD '{password}'"),
    )
    .await;
    exec(&db.store, &format!("GRANT SELECT ON orders TO {user}")).await;
    let data = std::env::temp_dir().join(format!("xdi-sources-api-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&data).unwrap();
    let data_dir = data.clone();
    let allowed = host.to_lowercase();
    let key = key.map(str::to_string);
    let configure = move |c: &mut Config| {
        c.data_dir = data_dir;
        c.source_secret_key = Secret::new(key.unwrap_or_default());
        c.source_allowed_hosts = vec![allowed];
        extra(c);
    };
    let app = app_with_llm(db.store.clone(), Some(CODE), configure, llm);
    let setup = json!({"setup_code": CODE, "email": "lan@example.com", "password": PASSWORD,
                       "workspace_name": "Cửa hàng"});
    let (status, lan, _) = call(&app, "POST", "/api/auth/setup", None, Some(setup)).await;
    assert_eq!(status, StatusCode::OK);
    let lan = lan.unwrap();
    let user_body = json!({"email": "minh@example.com", "password": PASSWORD});
    let (status, _, _) = call(
        &app,
        "POST",
        "/api/admin/users",
        Some(&lan),
        Some(user_body),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let add = json!({"email": "minh@example.com", "role": "member"});
    let (status, _, _) = call(
        &app,
        "POST",
        &format!("{SHOP}/members"),
        Some(&lan),
        Some(add),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let login = json!({"email": "minh@example.com", "password": PASSWORD});
    let (_, minh, _) = call(&app, "POST", "/api/auth/login", None, Some(login)).await;
    let form = json!({"host": host, "port": port, "database": db.name(), "user": user,
                      "password": password, "sslmode": "disable"});
    Some(Setup {
        db,
        data,
        app,
        lan,
        minh: minh.unwrap(),
        form,
        user,
        password,
    })
}

impl Setup {
    /// Tài khoản Postgres là của cả máy chủ, không mất theo database: xóa sau khi xóa database của test.
    pub(super) async fn finish(self) {
        let _ = std::fs::remove_dir_all(&self.data);
        self.db.drop().await;
        let url = crate::engine::store::test_support::database_url().unwrap();
        let admin =
            crate::engine::store::Store::connect(&url, 1, std::time::Duration::from_secs(5))
                .await
                .unwrap();
        exec(&admin, &format!("DROP ROLE IF EXISTS {}", self.user)).await;
    }
}

/// Thêm nguồn, liệt kê bảng, chụp bảng, hỏi trên dataset vừa chụp, chụp lại; mật khẩu không có trong response hay log.
#[tokio::test]
async fn snapshot_table_becomes_a_dataset() {
    let sql = "SELECT region, SUM(amount) AS total FROM orders GROUP BY region ORDER BY region";
    let Some(s) = start(Some(KEY), &[sql_reply(sql)]).await else {
        return;
    };
    // Bắt log sau start: câu CREATE ROLE của chính test có mật khẩu. Test chạy trên runtime một luồng, nên việc nền
    // của các request dưới đây cũng ghi vào subscriber này.
    let logs = LogBuffer::default();
    let writer = logs.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(move || writer.clone())
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);
    // tracing nhớ toàn cục điểm log nào có người nghe; test khác chạy song song có thể đã ghi nhớ "không ai nghe" trước
    // khi subscriber này có. Tính lại để các điểm log đó ghi vào đây.
    tracing::callsite::rebuild_interest_cache();
    let mut bodies = Vec::new();

    let (status, _, source) = call(
        &s.app,
        "POST",
        &format!("{SHOP}/sources"),
        Some(&s.lan),
        Some(s.form.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{source}");
    assert!(source.get("password").is_none());
    let id = source["id"].as_str().unwrap().to_string();
    bodies.push(source.to_string());

    let (status, _, list) = call(
        &s.app,
        "GET",
        &format!("{SHOP}/sources"),
        Some(&s.lan),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    bodies.push(list.to_string());

    let (status, _, tables) = call(
        &s.app,
        "GET",
        &format!("{SHOP}/sources/{id}/tables"),
        Some(&s.lan),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{tables}");
    assert!(tables
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["schema"] == "public" && t["name"] == "orders"));

    let snap = json!({"table": "public.orders"});
    let (status, _, result) = call(
        &s.app,
        "POST",
        &format!("{SHOP}/sources/{id}/snapshot"),
        Some(&s.lan),
        Some(snap),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{result}");
    bodies.push(result.to_string());
    assert_eq!(result["datasets"][0]["version"]["source_kind"], "postgres");
    assert_eq!(result["datasets"][0]["version"]["row_count"], 3);
    assert_eq!(result["datasets"][0]["dataset"]["source_kind"], "postgres");
    assert_eq!(result["datasets"][0]["dataset"]["source_id"], id.as_str());
    assert_eq!(
        result["datasets"][0]["dataset"]["source_table"],
        "public.orders"
    );
    assert_eq!(result["datasets"][0]["dataset"]["logical_table"], "orders");
    let dataset_id = result["datasets"][0]["dataset"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let ask = json!({"question": "Doanh thu theo vùng", "choices": []});
    let (status, _, answer) = call(
        &s.app,
        "POST",
        &format!("{SHOP}/datasets/{dataset_id}/ask"),
        Some(&s.lan),
        Some(ask),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["status"], "answered", "{answer}");
    let text = answer.to_string();
    assert!(text.contains("Bắc") && text.contains("150.5"), "{text}");

    // Chụp lại khi bảng không đổi: không tạo phiên bản. Thêm dòng rồi chụp lại: phiên bản 2.
    let again = json!({"dataset_id": dataset_id});
    let url = format!("{SHOP}/sources/{id}/snapshot");
    let (status, _, same) = call(&s.app, "POST", &url, Some(&s.lan), Some(again.clone())).await;
    assert_eq!(status, StatusCode::OK, "{same}");
    exec(
        &s.db.store,
        "INSERT INTO orders VALUES ('Nam', 10, '2024-03-01')",
    )
    .await;
    let (status, _, next) = call(&s.app, "POST", &url, Some(&s.lan), Some(again)).await;
    assert_eq!(status, StatusCode::CREATED, "{next}");
    assert_eq!(next["datasets"][0]["version"]["version"], 2);
    assert_eq!(next["datasets"][0]["version"]["row_count"], 4);

    // Nguồn còn dataset thì không xóa được; xóa dataset rồi xóa nguồn.
    let (status, _, _) = call(
        &s.app,
        "DELETE",
        &format!("{SHOP}/sources/{id}"),
        Some(&s.lan),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = call(
        &s.app,
        "DELETE",
        &format!("{SHOP}/datasets/{dataset_id}"),
        Some(&s.lan),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _, _) = call(
        &s.app,
        "DELETE",
        &format!("{SHOP}/sources/{id}"),
        Some(&s.lan),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    drop(guard);
    // Khi cả bộ test chạy song song, subscriber theo luồng của tracing đôi khi vẫn lỡ vài dòng (mức log tối đa và cache
    // điểm log là toàn cục), nên không đòi phải bắt được một dòng cụ thể. Phần chắc chắn nằm ở code: ConnectInfo không
    // có Debug, log của service/sources chỉ ghi máy chủ và cổng, sqlx tắt ghi câu SQL trên kết nối nguồn.
    let log_text = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
    for body in &bodies {
        assert!(!body.contains(&s.password), "password in response: {body}");
    }
    assert!(!log_text.contains(&s.password), "password in logs");
    s.finish().await;
}

/// Máy chủ ngoài danh sách, sai mật khẩu, member, thiếu khóa.
#[tokio::test]
async fn sources_are_guarded() {
    let Some(s) = start(Some(KEY), &[]).await else {
        return;
    };
    let url = format!("{SHOP}/sources");

    let mut other_host = s.form.clone();
    other_host["host"] = json!("10.1.2.3");
    let (status, _, body) = call(&s.app, "POST", &url, Some(&s.lan), Some(other_host)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("SOURCE_ALLOWED_HOSTS"),
        "{body}"
    );

    let mut wrong = s.form.clone();
    wrong["password"] = json!("not-the-password");
    let (status, _, body) = call(&s.app, "POST", &url, Some(&s.lan), Some(wrong)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["message"].as_str().unwrap().contains("mật khẩu"),
        "{body}"
    );

    let (status, _, source) = call(&s.app, "POST", &url, Some(&s.lan), Some(s.form.clone())).await;
    assert_eq!(status, StatusCode::CREATED, "{source}");
    let id = source["id"].as_str().unwrap();
    for (method, path, body) in [
        ("GET", url.clone(), None),
        ("POST", url.clone(), Some(s.form.clone())),
        ("POST", format!("{url}/{id}/test"), None),
        ("GET", format!("{url}/{id}/tables"), None),
        (
            "POST",
            format!("{url}/{id}/snapshot"),
            Some(json!({"table": "public.orders"})),
        ),
        ("DELETE", format!("{url}/{id}"), None),
    ] {
        let (status, _, _) = call(&s.app, method, &path, Some(&s.minh), body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}");
    }

    let (status, _, body) = call(
        &s.app,
        "POST",
        &format!("{url}/{id}/test"),
        Some(&s.lan),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // Tên bảng không có trong danh sách đọc được thì từ chối, không đưa vào câu SQL.
    let bad = json!({"table": "public.orders; DROP TABLE orders"});
    let (status, _, _) = call(
        &s.app,
        "POST",
        &format!("{url}/{id}/snapshot"),
        Some(&s.lan),
        Some(bad),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    s.finish().await;

    let Some(off) = start(None, &[]).await else {
        return;
    };
    let (status, _, body) = call(
        &off.app,
        "POST",
        &url,
        Some(&off.lan),
        Some(off.form.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert!(body["message"]
        .as_str()
        .unwrap()
        .contains("SOURCE_SECRET_KEY"));
    off.finish().await;
}

/// Kiểu cột của bản chụp lấy từ Postgres, không do DuckDB đoán từ phần đầu file: cột sku toàn số trong 25.000 dòng
/// đầu (quá mẫu 20.480 dòng của DuckDB) rồi mới có chữ vẫn chụp được, mã có số 0 ở đầu giữ nguyên, int và numeric(p,s)
/// giữ kiểu, chuỗi rỗng khác NULL.
#[tokio::test]
async fn snapshot_keeps_postgres_column_types() {
    let Some(s) = start(Some(KEY), &[]).await else {
        return;
    };
    exec(
        &s.db.store,
        "CREATE TABLE items (code text, sku text, note text, qty int, price numeric(10,2), ratio numeric, ok boolean,
                             at timestamptz, day date)",
    )
    .await;
    exec(
        &s.db.store,
        "INSERT INTO items
         SELECT lpad(i::text, 5, '0'), (100000 + i)::text, 'x', i, i * 1.25, i / 3.0, i % 2 = 0,
                timestamptz '2024-03-17 10:00:00+07' + i * interval '1 minute', date '2024-01-01' + i % 30
         FROM generate_series(1, 25000) AS i",
    )
    .await;
    exec(
        &s.db.store,
        "INSERT INTO items VALUES
         ('A1', '85123A', '', 1, 2.50, 0.5, true, '2024-03-17 10:00:00+07', '2024-01-01'),
         ('A2', '85124B', NULL, 2, NULL, NULL, NULL, NULL, NULL)",
    )
    .await;
    exec(&s.db.store, &format!("GRANT SELECT ON items TO {}", s.user)).await;

    let url = format!("{SHOP}/sources");
    let (status, _, source) = call(&s.app, "POST", &url, Some(&s.lan), Some(s.form.clone())).await;
    assert_eq!(status, StatusCode::CREATED, "{source}");
    let id = source["id"].as_str().unwrap();
    let snap = json!({"table": "public.items"});
    let (status, _, result) = call(
        &s.app,
        "POST",
        &format!("{url}/{id}/snapshot"),
        Some(&s.lan),
        Some(snap),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{result}");
    assert_eq!(result["datasets"][0]["version"]["row_count"], 25_002);
    let types: Vec<(String, String)> = result["datasets"][0]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let text = |key: &str| c[key].as_str().unwrap().to_string();
            (text("name"), text("data_type"))
        })
        .collect();
    let expected = [
        ("code", "VARCHAR"),
        ("sku", "VARCHAR"),
        ("note", "VARCHAR"),
        ("qty", "INTEGER"),
        ("price", "DECIMAL(10,2)"),
        ("ratio", "DOUBLE"),
        ("ok", "BOOLEAN"),
        ("at", "TIMESTAMP WITH TIME ZONE"),
        ("day", "DATE"),
    ];
    for (name, data_type) in expected {
        assert!(
            types.contains(&(name.to_string(), data_type.to_string())),
            "{name} {data_type}: {types:?}"
        );
    }

    let parquet = s
        .data
        .join(
            result["datasets"][0]["version"]["parquet_path"]
                .as_str()
                .unwrap(),
        )
        .join("*.parquet");
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let count = |filter: &str| -> i64 {
        let sql = format!(
            "SELECT count(*) FROM read_parquet('{}') WHERE {filter}",
            parquet.display()
        );
        conn.query_row(&sql, [], |row| row.get(0)).unwrap()
    };
    assert_eq!(count("code = '00123'"), 1);
    assert_eq!(count("sku = '85123A' AND note = ''"), 1);
    assert_eq!(count("sku = '85124B' AND note IS NULL"), 1);
    // timestamptz 10:00 giờ +07 là 03:00 UTC: epoch của 2024-03-17 03:00:00 UTC.
    assert_eq!(
        count("sku = '85123A' AND epoch(\"at\")::BIGINT = 1710644400"),
        1
    );
    assert_eq!(count("code = '00004' AND price = 5.00 AND ok"), 1);
    s.finish().await;
}

/// Máy chủ không bật TLS (Postgres của test, ssl = off như CI): require và verify-full báo lỗi TLS rõ cách sửa, không
/// lưu nguồn; prefer vẫn kết nối được. Máy chủ của test bật TLS thì bỏ qua phần lỗi.
#[tokio::test]
async fn tls_modes_against_a_server_without_tls() {
    let Some(s) = start(Some(KEY), &[]).await else {
        return;
    };
    let url = format!("{SHOP}/sources");
    let ssl = crate::engine::store::test_support::fetch_text(&s.db.store, "SHOW ssl").await;
    if ssl == "off" {
        for mode in ["require", "verify-full"] {
            let mut form = s.form.clone();
            form["sslmode"] = json!(mode);
            let (status, _, body) = call(&s.app, "POST", &url, Some(&s.lan), Some(form)).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
            let message = body["message"].as_str().unwrap();
            assert!(
                message.contains("TLS") && message.contains("prefer"),
                "{body}"
            );
        }
        let (_, _, list) = call(&s.app, "GET", &url, Some(&s.lan), None).await;
        assert_eq!(list.as_array().unwrap().len(), 0, "{list}");
    }
    let mut prefer = s.form.clone();
    prefer["sslmode"] = json!("prefer");
    let (status, _, source) = call(&s.app, "POST", &url, Some(&s.lan), Some(prefer)).await;
    assert_eq!(status, StatusCode::CREATED, "{source}");
    assert_eq!(source["custom_ca"], false);
    s.finish().await;
}
