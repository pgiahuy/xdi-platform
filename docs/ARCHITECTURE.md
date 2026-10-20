# Kiến trúc

Tài liệu này mô tả hệ thống như được thiết kế. Mục "Trạng thái" ở cuối ghi phần nào đã có, phần nào chưa.

## 1. Sơ đồ tổng thể

```
                                 TRÌNH DUYỆT
                 Next.js App Router, Tailwind, shadcn/ui, Recharts
            Bố cục 3 pane: dữ liệu | hỏi, bảng, biểu đồ | bằng chứng
                                      |
                              REST JSON, SSE cho ask
                     kiểu TypeScript sinh từ openapi.json
                                      |
=================================== docker compose =============================
                                      |
   +----------------------------------v-----------------------------------+
   |                      BACKEND  (Rust, Axum, 1 container)              |
   |                                                                      |
   |  api/       handler, request/response struct, JWT guard, OpenAPI     |
   |     |                                                                |
   |  service/   ingest (register, profile, materialize)                  |
   |             ask    (schema context -> LLM -> guard -> run -> explain)|
   |             explain (phân rã chênh lệch, tổng đóng góp = chênh lệch) |
   |     |                                                                |
   |  engine/    store.rs      sqlx  ----------------> POSTGRES           |
   |             analytics.rs  [DuckDB nhúng, chạy trong tiến trình này,  |
   |                            không container] -----> volume Parquet    |
   |             guard.rs      sqlparser, whitelist bảng và cột, tiêm RLS |
   |             llm/          trait LlmProvider ----> gemini | ollama | mock
   |  auth/      JWT (workspace, user, role), demo-login, OAuth tùy chọn     |
   +----------------------------------------------------------------------+
            |                              |
   +--------v-----------+        +---------v-------------------+
   |   POSTGRES         |        |  VOLUME /data               |
   |  (1 container)     |        |  <workspace>/<dataset>/     |
   |  mọi bảng có       |        |    <ver>/*.parquet          |
   |  workspace_id      |        |  (samples/ gắn riêng, chỉ đọc)|
   |                    |        +-----------------------------+
   |  workspaces        |
   |  memberships       |                   ngoài compose, tùy chọn
   |  users             |        +--------------------+   +-----------------+
   |  sources           |        |  Gemini API        |   |  Ollama local   |
   |  datasets          |        |  (có key)          |   |  (offline)      |
   |  dataset_versions  |        +--------------------+   +-----------------+
   |  columns           |
   |  column_values     |
   |  metrics           |
   |  rls_policies      |
   |  queries, settings |
   +--------------------+
```

Ba container: `backend`, `postgres`, `frontend`. DuckDB là thư viện nằm trong backend, không có tiến trình riêng.

## 2. Một câu hỏi đi qua chín bước

```
1. api        xác thực JWT, lấy workspace, role và dataset id
2. store      đọc hồ sơ cột, giá trị phân loại, chính sách phân quyền của role     (Postgres)
3. ask        ghép prompt: schema context + bí danh + câu hỏi mẫu đã đúng
4. llm        LlmProvider::complete -> SQL thô trên tên bảng logic
5. guard      parse AST, chỉ SELECT, whitelist bảng và cột, tiêm WHERE theo phân quyền, giới hạn dòng
6. analytics  resolve tên logic -> Parquet, chạy SQL, trả dòng và thời gian           (DuckDB)
7. explain    nếu là câu "tại sao": phân rã chênh lệch theo chiều, LLM chỉ diễn giải số
8. store      ghi queries: câu hỏi, SQL thô, SQL sau guard, thời gian, version_id
9. api        trả gói: SQL thô, SQL sau guard, dòng, gợi ý biểu đồ, phân rã, timeline từng bước
```

Mô hình ngôn ngữ chỉ chạm hai chỗ, bước 4 và bước 7. Nó không bao giờ đọc dữ liệu trực tiếp và không biết gì về
phân quyền. Mọi bước đều được ghi lại và trả về cho giao diện hiển thị.

## 3. Ba tầng, phụ thuộc một chiều

```
backend/src/
  main.rs            khởi động: đọc config, mở Postgres và DuckDB, dựng router
  config.rs          biến môi trường
  api/               tầng HTTP: handler, request/response struct, lỗi -> status
  service/           tầng nghiệp vụ: ingest.rs, ask.rs, explain.rs; không biết gì về HTTP
  engine/            tầng hạ tầng: analytics.rs (DuckDB), store.rs (Postgres), guard.rs, llm/
  auth/              JWT, role, demo-login
  error.rs           một enum lỗi chung
```

- `api` chỉ gọi `service`. `service` gọi `engine` và `auth`. Không có chiều ngược.
- Trait duy nhất là `LlmProvider` với hàm `complete`; hàm `embed` thêm khi bật tìm theo nghĩa. Chọn implementation lúc khởi động theo cấu hình:
  `gemini`, `ollama`, hoặc `mock`. Mock trả SQL cố định cho bộ câu hỏi mẫu, dùng khi không có khóa và trong test.
- Postgres và DuckDB không bọc trait. Service nhận struct `Store` và `Analytics` cụ thể.
- Trạng thái chia sẻ qua một `AppState`: `Store`, `Analytics`, `Arc<dyn LlmProvider>`, config. Không biến toàn cục.
- Test ở tầng service với mock LLM, DuckDB in-memory và Postgres trong CI.

## 4. Hai kho dữ liệu

| Kho | Chứa | Vì sao |
|---|---|---|
| Postgres (sqlx, migrations trong `backend/migrations/`) | `workspaces`, `memberships`, `users`, `sources`, `datasets`, `dataset_versions`, `columns`, `column_values`, `metrics`, `rls_policies`, `queries`, `settings`; mọi bảng có `workspace_id` | Nhiều ghi nhỏ, nhiều kết nối, đúng việc của database giao dịch |
| DuckDB nhúng | Không giữ gì bền. Đọc Parquet trên volume theo từng câu SQL | Engine phân tích theo cột, nhận SQL chuẩn, đọc thẳng CSV, Parquet, xlsx, không thêm tiến trình |

Image Postgres là `pgvector/pgvector`, có sẵn `unaccent`, `pg_trgm`, `vector`.

## 4b. Workspace và phân quyền

Một bản cài phục vụ một tổ chức. Workspace là phòng ban trong tổ chức đó, mỗi workspace có dataset, chỉ số và vai
riêng, vì các phòng thường tính chỉ số khác nhau: phòng kinh doanh tính doanh thu gộp theo ngày đặt, phòng tài chính
tính doanh thu ròng theo ngày giao. Cài xong có sẵn một workspace mặc định.

Ba tầng tách riêng:

| Tầng | Câu hỏi | Ví dụ |
|---|---|---|
| Workspace | Người dùng thuộc phòng ban nào | Người phòng kinh doanh không thấy dataset, chỉ số, lịch sử của phòng tài chính, trừ khi được mời |
| Vai trong ứng dụng | Trong workspace, được làm gì | Owner quản lý thành viên; Admin nạp dữ liệu, sửa chỉ số và chính sách; Analyst hỏi và lưu; Viewer chỉ xem |
| Lọc dòng | Trong dataset, thấy dòng nào | Vai Sales Manager South chỉ thấy `region = South`; guard thêm điều kiện vào SQL |

Mọi bảng dùng chung, có cột `workspace_id`. Mọi hàm trong `engine/store.rs` nhận kiểu `WorkspaceId`, chỉ tạo được từ JWT đã
xác thực, nên quên lọc theo workspace là lỗi biên dịch. Postgres Row Level Security là lớp chặn thứ hai. File Parquet nằm
dưới `/data/<workspace>/`, prompt gửi mô hình ngôn ngữ chỉ chứa hồ sơ của workspace hiện tại.

## 5. Nạp dữ liệu và lấy ngữ cảnh

Nạp chạy một lần cho mỗi phiên bản dataset, ba bước độc lập: đăng ký nguồn, lập hồ sơ, vật chất hóa.

| Lưu | Dùng để |
|---|---|
| File gốc chuyển sang Parquet trên volume | DuckDB đọc lại các phiên sau |
| `sources`, `datasets`, `dataset_versions` | Nguồn (kind, cấu hình), tên, hồ sơ tại thời điểm, đường dẫn và hash nếu là file |
| `columns`: tên, kiểu, vai trò (chiều, độ đo, thời gian), min, max, null, giá trị mẫu, mô tả và bí danh | Ghép vào prompt |
| `column_values`: giá trị phân loại của cột ít giá trị, kèm bí danh | Ánh xạ "miền Nam" sang "South" |

Mô tả và bí danh do mô hình ngôn ngữ viết một lần lúc nạp. Lúc hỏi, ngữ cảnh lấy bằng ba truy vấn Postgres:

| Tầng | Nguồn | Cách lấy |
|---|---|---|
| Schema context | `columns` | Cột có tên, mô tả, bí danh khớp từ khóa; dataset nhỏ thì đưa cả schema |
| Value mapping | `column_values` | `unaccent` bỏ dấu, `pg_trgm` chịu lỗi gõ |
| Few-shot | `queries` đã đánh dấu đúng | Từ khóa trùng |

Cột `embedding` có sẵn trên ba bảng; trait sẽ có thêm `embed`. Khi so chuỗi không đủ (cách diễn đạt không lường trước,
nguồn nhiều bảng), bật tìm theo vector trong cùng Postgres, gộp với so chuỗi thành một danh sách ứng viên.

## 6. Engine không biết trước schema

Mọi thứ phụ thuộc cột được suy ra lúc nạp, không ghi cứng:

| Phụ thuộc schema | Cách làm |
|---|---|
| Prompt | Ghép từ `columns` và `column_values` |
| Phân quyền theo dòng | `rls_policies` theo dataset: role, cột, giá trị |
| Câu "tại sao" | Vai trò cột từ hồ sơ: chuỗi ít giá trị là chiều, số là độ đo, ngày là thời gian |
| Câu hỏi gợi ý | Sinh từ hồ sơ lúc nạp, lưu lại |
| Biểu đồ | Theo kiểu kết quả: một chiều một độ đo thì cột, có thời gian thì đường |

Mô hình ngôn ngữ sinh SQL trên tên bảng logic. Engine resolve tên logic thành nguồn vật lý: file Parquet, hoặc bảng
trong database đã gắn. Nhờ đó nguồn database (Postgres, MySQL, SQLite qua extension của DuckDB) thêm vào mà không đổi
guard, prompt hay lịch sử.

## 7. Dữ liệu thay đổi

Mỗi lần nạp file là một phiên bản, không ghi đè. `queries` trỏ tới `version_id`.

- Hash SHA-256: trùng thì bỏ qua, khác thì phiên bản mới.
- So hồ sơ cũ và mới cho ra khác biệt schema: cột mất, thêm, đổi kiểu, giá trị phân loại mới. Hiện cho người dùng
  trước khi chuyển phiên bản hiện hành; cảnh báo nếu chính sách phân quyền trỏ tới giá trị không còn.
- Nguồn sống: phiên bản là bản chụp hồ sơ tại thời điểm, làm mới theo lịch hoặc khi bấm.

## 8. API

REST và JSON. OpenAPI sinh từ code bằng `utoipa`, phục vụ ở `/openapi.json` và Swagger UI ở `/docs`. Frontend sinh
kiểu TypeScript từ spec. `openapi.json` commit vào repo, CI so lại để hai bên không lệch. Tài liệu API đang viết ([#56](https://github.com/pgiahuy/xdi-platform/issues/56)).

## 9. Hướng phát triển

Bản hiện tại nhắm dự án cỡ nhỏ và vừa, một instance. Các giới hạn và đường nâng cấp:

| Giới hạn | Nguyên nhân | Đường nâng cấp |
|---|---|---|
| Một instance backend cho phần phân tích | DuckDB nhúng trong tiến trình; dữ liệu ứng dụng đã ở Postgres nên không vướng | Tách engine phân tích thành service riêng đọc Parquet trên object storage |
| Session và JWT tự quản | Không có identity provider | OIDC (Zitadel, Keycloak); `auth/` là module riêng |
| Dataset trên đĩa local | Volume docker | S3 hoặc MinIO, chỉ đổi phần nạp file trong `engine/analytics.rs` |
| Một crate | Chưa có gì để tách | Rút engine hoặc guard thành crate trong workspace khi cần dùng lại |
| Không cache, không queue | Chưa có tải | Redis cho cache câu hỏi, queue cho nạp file lớn, cắm vào tầng service |
| Nguồn chỉ là file | | Nối Postgres, MySQL, SQLite qua DuckDB; `sources` và `source_kind` đã có |

Ba điều giữ cho các nâng cấp trên không phải viết lại: state đi qua `AppState`; hợp đồng API là tài liệu và kiểu dữ
liệu; mọi lời gọi ra ngoài đi qua một chỗ để thêm retry, timeout, cache.

## Trạng thái

Cập nhật khi có thay đổi.

| Phần | Trạng thái |
|---|---|
| Khung bốn tầng, `AppState`, `AppError`, cấu hình | Có |
| `/health`, `/version` | Có |
| Trait `LlmProvider` và ba cài đặt | Khung: mock chưa có câu mẫu, Gemini và Ollama chưa gọi thật |
| Route cho dataset, ask, drill, auth | Có route, trả 501 |
| Postgres, DuckDB, guard, giải thích, auth, frontend | Chưa có |
