# Kiến trúc

Tài liệu này mô tả hệ thống như được thiết kế. Mục "Trạng thái" ở cuối ghi phần nào đã có, phần nào chưa.

## 1. Sơ đồ tổng thể

```
                                 TRÌNH DUYỆT
                 React, Vite, Tailwind, shadcn/ui, Recharts
            Bố cục 3 pane: dữ liệu | hỏi, bảng, biểu đồ | bằng chứng
                                      |
                                  REST JSON
                     kiểu TypeScript sinh từ openapi.json
                                      |
=================================== docker compose =============================
                                      |
   +----------------------------------v-----------------------------------+
   |          BACKEND  (Rust, Axum, 1 container, phục vụ cả giao diện)    |
   |                                                                      |
   |  api/       handler, request/response struct, kiểm phiên, OpenAPI    |
   |     |                                                                |
   |  service/   ingest (register, profile, materialize)                  |
   |             ask    (schema context -> LLM -> guard -> run -> explain)|
   |             explain (phân rã chênh lệch, tổng đóng góp = chênh lệch) |
   |     |                                                                |
   |  engine/    store/        sqlx  ----------------> POSTGRES           |
   |             analytics.rs  [DuckDB nhúng, chạy trong tiến trình này,  |
   |                            không container] -----> volume Parquet    |
   |             guard/        sqlparser, whitelist bảng và cột, tiêm RLS |
   |             llm/          trait LlmProvider ----> gemini | ollama | mock
   |  auth/      mật khẩu argon2, mã phiên, vai, demo-login, OAuth tùy chọn|
   +----------------------------------------------------------------------+
            |                              |
   +--------v-----------+        +---------v-------------------+
   |   POSTGRES         |        |  VOLUME /data               |
   |  (1 container)     |        |  <workspace>/<dataset>/     |
   |  workspace_id ở    |        |    <ver>/*.parquet          |
   |  mọi bảng trừ users|        |  samples/ gắn ở /samples:ro |
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

Hai container: `backend`, `postgres`. Giao diện build ra file tĩnh, backend phục vụ cùng địa chỉ với API (`/api`), nên
không cần CORS và cookie đăng nhập đơn giản. DuckDB là thư viện nằm trong backend, không có tiến trình riêng.

## 2. Một câu hỏi đi qua chín bước

```
1. api        kiểm phiên đăng nhập, lấy workspace, role và dataset id
2. store      đọc hồ sơ cột, giá trị phân loại, chính sách phân quyền của role     (Postgres)
3. ask        ghép prompt: schema context + bí danh + câu hỏi mẫu đã đúng
4. llm        LlmProvider::complete -> JSON {terms, assumptions, clarify, sql}, SQL viết trên tên bảng logic
               câu "tại sao" (nhận bằng từ khóa): mô hình điền form JSON, code kiểm form và tự dựng mọi SQL
5. guard      parse AST, chỉ SELECT, whitelist bảng và cột, tiêm WHERE theo phân quyền, giới hạn dòng
6. analytics  resolve tên logic -> Parquet, chạy SQL, trả dòng và thời gian           (DuckDB)
7. explain    nếu là câu "tại sao": phân rã chênh lệch theo chiều, LLM chỉ diễn giải số
8. store      ghi queries: câu hỏi, SQL thô, SQL sau guard, thời gian, version_id
9. api        trả gói: SQL thô, SQL sau guard, thẻ "Đã hiểu là", dòng, gợi ý biểu đồ, cảnh báo, phân rã, timeline
```

Gói trả về là một response JSON, hình dạng ở mục 8.

Mô hình ngôn ngữ chỉ làm ba việc: viết SQL kèm cách nó hiểu từng cụm chữ của câu hỏi (bước 4, câu trả lời là một
object JSON theo schema gửi kèm request), sửa đúng một lần khi SQL chạy lỗi, khi guard báo không có bảng hay cột đó,
hay câu có thể dựng giá trị quá lớn, hoặc khi câu trả lời không đọc được (bước 5, 6), và viết lời cho bảng
phân rã mà code đã tính (bước 7). Nó không nhận các dòng dữ liệu, chỉ nhận siêu dữ liệu của cột và vài
giá trị mẫu của cột phân loại, và không biết gì về phân quyền. Mọi bước đều được ghi lại và trả về cho giao diện hiển thị.

## 3. Ba tầng, phụ thuộc một chiều

```
backend/src/
  main.rs            khởi động: đọc config, mở Postgres và DuckDB, dựng router; hoặc chạy lệnh quản trị (cli.rs),
                     tiến trình con đọc Excel (engine/excel_worker.rs)
  config.rs          biến môi trường
  api/               tầng HTTP: handler, request/response struct, lỗi -> status
  service/           tầng nghiệp vụ: ingest.rs, excel.rs, ask.rs, explain.rs; không biết gì về HTTP
  engine/            tầng hạ tầng: analytics.rs (DuckDB), excel.rs (đọc xlsx), store/ (Postgres, chia theo bảng),
                     guard/ (guard SQL, GUARD.md), llm/
  auth/              mật khẩu, mã phiên, role, demo-login
  error.rs           một enum lỗi chung
```

- `api` chỉ gọi `service`. `service` gọi `engine` và `auth`. Không có chiều ngược.
- Trait duy nhất là `LlmProvider` với hàm `complete`; hàm `embed` thêm khi bật tìm theo nghĩa. Chọn implementation lúc khởi động theo cấu hình:
  `gemini`, `ollama`, hoặc `mock`. Mock trả SQL cố định cho bộ câu hỏi mẫu, dùng khi không có khóa và trong test.
- Postgres và DuckDB không bọc trait. Service nhận struct `Store` và `Analytics` cụ thể.
- Câu SQL của `Store` viết dạng chuỗi với `sqlx::query_as`, không dùng macro `query!` kiểm lúc biên dịch: không cần thư
  mục `.sqlx/` và `sqlx-cli`, build không cần Postgres; đổi lại, mọi câu SQL được kiểm bằng test trên Postgres thật.
- Trạng thái chia sẻ qua một `AppState`: `Store`, `Analytics`, `Arc<dyn LlmProvider>`, config. Không biến toàn cục.
- Test ở tầng service với mock LLM, DuckDB in-memory và Postgres trong CI.

## 4. Hai kho dữ liệu

| Kho | Chứa | Vì sao |
|---|---|---|
| Postgres (sqlx, migrations trong `backend/migrations/`) | `workspaces`, `memberships`, `users`, `sources`, `datasets`, `dataset_versions`, `columns`, `column_values`, `metrics`, `rls_policies`, `queries`, `settings`; mọi bảng có `workspace_id`, trừ `workspaces` và `users` | Nhiều ghi nhỏ, nhiều kết nối, đúng việc của database giao dịch |
| DuckDB nhúng | Không giữ gì bền. Đọc Parquet trên volume theo từng câu SQL | Engine phân tích theo cột, nhận SQL chuẩn, đọc thẳng CSV, Parquet, không thêm tiến trình; file Excel đổi sang CSV trước (mục 5) |

Image Postgres là `pgvector/pgvector`, có sẵn `unaccent`, `pg_trgm`, `vector`.

## 4b. Workspace và phân quyền

Một bản cài phục vụ một tổ chức. Workspace là phòng ban trong tổ chức đó, mỗi workspace có dataset, chỉ số và vai
riêng, vì các phòng thường tính chỉ số khác nhau: phòng kinh doanh tính doanh thu gộp theo ngày đặt, phòng tài chính
tính doanh thu ròng theo ngày giao. Lần thiết lập đầu tạo luôn workspace đầu tiên, người thiết lập là admin của nó.
Không có workspace chung mà mọi tài khoản tự thuộc vào: tài khoản tạo sau chỉ vào được workspace khi được admin thêm,
hoặc tự tạo workspace riêng.

Ba tầng tách riêng:

| Tầng | Câu hỏi | Ví dụ |
|---|---|---|
| Workspace | Người dùng thuộc phòng ban nào | Người phòng kinh doanh không thấy dataset, chỉ số, lịch sử của phòng tài chính, trừ khi được mời |
| Vai trong workspace | Trong workspace, được làm gì | `admin` nạp dữ liệu, quản lý thành viên; `member` xem và hỏi |
| Lọc dòng | Trong dataset, thấy dòng nào | Thành viên có `data_role` là `sales_south` chỉ thấy `region = South`; guard thêm điều kiện vào SQL |

Vai trong workspace (`role`) và vai dữ liệu (`data_role`) là hai cột riêng của `memberships`: một `member` có thể chỉ
thấy miền Nam, một `member` khác thấy toàn bộ. Mọi bảng dùng chung, có cột `workspace_id`. Mọi hàm trong `engine/store/` nhận kiểu `WorkspaceId`, chỉ tạo được từ phiên đăng
nhập đã kiểm, nên quên lọc theo workspace là lỗi biên dịch. Lớp chặn thứ hai nằm trong database: khóa ngoại giữa các bảng
nghiệp vụ gồm cả `workspace_id`, nên một dòng không trỏ được sang dòng của workspace khác. Postgres Row Level Security
sẽ thêm sau làm lớp thứ ba. File Parquet nằm
dưới `/data/<workspace>/`, prompt gửi mô hình ngôn ngữ chỉ chứa hồ sơ của workspace hiện tại.

Đăng nhập: luôn bật, kể cả bản cài chỉ có một người. Lần chạy đầu, khi chưa có tài khoản nào, backend in ra log một mã
thiết lập; ai nhập đúng mã thì tạo được tài khoản quản trị (quản lý tài khoản của cả bản cài) kèm workspace đầu tiên.
Mật khẩu băm bằng Argon2id. Phiên lưu trong Postgres (bảng `sessions` giữ SHA-256 của mã phiên), mã phiên nằm trong
cookie httpOnly. Không dùng JWT: đăng xuất, khóa tài khoản, đổi vai phải có hiệu lực ngay ở request kế tiếp, mà JWT đã
cấp thì không thu hồi được nếu không lưu thêm danh sách phiên.

Quản trị hệ thống tạo tài khoản, khóa, mở tài khoản và đặt lại mật khẩu (sinh mật khẩu tạm hiện một lần, đăng xuất mọi
phiên của người đó). Quản trị duy nhất quên mật khẩu thì chạy trên máy chủ: `xdi-backend reset-password --email <email>`
(trong compose: `docker compose exec backend xdi-backend reset-password --email <email>`); ai chạy được lệnh trên máy
chủ là chủ bản cài.

Trong workspace có hai vai. Người tạo workspace là `admin` của nó; workspace luôn còn ít nhất một `admin` (hạ vai hay
xóa admin cuối cùng bị từ chối, kể cả khi hai admin hạ vai nhau cùng lúc). Admin có tài khoản đã khóa vẫn được tính;
workspace chỉ còn admin như vậy thì quản trị hệ thống chỉ định admin mới.

| Việc | `admin` | `member` |
|---|---|---|
| Xem dataset, hỏi, xem bằng chứng | Có | Có |
| Xem danh sách thành viên | Có | Có |
| Tự rời workspace | Có, nếu còn admin khác | Có |
| Nạp file, tạo phiên bản mới | Có | Không (403) |
| Đổi tên, xóa workspace (xóa cả file Parquet) | Có | Không (403) |
| Thêm thành viên theo email, đổi vai, xóa thành viên | Có | Không (403) |
| Xóa dataset; sửa mô tả cột, bí danh; chốt định nghĩa chỉ số; đánh dấu câu trả lời đúng | Có | Không |
| Lưu câu hỏi, xem lịch sử | Có | Có |

Hai dòng cuối chưa có endpoint. Khi thêm, việc chỉ admin làm được gọi `member.require_admin()` ở đầu service hoặc
handler, kèm test cho cả hai vai.

Quản trị hệ thống (`users.is_system_admin`) là thuộc tính của tài khoản, không phải vai trong workspace. Quản trị hệ
thống quản lý tài khoản, xem danh sách mọi workspace kèm thành viên (`GET /api/admin/workspaces`), nhưng không tự có
quyền vào dữ liệu của workspace mình không thuộc: gọi vào đó nhận 404 như mọi người ngoài. Quản trị hệ thống chỉ định
được admin cho bất kỳ workspace nào, lúc nào cũng được, kể cả chính mình (`POST /api/admin/workspaces/{id}/admins`);
dùng khi workspace mất admin, ví dụ admin cuối nghỉ việc và bị khóa tài khoản. Vì vậy tách quyền giữa quản trị hệ
thống và dữ liệu workspace chỉ là tách mềm: mỗi lần chỉ định ghi một dòng log mức cảnh báo trong log của máy chủ,
chưa lưu vào database.

Thành viên mới phải có tài khoản đang hoạt động, do quản trị hệ thống tạo; admin thêm họ theo email. Vì ai cũng tạo được
workspace của mình, mọi người đăng nhập đều biết được một email có tài khoản trong bản cài hay không, và thêm được người
khác vào workspace mà không cần họ đồng ý (người bị thêm tự rời được). Chấp nhận vì một bản cài phục vụ một tổ chức,
tài khoản là của người trong tổ chức đó.

Bốn vai Owner, Admin, Analyst, Viewer của bản thiết kế đầu được gộp còn hai (migration `0003_two_roles.sql`): hai vai đủ
cho nhóm nhỏ, tách thêm sau này chỉ cần thêm giá trị vào ràng buộc của cột `role` và một dòng trong bảng trên.

## 5. Nạp dữ liệu và lấy ngữ cảnh

Nạp chạy một lần cho mỗi phiên bản dataset, ba bước độc lập: đăng ký nguồn, lập hồ sơ, vật chất hóa.

| Lưu | Dùng để |
|---|---|
| File gốc chuyển sang Parquet trên volume: mỗi phiên bản một thư mục `<DATA_DIR>/<workspace>/<dataset>/<thư mục phiên bản>/`, mỗi file gốc một phần `part-NNNN.parquet` kèm cột `_source_file` ghi tên file gốc | DuckDB đọc cả thư mục như một bảng (`read_parquet` với `union_by_name`); nhiều file cùng cột (mỗi tháng một file) thành một bảng, truy dòng nguồn biết dòng đến từ file nào |
| `sources`, `datasets`, `dataset_versions` | Nguồn (kind, cấu hình), tên, hồ sơ tại thời điểm, đường dẫn và hash nếu là file |
| `columns`: tên, kiểu, vai trò (chiều, độ đo, thời gian), min, max, null, giá trị mẫu, mô tả và bí danh | Ghép vào prompt |
| `column_values`: giá trị phân loại của cột ít giá trị, kèm bí danh | Ánh xạ "miền Nam" sang "South" |

File Excel (.xlsx) đọc bằng crate `calamine` (`engine/excel.rs`): mỗi sheet được chọn ghi thành CSV rồi đi tiếp như
CSV, cột `_source_file` ghi tên sheet. Không dùng extension `excel` của DuckDB vì extension đó phải tải từ Internet lúc
chạy và không liệt kê được sheet.

- Người dùng xem danh sách sheet (`POST .../datasets/sheets`: tên, sheet có ẩn không, tên cột, số dòng ước lượng) rồi
  chọn. Không chọn thì file phải có đúng một sheet đang hiện.
- Các sheet chọn cùng lúc phải cùng cột và gộp thành một dataset. Sheet khác cột thì nạp riêng từng lần, mỗi lần một
  dataset: route nạp luôn trả về một dataset, giao diện gửi một lần nạp cho mỗi nhóm sheet cùng cột.
- Dòng đầu có dữ liệu là dòng tiêu đề. Tên cột bỏ khoảng trắng hai đầu, ô tiêu đề trống lấy chữ cột Excel, tên trùng
  thêm hậu tố, nên tên ở danh sách sheet trùng tên cột của dataset. Ô gộp và tiêu đề lệch dòng chưa hỗ trợ.
- File Excel được đọc trong một tiến trình con (`xdi-backend excel-worker`, `engine/excel_worker.rs`) có trần RAM 2 GB
  và 5 phút CPU, mỗi lúc một tiến trình. calamine tin những con số file tự khai, nên một file vài KB cố ý làm sai có
  thể khiến nó cấp phát hàng trăm GB hay đọc rất lâu; khi đó chỉ tiến trình con dừng, request đó nhận 400, backend vẫn
  phục vụ người khác. Client ngắt kết nối thì tiến trình con bị tắt theo. Trước khi đọc còn kiểm kích thước giải nén
  (tổng tối đa 10 lần `MAX_UPLOAD_MB`, bảng chuỗi dùng chung tối đa 64 MB hoặc `MAX_UPLOAD_MB` nếu nhỏ hơn), số sheet
  (100), độ dài tên sheet (100 ký tự), độ dài một dòng (16 MB) và dung lượng CSV ghi ra; kết quả tiến trình con trả về
  tối đa 8 MB, để báo lỗi rõ thay vì để tiến trình con chạm trần. Một file như vậy vẫn giữ
  lượt đọc Excel tới khi chạm trần, nên người khác nạp Excel lúc đó phải chờ.

Mô tả và bí danh do mô hình ngôn ngữ viết một lần lúc nạp. Lúc hỏi, ngữ cảnh lấy từ ba nguồn trong Postgres:

| Tầng | Nguồn | Cách lấy |
|---|---|---|
| Schema context | `columns` | Câu hỏi tách thành cụm 1 đến 5 âm tiết; cụm bằng tên hay bí danh cột chọn cột đó, so có dấu khi cả hai bên có dấu, sau khi bỏ dấu khi một bên không dấu (cột `khu_vuc` khớp "khu vực"). Dataset tới 60 cột thì đưa cả schema, nhiều hơn thì cột khớp một cụm cộng cột thời gian. Một cụm khớp tên của nhiều cột với điểm chênh dưới 0.1 thì hỏi lại (id lựa chọn `column:<cụm đã bỏ dấu>`); chênh nhiều hơn thì chọn cột điểm cao và ghi thành giả định |
| Value mapping | `column_values` | Mỗi cột chiều không nhạy cảm được gửi kèm danh sách giá trị và bí danh, nhiều dòng nhất trước: đủ khi cột tới 50 giá trị, 20 giá trị nhiều dòng nhất khi nhiều hơn, tổng tối đa 400 giá trị (vượt thì cắt cột nhiều giá trị trước). Mô hình tự ánh xạ chữ người dùng sang giá trị trong danh sách ("miền Nam" thành `South`) và trả `{terms, sql}` (#29); endpoint hỏi (#33) báo `filter_value_not_found` khi một giá trị trong `terms` không có trong danh sách đủ của cột. Code chỉ thêm gợi ý khi một cụm từ hai âm tiết, không chứa từ chỉ thời gian, số hay từ nối, bằng chính xác một giá trị hay bí danh (dấu thanh kiểu cũ và mới là một). Giá trị tải một lần cho mỗi phiên bản rồi giữ trong bộ nhớ (32 phiên bản, 200 000 chuỗi); tải quá 2 giây thì đi tiếp không có giá trị |
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

REST và JSON. API dữ liệu nằm dưới workspace trong đường dẫn: `/api/workspaces/{slug}/datasets`,
`/api/workspaces/{slug}/datasets/{id}/ask`. Workspace đang làm việc không lưu trong phiên, nên mỗi tab trình duyệt làm
việc độc lập, tab này không đổi workspace của tab kia. Mỗi request kiểm phiên (chưa đăng nhập: 401) và kiểm người dùng
thuộc workspace đó (không thuộc hoặc không có workspace: cùng một lỗi 404, không dò được tên workspace).

OpenAPI 3.1 sinh từ code bằng `utoipa`: mỗi handler khai báo đường dẫn, tham số, request và response bằng
`#[utoipa::path]`, struct request và response derive `ToSchema`. Router đăng ký handler qua `OpenApiRouter` của
`utoipa-axum` (`api/mod.rs`), nên route có trong router thì có trong đặc tả; phần chung (tên API, địa chỉ gốc `/api`,
cookie phiên, kiểu lỗi `{code, message}`) ở `api/openapi.rs`. Mô tả trong đặc tả viết tiếng Anh, vì người đọc là người
dùng API.

Đặc tả phục vụ ở `/api/openapi.json`, Swagger UI ở `/api/docs`; cả hai không cần đăng nhập, gọi thử endpoint thì cần
cookie phiên như giao diện. Swagger UI kèm sẵn trong binary (feature `vendored` của `utoipa-swagger-ui`), build không
tải gì từ Internet; thông báo giấy phép của nó ở `/api/docs/licenses.txt` (LICENSES.md). Công khai đặc tả không lộ gì mới:
mọi đường dẫn đã có trong file JavaScript của giao diện, vốn tải được khi chưa đăng nhập, và đặc tả không chứa bí mật.
Swagger UI chạy cùng địa chỉ với cookie phiên (httpOnly, SameSite=Lax), nên một lỗi XSS của Swagger UI sẽ gọi được API
bằng quyền người đang mở trang; chấp nhận ở giai đoạn này. Tham số `?url=` của Swagger UI bị bỏ qua (cấu hình cố định),
nên không nạp được đặc tả từ nơi khác.

Hợp đồng nằm trong repo:

| File | Sinh từ | Kiểm lệch |
|---|---|---|
| `openapi.json` (gốc repo) | `xdi-backend openapi`, không cần database | Test `openapi_json_is_up_to_date` trong `make check-fast` (CI job backend, hook pre-push) |
| `frontend/src/api/schema.ts` | `openapi.json`, bằng `openapi-typescript` (`frontend/scripts/api-types.mjs`) | `npm run api:check` trong `make check-frontend` (CI job frontend) |
| `frontend/src/api/examples/ask.json` | `xdi-backend ask-examples` (`service/ask_examples.rs`), không cần database | Test `ask_examples_json_is_up_to_date` trong `make check-fast` |

Đổi API: sửa handler hoặc struct, chạy `make openapi`, commit cả ba file. Hook trong `frontend/src/api` lấy kiểu từ
`schema.ts`, không khai lại. Khái niệm chung (phiên, workspace, vai) và bảng mã lỗi: [API.md](API.md).

### Hỏi dữ liệu

`POST /api/workspaces/{slug}/datasets/{id}/ask` nhận `{question, choices}` và trả một `AskResponse`, JSON một lần,
không SSE: giao diện gọi bằng `useMutation` và chờ đủ gói. Kiểu nằm ở `service/ask_contract.rs` chứ không ở `api/`, vì
service dựng response mà không được gọi ngược lên api; nghĩa từng trường ghi trong đặc tả. `service/ask.rs` ghép các
bước của mục 2 cho câu thường ([#33](https://github.com/pgiahuy/xdi-platform/issues/33)). Câu "tại sao" (`run_why`, #38)
và thẻ "Đã hiểu là" (`build_understanding`, #35) đã có hàm nhưng `ask` chưa gọi; cảnh báo kiểm hợp lý (#36) và gợi ý
biểu đồ chưa có. Các trường đó là null hay rỗng, câu "tại sao" đi như câu thường.

| Điểm | Chọn | Vì sao |
|---|---|---|
| Câu không trả lời được | Vẫn 200, `status` trong body: `answered`, `needs_choice` (câu mơ hồ), `rejected` (guard từ chối), `failed` (SQL chạy lỗi cả sau vòng sửa, mô hình lỗi trong vòng sửa, câu trả lời của mô hình không đọc được cả sau vòng sửa, hoặc câu chạy quá thời gian hay quá bộ nhớ), kèm `message` tiếng Việt. Mã lỗi chỉ cho request sai (400), quyền (401, 404), dịch vụ mô hình lỗi (502) hay chưa cấu hình (503) khi chưa có SQL nào | Các trường hợp này là kết quả bình thường, phải hiện cùng bằng chứng: SQL bị từ chối, các bước, lựa chọn; `ErrorBody` chỉ có `code` và `message`. `useMutation` coi mã lỗi là lỗi, giao diện sẽ phải gỡ dữ liệu ra khỏi lỗi. Câu bị từ chối và câu lỗi vẫn ghi vào `queries` (status `rejected`, `error`) nên có `query_id`. Câu mơ hồ không ghi; nó có thể đến trước khi gọi mô hình (một từ khớp nhiều cột) hoặc sau (câu "tại sao" không xác định được hai lát cắt, [#38](https://github.com/pgiahuy/xdi-platform/issues/38)) |
| Giá trị ô | Mỗi cột có `kind`, ô là giá trị JSON theo `kind`, quyết định theo kiểu cột chứ không theo từng giá trị. Số nguyên tới 32 bit, FLOAT, DOUBLE: số JSON (NaN, vô cực thành null). BIGINT, HUGEINT, DECIMAL: chuỗi chữ số thập phân. Ngày `YYYY-MM-DD`, giờ ISO 8601 (TIMESTAMPTZ đổi sang UTC, có `Z`). NULL: null | Số JavaScript chỉ giữ đúng số nguyên tới 2^53 và làm tròn số thập phân, mà con số là bằng chứng. Cùng một cột luôn một dạng, giao diện đọc theo `kind` thay vì đoán từng ô. `COUNT` trả BIGINT nên số đếm cũng là chuỗi; vẽ biểu đồ hay sắp xếp thì đổi bằng `Number()` |
| Cột | Tên cột trong `result.columns` là duy nhất: DuckDB trả tên trùng nguyên văn, nên backend đổi từ trái sang phải, tên đã dùng nhận hậu tố `_2`, `_3`... nhỏ nhất mà không cột nào có (`a, a, a_2` thành `a, a_3, a_2`), bằng `build_unique_names` trong `service/ask_contract.rs`, drill dùng lại đúng hàm này; `chart`, tiêu đề CSV và khóa truy dòng nguồn trỏ cột bằng tên. Mỗi cột có `role`: `dimension`, `time`, `measure`, `other`, lấy từ cây cú pháp | Tên đọc được hơn chỉ số và đủ khi đã duy nhất. `role` cho giao diện biết ô nào bấm được (độ đo) và khóa của dòng (các cột `dimension`, `time`) |
| Số dòng | `rows` là mảng theo thứ tự cột, tối đa 1000 dòng đầu. `total_rows` là số dòng SQL sau guard trả về; `truncated` khi gửi ít hơn `total_rows`; `limit_reached` khi `total_rows` chạm trần số dòng của guard, dù guard thêm LIMIT hay hạ LIMIT có sẵn (kết quả đầy đủ có thể còn nữa) | Mảng gọn hơn object theo tên cột. 1000 dòng đủ cho bảng và biểu đồ của kết quả đã gộp. Giao diện của v0.1.0 phân trang trong các dòng đã gửi; khi `truncated` ghi rõ "1000 trên N dòng", khi `limit_reached` ghi "ít nhất N dòng" để không nói sai tổng. Tải CSV lấy đúng các dòng đã gửi, kèm ghi chú khi bị cắt. Tải đủ mọi dòng (endpoint riêng theo `query_id`, chạy lại SQL sau guard trên phiên bản đã lưu) làm sau. Trần của guard là `ASK_MAX_ROWS` (mặc định 100 000) và hiện ngay trong SQL |
| Thẻ "Đã hiểu là" | `understanding`: `measures`, `filters`, `group_by`, `order_by`, `limit`, `time_range`, mỗi mục có nhãn và đoạn SQL nó lấy ra; `filters` gồm cả điều kiện phân quyền guard thêm vào, nhãn nói rõ; `assumptions` là danh sách `Choice` đã chọn | [#35](https://github.com/pgiahuy/xdi-platform/issues/35) sinh từ cây cú pháp và kiểm từng mục khớp câu đã chạy; [#50](https://github.com/pgiahuy/xdi-platform/issues/50) vẽ chip từ `Choice`. Người dùng biết mình chỉ thấy một phần dữ liệu |
| Đổi giả định, trả lời câu hỏi lại | Response nào cũng có `choices`: mọi lựa chọn đã áp dụng, kể cả mặc định và câu hỏi lại đã trả lời, cùng dạng `[{choice_id, option_id}]` của request. Hỏi lại thì gửi cùng câu hỏi kèm `choices` của response gần nhất, thay hoặc thêm mục vừa chọn. Người dùng sửa chữ câu hỏi thì đó là câu mới, giao diện gửi `choices` rỗng. Câu mơ hồ trả `clarifications`, cùng kiểu `Choice` nhưng chưa chọn. Id của lựa chọn ổn định cho cùng câu hỏi trên cùng phiên bản dữ liệu: đặt từ loại lựa chọn và đối tượng của nó (một từ và các cột ứng viên, kỳ để so sánh), như `region_column`, `base_period` trong mẫu, không đặt từ chữ của SQL. Id không nhận ra thì bỏ qua, không có trong `choices` trả về, kèm cảnh báo `choice_not_applied`: giả định dùng mặc định, câu hỏi lại thì được hỏi lại. Nếu `needs_choice` quay lại đúng câu vừa trả lời kèm cảnh báo đó, giao diện hiện `message` thay vì hỏi lại y nguyên | Backend không giữ trạng thái giữa hai lần hỏi, và giao diện chỉ cần response gần nhất: thiếu một lựa chọn đã trả lời thì backend sẽ hỏi lại chỗ đó mãi. Id sinh từ SQL sẽ đổi khi mô hình viết SQL khác ở lần hỏi sau, và câu "tại sao" hỏi lại sau khi gọi mô hình sẽ lặp mãi. Bỏ qua thay vì 400 vì một giả định có thể thật sự không còn khi SQL đổi; trả 400 thì người dùng kẹt. Câu đã sửa mà giữ lựa chọn cũ thì lựa chọn hoặc áp dụng ngầm, hoặc sinh cảnh báo khó hiểu |
| Phân rã | `explanation`, chỉ có ở câu "tại sao": hai lát cắt (`base`, `compare`, mỗi lát có nhãn, điều kiện SQL, giá trị), chênh lệch, mỗi chiều một bảng đóng góp của từng nhóm kèm SQL đã chạy sau guard (chiều giải thích tốt nhất trước), tách lượng và giá kèm SQL, đoạn tóm tắt và cờ `summary_by_model`. Không phân rã được thì cảnh báo `explanation_unavailable` nói lý do. Số là chuỗi chữ số thập phân; độ đo đổi sang DECIMAL ở từng dòng nguồn trước khi cộng, không làm tròn từng nhóm. Phần lượng và giá có một phép chia (giá = độ đo / lượng): giá làm tròn tới số chữ số cố định (#40 chọn) và là chỗ làm tròn duy nhất, `volume_effect` tính đúng từ giá đã làm tròn, `price_effect = change - volume_effect` nhận phần lệch do làm tròn. Bảng kết quả cộng theo kiểu cột (DOUBLE là số thực), nên giá trị hai lát cắt có thể lệch ô của bảng ở chữ số cuối; con số của phân rã lấy từ `explanation` | Mọi con số kiểm được trên giao diện bằng SQL đứng sau nó ([#52](https://github.com/pgiahuy/xdi-platform/issues/52)). Phép cộng trên DECIMAL là đúng, nên tổng đóng góp bằng đúng chênh lệch ([#39](https://github.com/pgiahuy/xdi-platform/issues/39), [#40](https://github.com/pgiahuy/xdi-platform/issues/40)) và [#41](https://github.com/pgiahuy/xdi-platform/issues/41) so số trong lời với đúng các chuỗi này |
| Biểu đồ | `chart`: `bar` hoặc `line`, cột trục x và các cột vẽ; null thì chỉ hiện bảng | Luật ở mục 6 |
| Các bước | `steps`: tên bước, thời gian (ms), thành công hay không, chi tiết. `explain` (code tính phân rã) tách khỏi `narrate` (mô hình viết lời); `repair` là lần mô hình sửa SQL, tối đa một lần: sau `run` lỗi, sau `guard` từ chối vì không có bảng hay cột đó hay vì câu có thể dựng giá trị quá lớn, hoặc khi câu trả lời đầu của mô hình không đọc được (khi đó `repair` là bước đầu tiên; guard từ chối vì lý do khác, câu quá giờ hay quá bộ nhớ thì không sửa). Bước lỗi không nhất thiết dừng câu hỏi (`run` hay `guard` lỗi có thể theo sau là `repair`, `narrate` lỗi thì dùng câu khuôn); `status` cho biết câu hỏi có dừng không, và gọi theo bước đã dừng nó: guard thì `rejected`, chạy hay vòng sửa thì `failed` | Pane bằng chứng hiện timeline, thấy được mô hình chỉ tham gia ở đâu; giao diện tự đặt nhãn cho từng tên bước |

Truy dòng nguồn ([#34](https://github.com/pgiahuy/xdi-platform/issues/34)): `POST .../datasets/{id}/drill`, hợp đồng
ở `service/drill_contract.rs`. Request gửi `query_id` cùng khóa của chỗ được bấm, không bao giờ gửi SQL. Bấm một ô của
bảng: `cell`, giá trị các cột `dimension` và `time` của dòng đó, theo tên trong `result.columns`. Bấm một dòng phân rã:
`breakdown`, lát cắt (`base` hay `compare`), cột (tên trong dataset, không phải tên trong `result.columns`) và giá trị
của nhóm. Hai loại tên này khác nhau nên ở hai trường riêng. Client không đưa được SQL vào, và nạp phiên bản mới không
làm drill ra số của phiên bản khác.

| Điểm | Chọn | Vì sao |
|---|---|---|
| Dựng câu của ô | Từ `sql_guarded` đọc bằng `QueryShape`: giữ FROM (kể cả hộp lọc phân quyền của người đã hỏi) và WHERE, thêm `(<biểu thức của cột khóa>) = <giá trị>` cho mỗi cột khóa (`IS NULL` khi ô trống), literal dựng trong cây; bỏ GROUP BY, ORDER BY, LIMIT; `SELECT *` có `LIMIT`, `OFFSET`, cộng một câu `count(*)`. Tên cột khóa so với tên DuckDB trả cho câu gốc (chạy lại câu đó với `LIMIT 0`) | Đó là câu đã ra con số người dùng thấy. DuckDB tự đặt tên cột (`SELECT market` trên cột `Market` ra tên `Market`), đoán từ cây thì lệch. Khóa phải đủ mọi cột nhóm, thiếu thì trang gồm dòng của nhiều ô |
| Quyền | Guard với vai dữ liệu của người đang bấm, chạy trên `version_id` của câu hỏi | Câu do admin hỏi, thành viên bị lọc bấm vào chỉ thấy phần của mình; hộp lọc của người hỏi giữ lại nên không ai thấy dòng ngoài con số |
| Tổng khớp ô | Đúng khi độ đo là `SUM` không điều kiện; `SUM(CASE ...)`, `FILTER` thì trả mọi dòng của nhóm | Điều kiện riêng của từng độ đo không áp được cho cả dòng |
| Dạng chưa truy được | 400 "Câu hỏi này chưa truy được dòng nguồn: <lý do>": phép tập hợp, nhiều bảng, bảng tạm, câu con ở FROM, HAVING, hàm cửa sổ, ROLLUP, CUBE, GROUPING SETS, `*`, câu không có độ đo, khóa nhóm không có trong SELECT | Không trả dòng có thể sai |
| Phân trang | Không ORDER BY, dựa vào thứ tự đọc của DuckDB (`preserve_insertion_order` mặc định bật); mặc định 100 dòng, tối đa 1000 | Sắp xếp cả nhóm tốn hơn mà không thêm nghĩa; có test hai trang không trùng dòng |
| Dòng phân rã | Chưa truy được, trả 400 | Cần điều kiện hai lát cắt và bộ lọc của form lưu cùng dòng `queries`: cột `explanation` đã có (migration 0006) nhưng chỉ được ghi khi `ask` gọi `run_why` cho câu "tại sao" (#38) |

`service/ask_examples.rs` dựng năm response mẫu trên bảng `dataco`: câu "tại sao" có phân rã (`why`), câu một chiều
một độ đo (`simple`), câu mơ hồ (`needs_choice`), câu bị guard từ chối (`rejected`), câu chạy lỗi cả sau vòng sửa
(`failed`). Mẫu là example của response 200 trong đặc tả, và nằm ở `frontend/src/api/examples/ask.json` để giao diện
dựng màn hỏi và pane bằng chứng cả khi không có mô hình ngôn ngữ (công tắc "Dữ liệu mẫu" của `npm run dev`). Test kiểm file khớp code và mẫu khớp schema trong đặc tả, vì
TypeScript không kiểm được file JSON theo enum. Số trong mẫu, kể cả thời gian các bước, là số minh họa, không phải số
tính hay đo trên dữ liệu thật.

## 9. Hướng phát triển

Bản hiện tại nhắm dự án cỡ nhỏ và vừa, một instance. Các giới hạn và đường nâng cấp:

| Giới hạn | Nguyên nhân | Đường nâng cấp |
|---|---|---|
| Một instance backend cho phần phân tích | DuckDB nhúng trong tiến trình; dữ liệu ứng dụng đã ở Postgres nên không vướng | Tách engine phân tích thành service riêng đọc Parquet trên object storage |
| Phiên đăng nhập tự quản trong Postgres | Không có identity provider | OIDC (Zitadel, Keycloak); `auth/` là module riêng |
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
| `/api/health`, `/api/version`; mọi API dưới `/api`, đường dẫn API sai trả 404 JSON | Có |
| OpenAPI (mục 8): đặc tả sinh từ code ở `/api/openapi.json`, Swagger UI ở `/api/docs`, `openapi.json` và kiểu TypeScript `frontend/src/api/schema.ts` trong repo, test và CI báo khi lệch code | Có |
| Trait `LlmProvider` và ba cài đặt; `complete` nhận loại việc, câu hỏi gốc, prompt và JSON Schema của câu trả lời nếu cần | Mock trả lời 14 câu mẫu trên DataCo (bảng logic `dataco`): một câu cố ý đọc sai định dạng ngày và có câu sửa để thử vòng sửa, ba câu để thử nhánh bị từ chối, chạy lỗi cả sau khi sửa và giá trị lọc không có trong cột; cùng hồ sơ viết sẵn của `dataco` (`engine/llm/mock_answers.json`); câu trả lời sinh SQL là object `{terms, assumptions, clarify, sql}` (`service/ask_reply.rs`), Gemini nhận schema qua `responseJsonSchema`, Ollama qua `format`; Gemini (`generateContent`) và Ollama (`/api/generate`) gọi qua HTTP, có timeout theo cấu hình, không theo chuyển hướng; lỗi trả lời hướng dẫn tiếng Việt với mã 502 (`upstream`, dịch vụ mô hình lỗi) hoặc 503 (`unavailable`, máy chủ thiếu cấu hình); khóa Gemini không vào log; hồ sơ ngữ nghĩa (#23) gọi `complete` sau mỗi lần nạp, kèm JSON Schema của câu trả lời (`service/semantic_schema.rs`); endpoint hỏi (#33) gọi `complete` để sinh SQL và sửa; chưa thử với dịch vụ thật |
| Nạp dataset: `POST /api/workspaces/{slug}/datasets` (multipart, một hoặc nhiều file CSV hoặc Parquet, hoặc một file Excel kèm sheet đã chọn), mỗi lần nạp một phiên bản, bộ file trùng thì bỏ qua, giới hạn `MAX_UPLOAD_MB`; liệt kê sheet của file Excel ở `POST .../datasets/sheets` | Có; sheet Excel khác cột nạp riêng từng lần |
| Route ask, drill | Có route dưới `/api/workspaces/{slug}/`, kiểm đăng nhập và thành viên. Ask chạy luồng hỏi câu thường (`service/ask.rs`, #33): câu mơ hồ hỏi lại trước khi gọi mô hình, câu trả lời, câu bị từ chối và câu lỗi ghi vào `queries`; drill truy được dòng nguồn của một ô (mục 8), dòng phân rã trả 400 tới khi câu "tại sao" ghi form vào `queries.explanation`. Hợp đồng request và response của ask (mục 8) có trong đặc tả, kèm response mẫu cho giao diện |
| Trang hỏi dữ liệu (`pages/AskPage.tsx`): chọn dataset kèm dòng báo hồ sơ đang lập hoặc lỗi, ô hỏi, câu gợi ý của dataset (tự tải lại khi hồ sơ đang lập), thẻ "Đã hiểu là" với giả định đổi được, lời giải thích của câu "tại sao", câu hỏi lại, lời báo khi bị từ chối, chạy lỗi, mô hình lỗi; bảng kết quả phân trang, tải CSV các dòng đã nhận, biểu đồ cột hoặc đường theo gợi ý của backend (Recharts, tải riêng khi có biểu đồ) | Có, dựng trên hợp đồng; trang gọi route ask thật; `npm run dev` có công tắc dữ liệu mẫu |
| Hồ sơ cột lớp cấu trúc (`service/profile.rs`, `engine/profile.rs`, [DATA.md](DATA.md)): kiểu, số ô trống, số giá trị khác nhau, min, max, vai trò, định dạng của cột ngày dạng chữ, tần suất giá trị của cột tối đa 200 giá trị; đánh dấu cột nhạy cảm theo luật tên cột và dạng giá trị trong cùng giao dịch ghi hồ sơ; lập lúc nạp, phiên bản cũ lập nền và quét đánh dấu lại lúc khởi động | Có; chưa có API đọc |
| Hồ sơ cột lớp ngữ nghĩa (`service/semantic.rs`, [DATA.md](DATA.md)): sau khi nạp trả 201, mô hình đề xuất mô tả, bí danh cột và giá trị, câu hỏi gợi ý, cột nhạy cảm cần đánh dấu thêm (chỉ chiều bật), sửa vai trò trong phạm vi hẹp (measure số nguyên thành key, hoặc thành dimension khi tối đa 200 giá trị; dimension thành text); chạy nền một lượt mỗi lúc, lỗi giữ `structure` và đánh dấu đã thử; thử lại khi khởi động cho phiên bản chưa thử với provider đang cấu hình và khi nạp lại cùng bộ file; prompt chỉ chứa siêu dữ liệu và giá trị (có trần kích thước) của cột dimension không nhạy cảm; câu gợi ý và `current_profile_status` trả ra trong API dataset | Có, kèm test; chưa thử với mô hình thật; chưa có màn xem, sửa |
| Gói khái niệm ngành (`data/domains/*.json`, `engine/domains.rs`, [DOMAINS.md](DOMAINS.md)): `build.rs` nhúng mọi file vào binary, hàm liệt kê mọi gói và tìm theo id | Có gói bán lẻ và chuỗi cung ứng (`retail`); chưa chọn gói cho dataset lúc nạp, chưa nối vào prompt (#29) |
| Schema Postgres (migration `0001_init.sql`), kết nối và tự migrate lúc khởi động | Có |
| Tầng lưu trữ theo workspace: nguồn, dataset, phiên bản, lịch sử câu hỏi; kiểu `WorkspaceId` | Có; liệt kê và xem dataset đã dùng |
| Workspace: liệt kê, tạo mới (người tạo là admin), đổi tên, xóa; mọi API dữ liệu kiểm thành viên; `WorkspaceId` chỉ tạo từ dòng memberships của người đăng nhập | Có |
| Vai `admin` và `member` (mục 4b): quản lý thành viên, luật còn ít nhất một admin, quản trị hệ thống xem mọi workspace và chỉ định admin | Có |
| Khung giao diện React và Vite; bố cục chung: thanh bên, đổi workspace, hiện vai trong workspace đang mở, tài khoản và đăng xuất, dùng được ở bề ngang điện thoại; trang làm việc dưới `/w/<workspace>`, trang quản trị hệ thống và trang tài khoản ngoài workspace (`/admin/...`, `/account`) | Có |
| Backend phục vụ giao diện cùng cổng với API (`STATIC_DIR`), một image chứa cả hai | Có |
| Chạy thật: `docker-compose.prod.yml` (Postgres không mở cổng, mật khẩu bắt buộc, backend nghe `127.0.0.1`), cookie `Secure` theo `PUBLIC_URL`, header bảo mật trên mọi response (`api/security_headers.rs`), `make backup`, `make restore`, mẫu Caddy và nginx | Có; role Postgres riêng không phải superuser cho RLS chưa có |
| Tài khoản: mã thiết lập in ra log lần đầu, tạo tài khoản quản trị kèm workspace đầu tiên, đăng nhập, đăng xuất, đổi mật khẩu; quản trị hệ thống tạo, khóa, mở tài khoản, đặt lại mật khẩu; lệnh `reset-password`, `list-admins` (API, phiên lưu trong Postgres) | Có; giao diện có màn thiết lập lần đầu, đăng nhập, đổi mật khẩu, quản lý tài khoản của quản trị hệ thống |
| DuckDB nhúng: đổi CSV (UTF-8; file không phải UTF-8 đổi sang UTF-8 theo windows-1252, gồm cả latin-1, trừ file UTF-8 có byte hỏng thì báo lỗi kèm số dòng; `engine/csv_encoding.rs`) và Parquet sang Parquet, gộp nhiều file cùng cột kèm cột `_source_file` (`engine/analytics.rs`); sheet Excel đổi sang CSV bằng `calamine` (`engine/excel.rs`) | Có |
| Guard SQL (`engine/guard/`, [GUARD.md](GUARD.md)): một câu `SELECT` chỉ đọc, danh sách bảng, hàm, cột được phép, lọc dòng theo chính sách service truyền vào, giới hạn dòng, bản `shown` và bản `runnable` đọc Parquet | Có, kèm test; endpoint hỏi (#33) gọi guard; drill (#34) đọc chính sách lọc dòng theo vai của người bấm, luồng hỏi chưa đọc (#45) |
| Chạy SQL và vòng sửa (`engine/query.rs`, `service/ask_run.rs`, `service/ask_result.rs`): chạy bản `runnable` trên DuckDB, hai câu cùng lúc, dừng câu quá `QUERY_TIMEOUT_SECS` (mặc định 30 giây) và trần 1 GB cho phần bộ nhớ DuckDB quản lý; cột kiểu DuckDB chưa đọc được (TIME_NS, TIMETZ, BIT, BIGNUM, VARIANT, GEOMETRY, kiểu không rõ) báo lỗi để mô hình đổi sang VARCHAR; kết quả thành `ResultTable` (ô theo `kind`, 1000 dòng đầu, `truncated`, `limit_reached`); lỗi DuckDB, sai tên bảng, cột hay câu quá lớn theo luật của guard thì mô hình sửa một lần; lời báo lỗi không chứa đường dẫn file | Có, kèm test; endpoint hỏi (#33) gọi; vai trò cột là `other` tới khi endpoint hỏi gọi `build_column_roles` (#35); trần 1 GB và hai câu cùng lúc chưa đo lại trên DataCo; hai giới hạn không chặn được biểu thức hằng DuckDB tính lúc chuẩn bị câu; guard thu hẹp bằng luật kích thước hàm, tối đa 4 tầng câu, 20 mảnh ghép, chuỗi hằng 1000 byte, 200 cột mỗi SELECT, 4 nguồn mỗi FROM, 10 nhánh UNION (ca một dòng kết quả đo dưới 270 MB); câu nhân theo số dòng (cả `UNION ALL` 10 nhánh) vẫn lên khoảng 1,4 đến 1,9 GB, bảo đảm thật cần tiến trình con (#119; [GUARD.md](GUARD.md), mục Hạn chế đã biết) |
| Ngữ cảnh câu hỏi (`service/ask_context.rs`, `ask_phrases.rs`, `ask_match.rs`, `ask_values.rs`): cột gửi cho mô hình, danh sách giá trị của cột chiều kèm bí danh, gợi ý khớp chính xác, câu hỏi lại khi một cụm khớp tên nhiều cột; đọc hồ sơ của đúng phiên bản trong workspace người hỏi, theo hai tầng đầu của mục 5; cột nhạy cảm không kèm giá trị | Có hàm kèm test trên Postgres, gồm hồ sơ mẫu của DataCo; prompt (#29) và endpoint hỏi (#33) dùng; khớp gần, chịu gõ sai và tìm theo vector chưa làm |
| Giải thích câu "tại sao", phần toán (`service/explain.rs`): đóng góp từng nhóm bằng số thập phân chính xác, xếp hạng chiều theo Adtributor, tách lượng và giá với giá hai chữ số làm tròn nửa xa số 0 | Có hàm thuần kèm test |
| Đường câu "tại sao" (`service/explain_run.rs`, `explain_form.rs`, `explain_check.rs`, `explain_sql.rs`, `explain_breakdown.rs`): nhận câu bằng từ khóa, mô hình điền form (độ đo hoặc chỉ số, hai kỳ tháng, quý, năm hoặc hai nhóm, bộ lọc `=`), code kiểm form với hồ sơ, `column_values` và `metrics`, sửa form một lần theo lỗi từng trường, sai lần hai thì chạy như câu thường kèm `explanation_unavailable`; code dựng bảng tổng theo kỳ, điều kiện hai lát cắt, SQL từng chiều (tối đa 8 chiều tới 50 giá trị) và SQL lượng (một cột `key` lặp lại), mọi câu qua guard có lọc dòng theo vai dữ liệu | Có, kèm test trên Parquet tạm với mock; `run_why` chưa có route gọi (endpoint hỏi của #33 mới có câu thường); chỗ lưu form đã có (`queries.explanation`, migration 0006) nhưng chưa ghi; lượng chưa lấy từ `metrics` vì bảng chưa có trường đánh dấu chỉ số lượng |
| Diễn giải bảng phân rã bằng lời (`service/ask_narrate.rs`, phần chữ ở `service/explain.rs`): mô hình viết đoạn tóm tắt từ `Explanation`, code kiểm mọi số trong đoạn có nguyên văn trong bảng (số kèm `%`, `‰`, "phần trăm", "pct", "gấp", "x", "lần" luôn sai; nhãn và giá trị nhóm chép nguyên văn được miễn); mô hình lỗi, quá giờ hay sai số thì dùng câu khuôn của backend, câu hỏi vẫn `answered` và bước `narrate` có `ok = false` kèm lý do | Có, kèm test; mock trả lời câu "tại sao" mẫu; `run_why` gọi sau bước explain; chưa có route, cùng `run_why` |
| Thẻ "Đã hiểu là" và vai trò cột kết quả (`engine/sql_shape.rs`, `service/understanding.rs`, `service/understanding_labels.rs`): đọc `sql_guarded` bằng cây cú pháp ra cột kết quả, vế lọc, khóa nhóm, thứ tự, LIMIT, bảng (đọc xuyên hộp phân quyền của guard); thẻ có nhãn tiếng Việt và đoạn SQL nguyên văn cho từng mục, cụm vế thời gian liền nhau đầu tiên thành `time_range`, điều kiện phân quyền nhãn "Phân quyền: ...", giả định là danh sách `Choice` của `service/ask_choices.rs` | Có hàm kèm test trên 15 câu SQL của mock qua guard; endpoint hỏi chưa gọi (#33 trả `understanding` null); câu có CTE, câu con ở FROM chỉ đọc câu ngoài cùng, UNION chỉ có thứ tự và LIMIT, `SELECT *` thì mọi cột là `other`; câu "tại sao" dựng thẻ từ form ở #38 |
| Pane bằng chứng của trang hỏi (`pages/AskEvidence.tsx`, `components/ask/Evidence*.tsx`): bốn tab SQL (trước và sau guard), Phân rã (bảng đóng góp từng chiều, lượng và giá), Dòng nguồn (bấm số của một kỳ trong bảng phân rã), Các bước | Có, dựng trên hợp đồng ask; tab Dòng nguồn gọi route drill với khóa dòng phân rã, backend chưa truy được nên hiện lời báo của backend; `npm run dev` có dòng mẫu; điều kiện phân quyền guard thêm chưa tách riêng trong response nên chưa chỉ ra từng điều kiện |
| Các trang nghiệp vụ | Chưa có |
