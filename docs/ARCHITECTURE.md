# Kiến trúc

Tài liệu này mô tả hệ thống như được thiết kế. Mục "Trạng thái" ở cuối ghi phần nào đã có, phần nào chưa.

## 1. Sơ đồ tổng thể

```
                                 TRÌNH DUYỆT
                 React, Vite, Tailwind, shadcn/ui, Recharts
  Trang hỏi 2 cột: hỏi, bảng, biểu đồ | minh chứng; lịch sử ở thanh bên
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
   |  auth/      mật khẩu argon2, mã phiên, mã thiết lập, WorkspaceId     |
   +----------------------------------------------------------------------+
            |                              |
   +--------v-----------+        +---------v-------------------+
   |   POSTGRES         |        |  VOLUME /data               |
   |  (1 container)     |        |  <workspace>/<dataset>/     |
   |  workspace_id ở    |        |    <ver>/*.parquet          |
   |  bảng nghiệp vụ    |        |  samples/ gắn ở /samples:ro |
   |                    |        +-----------------------------+
   |  workspaces        |
   |  memberships       |                   ngoài compose, tùy chọn
   |  users             |        +--------------------+   +-----------------+
   |  sources           |        |  Gemini API        |   |  Ollama local   |
   |  datasets          |        |  (có key)          |   |  (offline)      |
   |  dataset_relations |        +--------------------+   +-----------------+
   |  dataset_versions  |
   |  columns           |
   |  column_values     |
   |  metrics           |
   |  rls_policies      |
   |  queries, settings |
   |  conversations     |
   |  sessions          |
   +--------------------+
```

Hai container: `backend`, `postgres`. Giao diện build ra file tĩnh, backend phục vụ cùng địa chỉ với API (`/api`), nên
không cần CORS và cookie đăng nhập đơn giản. DuckDB là thư viện nằm trong backend, không có tiến trình riêng.

## 2. Một câu hỏi đi qua chín bước

```
1. api        kiểm phiên đăng nhập, lấy workspace, role và dataset id
2. store      đọc hồ sơ cột, giá trị phân loại, chính sách phân quyền của role     (Postgres)
3. ask        ghép prompt: schema context + bí danh + câu mẫu admin đã duyệt + các lượt trước của cuộc trò chuyện;
               dataset chụp từ database: thêm cột của các bảng liên quan và danh sách quan hệ khóa ngoại
4. llm        phân loại câu hỏi -> JSON {kind, reason}: câu thường hay câu hỏi tìm nguyên nhân (lỗi thì luật từ khóa)
               câu thường: LlmProvider::complete -> JSON {terms, assumptions, clarify, unanswerable, sql}, SQL trên
               tên bảng logic
               câu "tại sao": mô hình điền form JSON, code kiểm form và tự dựng mọi SQL
5. guard      parse AST, chỉ SELECT, whitelist bảng và cột, tiêm WHERE theo phân quyền, giới hạn dòng
6. analytics  resolve tên logic -> Parquet, chạy SQL, trả dòng và thời gian           (DuckDB)
7. explain    câu "tại sao": phân rã chênh lệch theo chiều (cả cột của bảng nối nhiều-một), LLM chỉ viết lời khi so
               hai kỳ;
               câu thường: LLM viết đoạn trả lời ngắn từ tối đa 10 dòng đầu của kết quả; code chọn phần giải thích
               theo hình dạng kết quả, tính số và phân rã khi cần, LLM viết lại lời từ các câu code đã viết
8. store      ghi queries: câu hỏi, SQL thô, SQL sau guard, thời gian, version_id, phiên bản mọi bảng đã đọc
9. api        trả gói: SQL thô, SQL sau guard, thẻ "Hệ thống đã hiểu là", dòng, gợi ý biểu đồ, cảnh báo, phân rã, timeline
```

Gói trả về là một response JSON, hình dạng ở mục 8.

Lúc hỏi, mô hình ngôn ngữ chỉ làm sáu việc:

- Phân loại câu hỏi là câu thường hay câu hỏi tìm nguyên nhân (bước 4, `service/ask/classify.rs`), trước khi viết SQL
  hay điền form. Mô hình lỗi, quá giờ hay trả JSON không đọc được thì luật từ khóa (`build_question_kind`,
  `service/explain/form.rs`) quyết định, câu hỏi vẫn chạy, bước `classify` trong `steps` ghi lý do. Luật từ khóa bỏ
  sót những câu không có cụm nào của nó ("Doanh thu giảm do gì?") và bắt nhầm câu đếm theo một cột tên "lý do"; bộ câu
  thử `eval/classify.json` (`make eval-classify`) đo cả hai cách.
- Viết SQL kèm cách nó hiểu từng cụm chữ của câu hỏi, hoặc điền form với câu "tại sao" (bước 4). Câu trả lời là một
  object JSON theo schema gửi kèm request; prompt có luật của guard sinh từ chính guard.
- Sửa đúng một lần khi SQL chạy lỗi, khi guard từ chối vì bất kỳ lý do nào, khi câu trả lời không đọc được (bước 5,
  6), hoặc khi form câu "tại sao" sai.
- Viết lời cho bảng phân rã mà code đã tính, chỉ khi so hai kỳ (bước 7).
- Viết đoạn trả lời ngắn cho câu thường từ tối đa 10 dòng đầu của kết quả (bước 7, tắt bằng `ANSWER_TEXT=false`).
- Viết lại phần giải thích của câu thường từ câu chính và các ý code đã viết (bước 7, `service/insight/`, tắt bằng
  `INSIGHT_TEXT=false`): một lần gọi thêm mỗi câu; lời được kiểm số, chiều tăng giảm và từ chỉ nguyên nhân, sai thì
  dùng câu của code.

Nó không nhận bảng dữ liệu. Khi phân loại câu hỏi, nó nhận câu hỏi, tên cột kèm bí danh và mô tả, và câu hỏi của các
lượt trước, không có giá trị nào của cột. Khi viết SQL hay điền form, nó nhận siêu dữ liệu của cột và giá trị của cột
phân loại; khi viết lời cho câu "tại sao", nó nhận số đã tính; khi viết đoạn trả lời của câu thường, nó nhận tối đa 10
dòng đầu của kết quả, là dòng gốc khi câu không gộp; khi viết phần giải thích của câu thường, nó nhận các câu code đã
viết (số đã tính, tên nhóm, tên kỳ), không nhận dòng nào. Nó không biết phân quyền của người hỏi (prompt chỉ nói chung rằng
hệ thống tự lọc theo quyền). Mọi bước đều được ghi lại và trả về cho giao diện hiển thị. Phần gửi cho mô hình, chi tiết
từng lúc: [PRIVACY.md](PRIVACY.md).

Chữ cố định của các prompt (luật, khuôn câu) nằm trong `backend/prompts/`, mỗi prompt một file: đọc ở đó để thấy
nguyên văn những gì gửi cho mô hình. Chỗ trống viết `{{tên}}`, code điền phần dữ liệu (dòng cột, danh sách giá trị,
JSON, lượt trước) qua `engine/prompts.rs`; file nhúng vào binary lúc biên dịch, và mỗi prompt có file mẫu so từng byte
trong `backend/src/service/testdata/`. Riêng prompt diễn giải câu "tại sao" còn trong `service/explain/mod.rs`.
Luật viết tiếng Anh vì là phần máy đọc; bản dịch tiếng Việt để cả đội đọc nằm ở `backend/prompts/vi/`, không gửi cho
mô hình, và một test báo lỗi khi bản dịch thiếu hay khác chỗ trống với bản gốc.

## 3. Ba tầng, phụ thuộc một chiều

```
backend/src/
  main.rs            khởi động: đọc config, mở Postgres, dựng AppState và router; hoặc chạy lệnh quản trị (cli.rs),
                     tiến trình con đọc Excel (engine/excel_worker.rs)
  config.rs          biến môi trường
  api/               tầng HTTP: handler, request/response struct, lỗi -> status
  service/           tầng nghiệp vụ: ingest.rs, excel.rs, samples.rs, eval.rs và các module con ask/, explain/,
                     drill/, understanding/, semantic/, conversation/; không biết gì về HTTP
  engine/            tầng hạ tầng: analytics.rs, query.rs (DuckDB), excel.rs (đọc xlsx), store/ (Postgres, chia theo
                     bảng), guard/ (guard SQL, GUARD.md), llm/, prompts.rs (điền chữ của backend/prompts/)
  auth/              mật khẩu, mã phiên, mã thiết lập, vai, kiểu WorkspaceId
  error.rs           một enum lỗi chung
```

- `api` chỉ gọi `service`. `service` gọi `engine` và `auth`. Không có chiều ngược.
- Trait duy nhất là `LlmProvider` với hàm `complete`; hàm `embed` thêm khi bật tìm theo nghĩa. Chọn implementation
  lúc khởi động theo cấu hình: `gemini`, `ollama`, hoặc `mock`. Mock trả câu trả lời viết sẵn cho bộ câu hỏi mẫu, dùng
  khi không có khóa và trong test.
- Postgres và DuckDB không bọc trait. Service nhận struct `Store` cụ thể; DuckDB đi qua hàm của `engine/analytics.rs`
  và `engine/query.rs`, mỗi lần nạp hay chạy câu mở một kết nối DuckDB trong bộ nhớ.
- Câu SQL của `Store` viết dạng chuỗi với `sqlx::query_as`, không dùng macro `query!` kiểm lúc biên dịch: không cần thư
  mục `.sqlx/` và `sqlx-cli`, build không cần Postgres; đổi lại, mọi câu SQL được kiểm bằng test trên Postgres thật.
- Trạng thái chia sẻ qua một `AppState`: config, `Arc<dyn LlmProvider>`, `Store`, mã thiết lập, bộ nhớ đệm giá trị
  của cột và các semaphore giới hạn việc chạy cùng lúc (đọc Excel, chạy SQL, lập hồ sơ ngữ nghĩa). Không biến toàn cục.
- Test ở tầng service và qua route (`api/*_tests.rs`), với mock LLM, DuckDB in-memory và Postgres thật (CI có
  Postgres).

## 4. Hai kho dữ liệu

| Kho | Chứa | Vì sao |
|---|---|---|
| Postgres (sqlx, migrations trong `backend/migrations/`) | `workspaces`, `memberships`, `users`, `sources`, `datasets`, `dataset_versions`, `columns`, `column_values`, `metrics`, `rls_policies`, `queries`, `conversations`, `settings`, `sessions`; mọi bảng có `workspace_id`, trừ `workspaces`, `users` và `sessions` | Nhiều ghi nhỏ, nhiều kết nối, đúng việc của database giao dịch |
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
thấy miền Nam, một `member` khác thấy toàn bộ. Mọi bảng dùng chung, có cột `workspace_id`. Mọi hàm trong `engine/store/`
nhận kiểu `WorkspaceId`, chỉ tạo được từ phiên đăng nhập đã kiểm, nên quên lọc theo workspace là lỗi biên dịch. Lớp chặn
thứ hai nằm trong database: khóa ngoại giữa các bảng nghiệp vụ gồm cả `workspace_id`, nên một dòng không trỏ được sang
dòng của workspace khác. Postgres Row Level Security sẽ thêm sau làm lớp thứ ba. File Parquet nằm dưới
`/data/<workspace>/`, prompt gửi mô hình ngôn ngữ chỉ chứa hồ sơ của workspace hiện tại.

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
| Xem dataset, hỏi, xem minh chứng | Có | Có |
| Xem danh sách thành viên | Có | Có |
| Tự rời workspace | Có, nếu còn admin khác | Có |
| Nạp file, tạo phiên bản mới | Có | Không (403) |
| Đổi tên, xóa workspace (xóa cả file Parquet) | Có | Không (403) |
| Thêm thành viên theo email, đổi vai, xóa thành viên | Có | Không (403) |
| Xóa dataset (xóa cả file Parquet) | Có | Không (403) |
| Xem chỉ số và cách gộp của cột số đo | Có | Có |
| Tạo, sửa, xóa chỉ số; đổi cách gộp của cột số đo | Có | Không |
| Đánh giá Tốt, Chưa đạt câu trả lời của mình | Có | Có |
| Xem câu được đánh giá Tốt, duyệt làm câu mẫu, xuất câu mẫu cho bộ đánh giá | Có | Không (403) |
| Sửa mô tả cột, bí danh | Có | Không |
| Xem lại câu hỏi của mình (qua cuộc trò chuyện) | Có | Có |

Dòng sửa mô tả cột, bí danh chưa có endpoint; cũng chưa có lưu riêng một câu
hỏi. Câu đã trả lời, bị từ chối hay lỗi đều được ghi vào lịch sử; mỗi
người mới xem lại được câu hỏi của mình qua danh sách cuộc trò chuyện (mục 8), chưa có màn xem lịch sử chung. Khi
thêm, việc chỉ admin làm được gọi `member.require_admin()` ở đầu service hoặc handler, kèm test cho cả hai vai.

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
| Schema context | `columns` | Câu hỏi tách thành cụm 1 đến 5 âm tiết; cụm bằng tên hay bí danh cột chọn cột đó, so có dấu khi cả hai bên có dấu, sau khi bỏ dấu khi một bên không dấu (cột `khu_vuc` khớp "khu vực"). Cả lượt phân loại lẫn lượt viết SQL nhận metadata đủ của mọi cột của mọi bảng câu hỏi đọc: tên, kiểu, vai trò, đơn vị, cách gộp, mô tả, bí danh (#230); cột nhạy cảm cũng vậy, không bao giờ kèm giá trị. Không bỏ cột nào khi prompt dài. Chỉ khi tổng metadata vượt 60 000 ký tự (cỡ 600 cột) thì cột không liên quan rút về dòng gọn (tên, kiểu, vai trò). Cột liên quan: khớp một cụm hay có gợi ý giá trị, lượt phân loại chọn (trường `columns`, dạng bảng.cột, tên không có thật bị bỏ; rơi về luật từ khóa thì rỗng), cột thời gian, cột khóa nối bảng, cột dùng trong công thức chỉ số. Khớp cụm dùng để chọn cột liên quan, gợi ý giá trị và hỏi lại, không để lọc cột. Prompt SQL quá 24 000 ký tự thì bỏ giá trị mẫu trước, rồi giá trị của cột không liên quan; vẫn quá thì gửi nguyên và ghi log (Ollama cần `OLLAMA_NUM_CTX` lớn hơn với schema rất nhiều cột). Một cụm khớp tên của nhiều cột với điểm chênh dưới 0.1 thì hỏi lại (id lựa chọn `column:<cụm đã bỏ dấu>`); chênh nhiều hơn thì chọn cột điểm cao và ghi thành giả định |
| Value mapping | `column_values` | Mỗi cột chiều không nhạy cảm được gửi kèm danh sách giá trị và bí danh, nhiều dòng nhất trước: đủ khi cột tới 50 giá trị, 20 giá trị nhiều dòng nhất khi nhiều hơn, tổng tối đa 400 giá trị (vượt thì cắt cột nhiều giá trị trước). Mô hình tự ánh xạ chữ người dùng sang giá trị trong danh sách ("miền Nam" thành `South`) và trả `{terms, sql}` (#29); endpoint hỏi (#33) báo `filter_value_not_found` khi một giá trị trong `terms` không có trong danh sách đủ của cột. Code chỉ thêm gợi ý khi một cụm từ hai âm tiết, không chứa từ chỉ thời gian, số hay từ nối, bằng chính xác một giá trị hay bí danh (dấu thanh kiểu cũ và mới là một). Giá trị tải một lần cho mỗi phiên bản rồi giữ trong bộ nhớ (32 phiên bản, 200 000 chuỗi); tải quá 2 giây thì đi tiếp không có giá trị |
| Few-shot | `queries` được người hỏi đánh giá Tốt và admin duyệt làm câu mẫu (#212) | Từ khóa trùng, tối đa 3 câu |

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
| Biểu đồ | Theo vai trò cột, loại số đo và đơn vị của kết quả; luật ở mục 6b |

Mô hình ngôn ngữ sinh SQL trên tên bảng logic. Engine resolve tên logic thành nguồn vật lý, hiện luôn là file Parquet
của một phiên bản. Nguồn Postgres (#165) chụp bảng thành phiên bản Parquet như nạp file (đọc bằng sqlx `COPY`, không
qua extension của DuckDB vì extension phải tải từ Internet lúc chạy), nên không đổi guard, prompt hay lịch sử. Nhiều
bảng liên quan chụp cùng lúc thành nhiều dataset, khóa ngoại giữa chúng ghi vào `dataset_relations`, không nối thành
một bảng phẳng (cột của bảng phía một sẽ lặp lại trên mọi dòng phía nhiều và bị cộng nhầm). Truy vấn thẳng database
ngoài lúc hỏi để sau, cần quyết định riêng.

Hỏi trên một dataset có quan hệ (#213, `service/ask/related.rs`): câu thường được đọc thêm các dataset nối với nó qua
`dataset_relations`, cả hai chiều, tối đa 2 bước và 8 bảng kể cả bảng đang chọn, chỉ dataset có phiên bản đang dùng
và đã lập hồ sơ. Mỗi bảng vào guard với bộ lọc dòng của riêng nó. Prompt SQL có một khối cột mỗi bảng, danh sách quan
hệ dạng `order_items.order_id = orders.order_id (many order_items rows per orders row)` và luật nối bảng
(`backend/prompts/sql_joins.txt`): chỉ nối theo quan hệ đã liệt kê, ghi tên bảng trước cột, gộp bảng phía nhiều trước
khi SUM hay AVG cột của bảng phía một. Trần metadata và trần 400 giá trị tính chung cho mọi bảng; trần 24 000 ký tự tính trên
cả prompt. Hỏi lại khi một cụm khớp nhiều cột, giả định cột và cảnh báo giá trị lọc không có chỉ xét bảng đang chọn,
vì tên cột ở đó không kèm tên bảng. Câu "tại sao" và truy dòng nguồn vẫn chỉ một bảng: truy dòng nguồn từ chối câu đọc
nhiều bảng. Guard báo các bảng câu đã đọc; response trả chúng trong `tables_read` và lịch sử ghi phiên bản của chúng vào
`queries.read_versions`. Khi mô hình bỏ qua luật gộp trước, `service/ask/fanout.rs` (#214) bắt câu SUM hay AVG cột của
bảng phía một sau khi nối bảng phía nhiều, sau guard và trước khi chạy: câu không chạy mà vào vòng sửa một lần, câu sửa
vẫn cộng lặp thì bị từ chối. Luật và giới hạn ở [GUARD.md](GUARD.md), mục Kiểm cộng lặp sau guard.

Bộ lọc dòng lan theo khóa ngoại (#215, `service/ask/row_filters.rs`): chính sách chỉ gắn với bảng có cột lọc, nên người
bị lọc theo `customers.region` mà đọc `orders` (không có cột vùng) sẽ thấy đơn của mọi vùng. Vì vậy bảng trỏ tới một
bảng bị lọc (theo `dataset_relations`, chiều khóa ngoại, đi hết mọi bước) nhận thêm điều kiện do code dựng
`"customer_id" IN (SELECT "id" FROM "customers" WHERE <điều kiện của customers>)`. Điều kiện nằm trong hộp lọc của
guard, nên áp ở mọi chỗ đọc bảng đó: câu thường, câu "tại sao", truy dòng nguồn, tổng quan dataset, kể cả khi chọn thẳng
`orders` để hỏi. Hai đường tới bảng bị lọc, hay chính sách riêng cộng điều kiện lan, nối `AND`. Khóa ngoại rỗng: một
đường thì dòng bị ẩn; nhiều đường thì đường rỗng được bỏ qua (khóa tùy chọn), nhưng mọi đường rỗng thì ẩn. Chi tiết và hạn chế ở [GUARD.md](GUARD.md), mục Bước 3. Vai và chính sách vẫn chưa đặt được qua API (#45).

## 6b. Chọn biểu đồ

Biểu đồ là cách người đọc báo cáo tài chính so số, nên luật theo chuẩn của họ chứ không tự đặt: IBCS (International
Business Communication Standards), cùng *Say It With Charts* của Gene Zelazny cho cách chia loại so sánh. Một luật duy
nhất không đủ, vì chọn đúng phải biết bốn điều về kết quả: so sánh gì (các nhóm, theo thời gian, cơ cấu theo thời
gian, chênh lệch hai kỳ), số đo cộng được hay không (tổng, đếm so với trung bình, tỷ lệ), có bao nhiêu nhóm hay kỳ, và
các số đo cùng đơn vị không. Chọn sai làm người đọc hiểu sai: nối đường giữa các nhóm như thể chúng liên tục, cột
không bắt đầu từ 0 làm chênh lệch nhỏ trông lớn, hai trục khác thang đo làm hai số đo trông cùng chiều hay ngược chiều
tùy cách chọn thang.

`service/ask/chart.rs` chọn từ vai trò cột (`service/understanding/mod.rs`), cây SQL đã chạy (loại hàm tổng hợp) và
đơn vị trong hồ sơ cột (`columns.unit`, [Dữ liệu](DATA.md)); giao diện vẽ từ chính các dòng của `result`. Biểu đồ
dựng sẵn của trang tổng quan dataset (`service/overview/charts.rs`) đi qua cùng hàm, kể cả số dòng theo giá trị lấy từ
hồ sơ: luật đọc câu `GROUP BY` lẽ ra tính ra bảng đó.

1. Một khóa là nhóm (doanh thu theo thị trường): thanh ngang, xếp lớn trước, vì IBCS đặt cấu trúc trên trục đứng và tên
   nhóm dài vẫn đọc được. Quá 15 nhóm thì vẽ 15 nhóm lớn nhất; số đo cộng được và đủ dòng thì phần còn lại gộp "Khác".
   IBCS không đặt số cho thanh đơn: 15 chọn ở đây.
2. Một khóa là thời gian: thời gian luôn trên trục ngang (IBCS). Tới 10 kỳ là cột đứng, hơn 10 kỳ là đường có điểm
   đánh dấu ở từng kỳ (mẫu C07 của IBCS: đường hợp khi hơn 10 kỳ).
3. Một dòng: không vẽ, bảng đã hiện con số.
4. Thời gian và một nhóm, một số đo: số đo cộng được và tới 25 kỳ thì cột chồng, tối đa 6 phần (5 nhóm lớn nhất cộng
   "Khác"), theo mẫu C01 của IBCS. Số đo không cộng được (trung bình, đếm phân biệt) hay quá 25 kỳ thì mỗi nhóm một
   đường, khi không quá 5 nhóm (số này chọn ở đây); nhiều hơn thì chỉ bảng. Không dùng biểu đồ tròn: IBCS không xếp nó
   vào các loại chính, Zelazny khuyên dùng cho dưới 5% số biểu đồ.
5. Câu "tại sao": thác nước (tổng kỳ trước, phần tăng giảm của từng nhóm, tổng kỳ sau), loại IBCS dùng cho phân tích
   chênh lệch; vẽ từ `explanation`, không qua `chart`.
6. Nhiều số đo khác đơn vị hay chưa biết đơn vị: mỗi số đo một biểu đồ nhỏ, chung trục nhóm hay thời gian, không dùng
   hai trục.
7. Nhiều số đo cùng đơn vị (số ngày giao thực tế và dự kiến; doanh thu năm nay và năm trước): cạnh nhau trên một thang
   đo.

Chung cho mọi biểu đồ: cột và thanh bắt đầu từ 0; số đo không cộng được vẽ cột hẹp (IBCS: rộng 1/3 thay vì 2/3) và
không bao giờ gộp "Khác"; tối đa 4 số đo; hai khóa đều là nhóm, hay ba khóa trở lên, thì chỉ bảng.

## 7. Dữ liệu thay đổi

Mỗi lần nạp file là một phiên bản, không ghi đè. `queries` trỏ tới `version_id`.

- Hash SHA-256: trùng thì bỏ qua, khác thì phiên bản mới.
- So hồ sơ cũ và mới cho ra khác biệt schema: cột mất, thêm, đổi kiểu, giá trị phân loại mới. Hiện cho người dùng
  trước khi chuyển phiên bản hiện hành; cảnh báo nếu chính sách phân quyền trỏ tới giá trị không còn.
- Nguồn sống: phiên bản là bản chụp hồ sơ tại thời điểm, làm mới theo lịch hoặc khi bấm.

## 8. API

REST và JSON. API dữ liệu nằm dưới workspace trong đường dẫn: `/api/workspaces/{ws}/datasets`,
`/api/workspaces/{ws}/datasets/{id}/ask`. Workspace đang làm việc không lưu trong phiên, nên mỗi tab trình duyệt làm
việc độc lập, tab này không đổi workspace của tab kia. Mỗi request kiểm phiên (chưa đăng nhập: 401) và kiểm người dùng
thuộc workspace đó (không thuộc hoặc không có workspace: cùng một lỗi 404, không dò được tên workspace).

OpenAPI 3.1 sinh từ code bằng `utoipa`: mỗi handler khai báo đường dẫn, tham số, request và response bằng
`#[utoipa::path]`, struct request và response derive `ToSchema`. Router đăng ký handler qua `OpenApiRouter` của
`utoipa-axum` (`api/mod.rs`), nên route có trong router thì có trong đặc tả; phần chung (tên API, địa chỉ gốc `/api`,
cookie phiên, kiểu lỗi `{code, message}`) ở `api/openapi.rs`. Mô tả trong đặc tả viết tiếng Anh, vì người đọc là người
dùng API.

Đặc tả phục vụ ở `/api/openapi.json`, Swagger UI ở `/api/docs`; cả hai không cần đăng nhập, gọi thử endpoint thì cần
cookie phiên như giao diện. Swagger UI kèm sẵn trong binary (feature `vendored` của `utoipa-swagger-ui`), build không
tải gì từ Internet; thông báo giấy phép của nó ở `/api/docs/licenses.txt` (LICENSES.md). Công khai đặc tả không lộ gì
mới: mọi đường dẫn đã có trong file JavaScript của giao diện, vốn tải được khi chưa đăng nhập, và đặc tả không chứa bí
mật. Swagger UI chạy cùng địa chỉ với cookie phiên (httpOnly, SameSite=Lax), nên một lỗi XSS của Swagger UI sẽ gọi được
API bằng quyền người đang mở trang; chấp nhận ở giai đoạn này. Tham số `?url=` của Swagger UI bị bỏ qua (cấu hình cố
định), nên không nạp được đặc tả từ nơi khác.

Hợp đồng nằm trong repo:

| File | Sinh từ | Kiểm lệch |
|---|---|---|
| `openapi.json` (gốc repo) | `xdi-backend openapi`, không cần database | Test `openapi_json_is_up_to_date` trong `make check-fast` (CI job backend, hook pre-push) |
| `frontend/src/api/schema.ts` | `openapi.json`, bằng `openapi-typescript` (`frontend/scripts/api-types.mjs`) | `npm run api:check` trong `make check-frontend` (CI job frontend) |
| `frontend/src/api/examples/ask.json` | `xdi-backend ask-examples` (`service/ask/examples.rs`), không cần database | Test `ask_examples_json_is_up_to_date` trong `make check-fast` |

Đổi API: sửa handler hoặc struct, chạy `make openapi`, commit cả ba file. Hook trong `frontend/src/api` lấy kiểu từ
`schema.ts`, không khai lại. Khái niệm chung (phiên, workspace, vai) và bảng mã lỗi: [API.md](API.md).

### Hỏi dữ liệu

`POST /api/workspaces/{ws}/datasets/{id}/ask` nhận `{question, choices, conversation_id}` và trả một `AskResponse`, JSON
một lần, không SSE: giao diện gọi bằng `useMutation` và chờ đủ gói. Kiểu nằm ở `service/ask/contract.rs` chứ không ở
`api/`, vì service dựng response mà không được gọi ngược lên api; nghĩa từng trường ghi trong đặc tả.
`service/ask/mod.rs` ghép các bước của mục 2 cho câu thường ([#33](https://github.com/pgiahuy/xdi-platform/issues/33)).
Câu answered có thẻ "Hệ thống đã hiểu là", vai trò cột và gợi ý biểu đồ (`service/ask/view.rs`). Câu "tại sao" rẽ sang
`service/ask/why.rs` sau bước context: gọi `run_why` ([#38](https://github.com/pgiahuy/xdi-platform/issues/38)), thẻ "Đã
hiểu là" dựng từ form đã kiểm (`service/understanding/why.rs`), ghi `{form, explanation}` vào `queries.explanation` để
truy dòng phân rã. Mọi SQL của cả hai đường qua guard với lọc dòng theo vai dữ liệu của người hỏi (admin không bị lọc).
Cảnh báo kiểm hợp lý ([#36](https://github.com/pgiahuy/xdi-platform/issues/36)) dựng ở `service/ask/checks.rs` sau khi
chạy, không đổi `status`; câu "tại sao" chỉ nhận `empty_result`.

| Điểm | Chọn | Vì sao |
|---|---|---|
| Câu không trả lời được | Vẫn 200, `status` trong body: `answered`, `needs_choice` (câu mơ hồ), `rejected` (guard từ chối), `failed` (SQL chạy lỗi cả sau vòng sửa, mô hình lỗi trong vòng sửa, câu trả lời của mô hình không đọc được cả sau vòng sửa, hoặc câu chạy quá thời gian hay quá bộ nhớ), kèm `message` tiếng Việt. Mã lỗi chỉ cho request sai (400), quyền (401, 404), dịch vụ mô hình lỗi (502) hay chưa cấu hình (503) khi chưa có SQL nào | Các trường hợp này là kết quả bình thường, phải hiện cùng bằng chứng: SQL bị từ chối, các bước, lựa chọn; `ErrorBody` chỉ có `code` và `message`. `useMutation` coi mã lỗi là lỗi, giao diện sẽ phải gỡ dữ liệu ra khỏi lỗi. Câu bị từ chối và câu lỗi vẫn ghi vào `queries` (status `rejected`, `error`) nên có `query_id`. Câu mơ hồ không ghi; nó có thể đến trước khi gọi mô hình (một từ khớp nhiều cột) hoặc sau (câu "tại sao" không xác định được hai lát cắt, [#38](https://github.com/pgiahuy/xdi-platform/issues/38)) |
| Giá trị ô | Mỗi cột có `kind`, ô là giá trị JSON theo `kind`, quyết định theo kiểu cột chứ không theo từng giá trị. Số nguyên tới 32 bit, FLOAT, DOUBLE: số JSON (NaN, vô cực thành null). BIGINT, HUGEINT, DECIMAL: chuỗi chữ số thập phân. Ngày `YYYY-MM-DD`, giờ ISO 8601 (TIMESTAMPTZ đổi sang UTC, có `Z`). NULL: null | Số JavaScript chỉ giữ đúng số nguyên tới 2^53 và làm tròn số thập phân, mà con số là bằng chứng. Cùng một cột luôn một dạng, giao diện đọc theo `kind` thay vì đoán từng ô. `COUNT` trả BIGINT nên số đếm cũng là chuỗi; vẽ biểu đồ hay sắp xếp thì đổi bằng `Number()` |
| Cột | Tên cột trong `result.columns` là duy nhất: DuckDB trả tên trùng nguyên văn, nên backend đổi từ trái sang phải, tên đã dùng nhận hậu tố `_2`, `_3`... nhỏ nhất mà không cột nào có (`a, a, a_2` thành `a, a_3, a_2`), bằng `build_unique_names` trong `service/ask/contract.rs`, drill dùng lại đúng hàm này; `chart`, tiêu đề CSV và khóa truy dòng nguồn trỏ cột bằng tên. Mỗi cột có `role`: `dimension`, `time`, `measure`, `other`, lấy từ cây cú pháp | Tên đọc được hơn chỉ số và đủ khi đã duy nhất. `role` cho giao diện biết ô nào bấm được (độ đo) và khóa của dòng (các cột `dimension`, `time`) |
| Số dòng | `rows` là mảng theo thứ tự cột, tối đa 1000 dòng đầu. `total_rows` là số dòng SQL sau guard trả về; `truncated` khi gửi ít hơn `total_rows`; `limit_reached` khi `total_rows` chạm trần số dòng của guard, dù guard thêm LIMIT hay hạ LIMIT có sẵn (kết quả đầy đủ có thể còn nữa) | Mảng gọn hơn object theo tên cột. 1000 dòng đủ cho bảng và biểu đồ của kết quả đã gộp. Giao diện của v0.1.0 phân trang trong các dòng đã gửi; khi `truncated` ghi rõ "1000 trên N dòng", khi `limit_reached` ghi "ít nhất N dòng" để không nói sai tổng. Tải CSV lấy đúng các dòng đã gửi, kèm ghi chú khi bị cắt. Tải đủ mọi dòng (endpoint riêng theo `query_id`, chạy lại SQL sau guard trên phiên bản đã lưu) làm sau. Trần của guard là `ASK_MAX_ROWS` (mặc định 100 000) và hiện ngay trong SQL |
| Thẻ "Hệ thống đã hiểu là" | `understanding`: `measures`, `filters`, `group_by`, `order_by`, `limit`, `time_range`, mỗi mục có nhãn và đoạn SQL nó lấy ra; `filters` gồm cả điều kiện phân quyền guard thêm vào, nhãn nói rõ; `assumptions` là danh sách `Choice` đã chọn | [#35](https://github.com/pgiahuy/xdi-platform/issues/35) sinh từ cây cú pháp và kiểm từng mục khớp câu đã chạy; [#50](https://github.com/pgiahuy/xdi-platform/issues/50) vẽ chip từ `Choice`. Người dùng biết mình chỉ thấy một phần dữ liệu |
| Đổi giả định, trả lời câu hỏi lại | Response nào cũng có `choices`: mọi lựa chọn đã áp dụng, kể cả mặc định và câu hỏi lại đã trả lời, cùng dạng `[{choice_id, option_id}]` của request. Hỏi lại thì gửi cùng câu hỏi kèm `choices` của response gần nhất, thay hoặc thêm mục vừa chọn. Người dùng sửa chữ câu hỏi thì đó là câu mới, giao diện gửi `choices` rỗng. Câu mơ hồ trả `clarifications`, cùng kiểu `Choice` nhưng chưa chọn. Id của lựa chọn ổn định cho cùng câu hỏi trên cùng phiên bản dữ liệu: đặt từ loại lựa chọn và đối tượng của nó (một từ và các cột ứng viên, kỳ để so sánh), như `region_column`, `base_period` trong mẫu, không đặt từ chữ của SQL. Id không nhận ra thì bỏ qua, không có trong `choices` trả về, kèm cảnh báo `choice_not_applied`: giả định dùng mặc định, câu hỏi lại thì được hỏi lại. Nếu `needs_choice` quay lại đúng câu vừa trả lời kèm cảnh báo đó, giao diện hiện `message` thay vì hỏi lại y nguyên | Backend không giữ trạng thái giữa hai lần hỏi, và giao diện chỉ cần response gần nhất: thiếu một lựa chọn đã trả lời thì backend sẽ hỏi lại chỗ đó mãi. Id sinh từ SQL sẽ đổi khi mô hình viết SQL khác ở lần hỏi sau, và câu "tại sao" hỏi lại sau khi gọi mô hình sẽ lặp mãi. Bỏ qua thay vì 400 vì một giả định có thể thật sự không còn khi SQL đổi; trả 400 thì người dùng kẹt. Câu đã sửa mà giữ lựa chọn cũ thì lựa chọn hoặc áp dụng ngầm, hoặc sinh cảnh báo khó hiểu |
| Phân rã | `explanation`, chỉ có ở câu "tại sao": hai lát cắt (`base`, `compare`, mỗi lát có nhãn, điều kiện SQL, giá trị), chênh lệch, mỗi chiều một bảng đóng góp của từng nhóm kèm SQL đã chạy sau guard (chiều giải thích tốt nhất trước), tách lượng và giá kèm SQL, đoạn tóm tắt và cờ `summary_by_model`. `comparison` cho biết hai lát cắt là hai kỳ (`periods`, chữ "tăng", "giảm") hay hai nhóm cùng lúc (`groups`, chữ "cao hơn", "thấp hơn"); lời viết theo khuôn, prompt của mô hình và giao diện chọn chữ theo trường này. Không phân rã được thì cảnh báo `explanation_unavailable` nói lý do. Số là chuỗi chữ số thập phân; độ đo đổi sang DECIMAL ở từng dòng nguồn trước khi cộng, không làm tròn từng nhóm. Phần lượng và giá có một phép chia (giá = độ đo / lượng): giá làm tròn tới số chữ số cố định (#40 chọn) và là chỗ làm tròn duy nhất, `volume_effect` tính đúng từ giá đã làm tròn, `price_effect = change - volume_effect` nhận phần lệch do làm tròn. Bảng kết quả cộng theo kiểu cột (DOUBLE là số thực), nên giá trị hai lát cắt có thể lệch ô của bảng ở chữ số cuối; con số của phân rã lấy từ `explanation`. Độ đo tỷ lệ (`measure_kind` là `ratio`: chỉ số dạng AVG(x), SUM(a) / SUM(b), COUNT(...) / COUNT(*)) không cộng được theo nhóm: giá trị lát cắt và nhóm là tỷ lệ bốn chữ số thập phân, đóng góp của nhóm tách đối xứng theo Kitagawa thành `mix_effect` (tỷ trọng mẫu số của nhóm đổi) và `rate_effect` (tỷ lệ trong nhóm đổi), mỗi chiều có tổng hai phần; nhóm chỉ có ở một lát cắt lấy tỷ lệ của lát còn lại nên chỉ có phần cơ cấu; phần lệch làm tròn chia theo cách phần dư lớn nhất để tổng đóng góp bằng đúng chênh lệch; không có tách lượng và giá | Mọi con số kiểm được trên giao diện bằng SQL đứng sau nó ([#52](https://github.com/pgiahuy/xdi-platform/issues/52)). Phép cộng trên DECIMAL là đúng, nên tổng đóng góp bằng đúng chênh lệch ([#39](https://github.com/pgiahuy/xdi-platform/issues/39), [#40](https://github.com/pgiahuy/xdi-platform/issues/40)) và [#41](https://github.com/pgiahuy/xdi-platform/issues/41) so số trong lời với đúng các chuỗi này |
| Biểu đồ | `chart`: `kind` (`bar` thanh ngang, `column` cột đứng, `line` đường, `stacked_column` cột chồng), `x` là trục nhóm hay thời gian, `y` các số đo, `layout` (`single`, `grouped` cạnh nhau, `panels` mỗi số đo một biểu đồ), `series` là cột chia phần hay chia đường, `ratio` các số đo không cộng được, `top` và `other_bucket` cho số nhóm vẽ và phần "Khác"; null thì chỉ hiện bảng | Luật ở mục 6b (`service/ask/chart.rs`). Gợi ý chỉ nêu tên cột, số liệu lấy từ `result` nên biểu đồ luôn khớp bảng |
| Các bước | `steps`: tên bước, thời gian (ms), thành công hay không, chi tiết. `classify` là bước phân loại câu hỏi, chi tiết là loại câu kèm lý do của mô hình, hoặc lý do dùng luật từ khóa (khi đó `ok` là false). `explain` (code tính phân rã) tách khỏi `narrate` (mô hình viết lời); `repair` là lần mô hình sửa SQL, tối đa một lần: sau `run` lỗi, sau `guard` từ chối vì bất kỳ lý do nào, hoặc khi câu trả lời đầu của mô hình không đọc được (khi đó `repair` là bước đầu tiên; câu quá giờ hay quá bộ nhớ thì không sửa). Bước lỗi không nhất thiết dừng câu hỏi (`run` hay `guard` lỗi có thể theo sau là `repair`, `narrate` lỗi thì dùng câu khuôn); `status` cho biết câu hỏi có dừng không, và gọi theo bước đã dừng nó: guard thì `rejected`, chạy hay vòng sửa thì `failed` | Tab Các bước của pane Minh chứng hiện danh sách này, thấy được mô hình chỉ tham gia ở đâu; giao diện tự đặt nhãn cho từng tên bước |

Truy dòng nguồn ([#34](https://github.com/pgiahuy/xdi-platform/issues/34)): `POST .../datasets/{id}/drill`, hợp đồng
ở `service/drill/contract.rs`. Request gửi `query_id` cùng khóa của chỗ được bấm, không bao giờ gửi SQL. Bấm một ô của
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
| Dòng phân rã | Đọc `queries.explanation`: câu SQL của chiều được bấm (`DimensionBreakdown.sql`, đã qua guard khi hỏi) và điều kiện của lát cắt được bấm; truy như một ô của câu chiều đó, cộng điều kiện lát cắt. Câu không có phân rã, hay chiều không có trong phân rã: 400 | Câu chiều đã mang mọi bộ lọc của câu hỏi, của chỉ số và hộp phân quyền của người hỏi, nên dùng lại đúng cách dựng câu của ô; câu chiều đọc cả hai lát cắt, nên phải thêm điều kiện lát cắt. Điều kiện và SQL luôn lấy từ chỗ đã lưu, request chỉ gửi tên |

`service/ask/examples.rs` dựng năm response mẫu trên bảng `dataco`: câu "tại sao" có phân rã (`why`), câu một chiều một
độ đo (`simple`), câu mơ hồ (`needs_choice`), câu bị guard từ chối (`rejected`), câu chạy lỗi cả sau vòng sửa
(`failed`). Mẫu là example của response 200 trong đặc tả, và nằm ở `frontend/src/api/examples/ask.json` làm mẫu cho
người viết giao diện. Test kiểm file khớp code và mẫu khớp schema trong đặc tả, vì TypeScript không kiểm được file JSON theo enum. Số trong mẫu, kể cả
thời gian các bước, là số minh họa, không phải số tính hay đo trên dữ liệu thật.

## 9. Hướng phát triển

Bản hiện tại nhắm dự án cỡ nhỏ và vừa, một instance. Các giới hạn và đường nâng cấp:

| Giới hạn | Nguyên nhân | Đường nâng cấp |
|---|---|---|
| Một instance backend cho phần phân tích | DuckDB nhúng trong tiến trình; dữ liệu ứng dụng đã ở Postgres nên không vướng | Tách engine phân tích thành service riêng đọc Parquet trên object storage |
| Phiên đăng nhập tự quản trong Postgres | Không có identity provider | OIDC (Zitadel, Keycloak); `auth/` là module riêng |
| Dataset trên đĩa local | Volume docker | S3 hoặc MinIO, chỉ đổi phần nạp file trong `engine/analytics.rs` |
| Một crate | Chưa có gì để tách | Rút engine hoặc guard thành crate trong workspace khi cần dùng lại |
| Không cache, không queue | Chưa có tải | Redis cho cache câu hỏi, queue cho nạp file lớn, cắm vào tầng service |
| Nguồn là file hoặc bản chụp bảng Postgres | Chụp giữ được bằng chứng tái lập | MySQL, SQLite; truy vấn thẳng database ngoài lúc hỏi |

Ba điều giữ cho các nâng cấp trên không phải viết lại: state đi qua `AppState`; hợp đồng API là tài liệu và kiểu dữ
liệu; mọi lời gọi ra ngoài đi qua một chỗ để thêm retry, timeout, cache.

## Trạng thái

Cập nhật khi có thay đổi.

| Phần | Trạng thái |
|---|---|
| Khung bốn tầng, `AppState`, `AppError`, cấu hình | Có |
| `/api/health`, `/api/version`; mọi API dưới `/api`, đường dẫn API sai trả 404 JSON | Có |
| OpenAPI (mục 8): đặc tả sinh từ code ở `/api/openapi.json`, Swagger UI ở `/api/docs`, `openapi.json` và kiểu TypeScript `frontend/src/api/schema.ts` trong repo, test và CI báo khi lệch code | Có |
| Trait `LlmProvider` và ba cài đặt; `complete` nhận loại việc, câu hỏi gốc, prompt và JSON Schema của câu trả lời nếu cần | Mock trả lời các câu viết sẵn trên 13 bảng chụp từ database mẫu (`engine/llm/mock_answers_database.json`: hồ sơ ngữ nghĩa và câu gợi ý của từng bảng, 18 câu thường của `eval/sales_joins.json` và `eval/edu.json`, phần lớn nối bảng, kèm câu phân loại) và trên DataCo (bảng logic `dataco`, `engine/llm/mock_answers.json`): 16 câu thường, trong đó một câu cố ý đọc sai định dạng ngày và có câu sửa để thử vòng sửa, ba câu để thử nhánh bị từ chối, chạy lỗi cả sau khi sửa và giá trị lọc không có trong cột; 5 câu "tại sao", trong đó một câu dữ liệu không trả lời được; câu phân loại cho cả 21 câu đó; cùng hồ sơ ngữ nghĩa và một lời diễn giải viết sẵn; câu trả lời sinh SQL là object `{terms, assumptions, clarify, unanswerable, sql}` (`service/ask/reply.rs`), Gemini nhận schema qua `responseJsonSchema`, Ollama qua `format`; Gemini (`generateContent`) và Ollama (`/api/generate`, kèm `options.num_ctx` theo `OLLAMA_NUM_CTX`) gọi qua HTTP, có timeout theo cấu hình, không theo chuyển hướng; lỗi trả lời hướng dẫn tiếng Việt với mã 502 (`upstream`, dịch vụ mô hình lỗi) hoặc 503 (`unavailable`, máy chủ thiếu cấu hình); khóa Gemini không vào log; hồ sơ ngữ nghĩa (#23) gọi `complete` sau mỗi lần nạp, kèm JSON Schema của câu trả lời (`service/semantic/schema.rs`); endpoint hỏi (#33) gọi `complete` để sinh SQL và sửa; đã chạy với Gemini thật trên DataCo (hồ sơ ngữ nghĩa và một câu thường, #135), Ollama mới thử với server giả |
| Nạp dataset: `POST /api/workspaces/{ws}/datasets` (multipart, một hoặc nhiều file CSV hoặc Parquet, hoặc một file Excel kèm sheet đã chọn), mỗi lần nạp một phiên bản, bộ file trùng thì bỏ qua, giới hạn `MAX_UPLOAD_MB`; liệt kê sheet của file Excel ở `POST .../datasets/sheets`; xóa dataset cùng mọi phiên bản, hồ sơ, chỉ số, cuộc trò chuyện, lịch sử câu hỏi và file Parquet ở `DELETE .../datasets/{id}` (chỉ admin; migration `0011_delete_query_history.sql`, #187) | Có; sheet Excel khác cột nạp riêng từng lần; chưa xóa riêng một phiên bản hay chọn lại phiên bản cũ |
| Nguồn database (`service/sources/`, `api/sources.rs`, [DATA.md](DATA.md)): admin thêm nguồn Postgres (thử kết nối trước khi lưu, máy chủ trong `SOURCE_ALLOWED_HOSTS`, thông tin kết nối mã hóa bằng `SOURCE_SECRET_KEY`), liệt kê bảng và view, chụp một bảng thành phiên bản Parquet qua đường nạp CSV, chụp lại thành phiên bản mới, xóa nguồn; trang Dữ liệu có khung "Kết nối database". Chụp kèm bảng liên quan theo khóa ngoại (cả hai chiều, tối đa 10 bảng, một giao dịch `REPEATABLE READ`): mỗi bảng một dataset, bảng đã chụp thì thêm phiên bản, khóa ngoại giữa chúng ghi vào `dataset_relations`; chụp lại cả nhóm. Giao diện chọn bảng tách hai chiều, mở thêm một bước từ bảng đã chọn, danh sách dataset ghi dataset liên quan, hộp xóa báo quan hệ mất theo. Trang Dữ liệu đặt khung "Kết nối database" lên đầu, nạp file là nút phụ (#230). TLS bằng rustls: `sslmode` `disable`, `prefer`, `require`, `verify-full` (CA của hệ thống, cộng CA dán vào) | Có, chỉ Postgres (#165); TLS có test luật và test qua route với máy chủ không bật TLS, `require` và `verify-full` mới thử tay với một Postgres bật TLS trên máy dev; chưa có `verify-ca`, chứng chỉ phía client, MySQL, SQLite, truy vấn thẳng database ngoài lúc hỏi |
| Hỏi qua bảng liên quan (`service/ask/related.rs`, `service/ask/group_prompt.rs`, #213): câu thường trên dataset chụp từ database đọc thêm các dataset nối qua `dataset_relations` (hai chiều, 2 bước, 8 bảng), mỗi bảng một bộ lọc dòng; prompt có khối cột mỗi bảng, danh sách quan hệ và luật nối bảng; guard nhận tối đa 6 nguồn mỗi `FROM`; `tables_read` trong response, `queries.read_versions` trong lịch sử, khung Minh chứng ghi bảng đã đọc; bảng trỏ tới bảng bị lọc nhận điều kiện lan theo khóa ngoại (#215) | Có, kèm test prompt, test luật chọn bảng và test qua route trên ảnh chụp Postgres thật với mô hình giả. Chưa chạy với mô hình thật; `make eval` chạy bộ 10 câu JOIN trên schema `sales` (`eval/sales_joins.json`) và 8 câu trên schema `edu` (`eval/edu.json`) trên ảnh chụp database mẫu nhỏ với mock (#230); chặn cộng lặp (`service/ask/fanout.rs`, #214) có test luật và test qua route, chưa xét `COUNT(*)` và bảng `WITH`; form câu "tại sao" chỉ nêu cột của bảng đang hỏi (phân rã thì xét cả bảng liên quan, #227); truy dòng nguồn chỉ một bảng |
| Route ask, drill | Có route dưới `/api/workspaces/{ws}/`, kiểm đăng nhập và thành viên. Ask chạy luồng hỏi câu thường (`service/ask/mod.rs`, #33) và câu "tại sao" (`service/ask/why.rs`, #38), trả thẻ "Hệ thống đã hiểu là", vai trò cột và gợi ý biểu đồ: câu mơ hồ hỏi lại trước khi gọi mô hình, câu cần thông tin không cột nào có thì mô hình báo `unanswerable` và trả `rejected` không chạy SQL (`service/ask/unanswerable.rs`), câu trả lời, câu bị từ chối và câu lỗi ghi vào `queries`; drill truy được dòng nguồn của một ô và của một dòng phân rã (mục 8). Hợp đồng request và response của ask (mục 8) có trong đặc tả, kèm response mẫu cho giao diện |
| Trang hỏi dữ liệu (`pages/AskPage.tsx`): ô chọn dataset ở đầu khung trò chuyện, dính khi cuộn (`components/ask/DatasetPicker.tsx`), kèm dòng báo hồ sơ đang lập, không có câu gợi ý với mô hình hiện tại, hoặc lỗi; câu gợi ý của dataset (tự tải lại khi hồ sơ đang lập); ô hỏi dính đáy khung; câu trả lời gồm đoạn trả lời ngắn, biểu đồ theo gợi ý của backend (Recharts, tải riêng khi có biểu đồ), bảng kết quả phân trang kèm tải CSV các dòng đã nhận, thẻ "Hệ thống đã hiểu là" với giả định đổi được; câu "tại sao" có một hàng ba số (hai lát cắt và chênh lệch), lời giải thích và thanh đóng góp của từng nhóm ở chiều đầu; câu hỏi lại, lời báo khi bị từ chối, chạy lỗi, mô hình lỗi | Có, dựng trên hợp đồng; trang gọi route ask thật |
| Hồ sơ cột lớp cấu trúc (`service/profile.rs`, `engine/profile.rs`, [DATA.md](DATA.md)): kiểu, số ô trống, số giá trị khác nhau, min, max, vai trò, định dạng của cột ngày dạng chữ, tần suất giá trị của cột tối đa 200 giá trị; đánh dấu cột nhạy cảm theo luật tên cột và dạng giá trị trong cùng giao dịch ghi hồ sơ; lập lúc nạp, phiên bản cũ lập nền và quét đánh dấu lại lúc khởi động | Có; đọc qua route tổng quan dataset (dòng dưới) |
| Hồ sơ cột lớp ngữ nghĩa (`service/semantic/mod.rs`, [DATA.md](DATA.md)): sau khi nạp trả 201, mô hình đề xuất mô tả, bí danh cột và giá trị, đơn vị của cột số đo (danh sách cố định, `columns.unit`), cách gộp mặc định của cột số đo (`sum` hay `avg`, `columns.aggregation`, #185), tối đa 5 câu hỏi gợi ý (câu khó trước: hai câu "vì sao" theo khuôn trung tính, không nêu chiều tăng giảm, số đo là số dòng, tổng của cột `sum` hay trung bình của cột `avg`; câu còn chỗ trống `<...>` của khuôn bị bỏ), cột nhạy cảm cần đánh dấu thêm (chỉ chiều bật), sửa vai trò trong phạm vi hẹp (measure số nguyên thành key, hoặc thành dimension khi tối đa 200 giá trị; dimension thành text); chạy nền một lượt mỗi lúc, lỗi giữ `structure` và đánh dấu đã thử; thử lại khi khởi động cho phiên bản chưa thử với provider đang cấu hình và khi nạp lại cùng bộ file; prompt chỉ chứa siêu dữ liệu và giá trị (có trần kích thước) của cột dimension không nhạy cảm; câu gợi ý và `current_profile_status` trả ra trong API dataset | Có, kèm test; đã chạy với Gemini thật trên DataCo (#135, #180); chưa có API và màn xem, sửa hồ sơ; phiên bản đã lập hồ sơ trước #180 giữ câu gợi ý cũ tới khi nạp phiên bản mới |
| Gói khái niệm ngành (`data/domains/*.json`, `engine/domains.rs`, [DOMAINS.md](DOMAINS.md)): `build.rs` nhúng mọi file vào binary, hàm liệt kê mọi gói và tìm theo id | Có gói bán lẻ và chuỗi cung ứng (`retail`); prompt câu thường và form câu "tại sao" chèn gói theo `datasets.domain`, nhưng chưa có chỗ nào đặt cột này (lúc nạp hay qua API), nên hiện chưa dataset nào dùng gói |
| Schema Postgres (migration `0001_init.sql` tới `0010_column_aggregation.sql`), kết nối và tự migrate lúc khởi động | Có |
| Tầng lưu trữ theo workspace: nguồn, dataset, phiên bản, lịch sử câu hỏi; kiểu `WorkspaceId` | Có; liệt kê, xem và xóa dataset đã dùng |
| Workspace: liệt kê, tạo mới (người tạo là admin), đổi tên, xóa; mọi API dữ liệu kiểm thành viên; `WorkspaceId` chỉ tạo từ dòng memberships của người đăng nhập | Có; giao diện tạo workspace từ ô chọn workspace, đổi tên và xóa ở trang Thành viên |
| Vai `admin` và `member` (mục 4b): quản lý thành viên, luật còn ít nhất một admin, quản trị hệ thống xem mọi workspace và chỉ định admin | Có |
| Khung giao diện React và Vite; bố cục chung: thanh bên có hàng thương hiệu, ô đổi workspace (kèm mục tạo workspace) hiện vai trong workspace đang mở, mục điều hướng có icon Lucide, tài khoản, đăng xuất và phiên bản ở chân; màn rộng thu gọn được thanh bên thành dải icon (nút ở hàng thương hiệu, nhớ trong localStorage); vùng nội dung rộng hết phần còn lại của màn; màn hẹp: thanh bên vào ngăn kéo, tài khoản ở góc thanh trên; dùng được ở bề ngang điện thoại; trang làm việc dưới `/w/<workspace>`, trang quản trị hệ thống và trang tài khoản ngoài workspace (`/admin/...`, `/account`) | Có |
| Backend phục vụ giao diện cùng cổng với API (`STATIC_DIR`), một image chứa cả hai | Có |
| Chạy thật: `docker-compose.prod.yml` (Postgres không mở cổng, mật khẩu bắt buộc, backend nghe `127.0.0.1`), cookie `Secure` theo `PUBLIC_URL`, header bảo mật trên mọi response (`api/security_headers.rs`), `make backup`, `make restore`, mẫu Caddy và nginx | Có; role Postgres riêng không phải superuser cho RLS chưa có |
| Tài khoản: mã thiết lập in ra log lần đầu, tạo tài khoản quản trị kèm workspace đầu tiên, đăng nhập, đăng xuất, đổi mật khẩu; quản trị hệ thống tạo, khóa, mở tài khoản, đặt lại mật khẩu; lệnh `reset-password`, `list-admins` (API, phiên lưu trong Postgres) | Có; giao diện có màn thiết lập lần đầu và đăng nhập (panel giới thiệu sản phẩm cạnh form, xếp dọc khi màn hẹp), đổi mật khẩu, quản lý tài khoản của quản trị hệ thống |
| DuckDB nhúng: đổi CSV (UTF-8; file không phải UTF-8 đổi sang UTF-8 theo windows-1252, gồm cả latin-1, trừ file UTF-8 có byte hỏng thì báo lỗi kèm số dòng; `engine/csv_encoding.rs`) và Parquet sang Parquet, gộp nhiều file cùng cột kèm cột `_source_file` (`engine/analytics.rs`); sheet Excel đổi sang CSV bằng `calamine` (`engine/excel.rs`) | Có |
| Guard SQL (`engine/guard/`, [GUARD.md](GUARD.md)): một câu `SELECT` chỉ đọc, danh sách bảng, hàm, cột được phép, lọc dòng theo chính sách service truyền vào, giới hạn dòng, bản `shown` và bản `runnable` đọc Parquet | Có, kèm test; endpoint hỏi (#33, #38), drill (#34) và tổng quan (#198) đọc chính sách lọc dòng theo vai dữ liệu của người hỏi, người bấm hay người xem (admin không bị lọc), kèm điều kiện lan theo khóa ngoại giữa các dataset chụp từ database (#215); store đã có hàm đặt `data_role` cho thành viên và ghi `rls_policies` (`engine/store/policies.rs`), nhưng chưa service hay API nào gọi, nên lọc dòng chưa dùng được ngoài test |
| Chạy SQL và vòng sửa (`engine/query.rs`, `service/ask/run.rs`, `service/ask/result.rs`): chạy bản `runnable` trên DuckDB, hai câu cùng lúc, dừng câu quá `QUERY_TIMEOUT_SECS` (mặc định 30 giây) và trần 1 GB cho phần bộ nhớ DuckDB quản lý; cột kiểu DuckDB chưa đọc được (TIME_NS, TIMETZ, BIT, BIGNUM, VARIANT, GEOMETRY, kiểu không rõ) báo lỗi để mô hình đổi sang VARCHAR; kết quả thành `ResultTable` (ô theo `kind`, 1000 dòng đầu, `truncated`, `limit_reached`); lỗi DuckDB hay guard từ chối vì bất kỳ lý do nào thì mô hình sửa một lần, câu sửa vẫn bị từ chối thì người hỏi nhận lời của hệ thống; lời báo lỗi không chứa đường dẫn file | Có, kèm test; endpoint hỏi (#33) gọi; trần 1 GB và hai câu cùng lúc chưa đo lại trên DataCo; hai giới hạn không chặn được biểu thức hằng DuckDB tính lúc chuẩn bị câu; guard thu hẹp bằng luật kích thước hàm, tối đa 4 tầng câu, 20 mảnh ghép, chuỗi hằng 1000 byte, 200 cột mỗi SELECT, 4 nguồn mỗi FROM, 10 nhánh UNION (ca một dòng kết quả đo dưới 270 MB); câu nhân theo số dòng (cả `UNION ALL` 10 nhánh) vẫn lên khoảng 1,4 đến 1,9 GB, bảo đảm thật cần tiến trình con (#119; [GUARD.md](GUARD.md), mục Hạn chế đã biết) |
| Ngữ cảnh câu hỏi (`service/ask/context.rs`, `service/ask/phrases.rs`, `service/ask/matching.rs`, `service/ask/values.rs`): cột gửi cho mô hình, danh sách giá trị của cột chiều kèm bí danh, gợi ý khớp chính xác, câu hỏi lại khi một cụm khớp tên nhiều cột; đọc hồ sơ của đúng phiên bản trong workspace người hỏi, theo hai tầng đầu của mục 5; cột nhạy cảm không kèm giá trị | Có hàm kèm test trên Postgres, gồm hồ sơ mẫu của DataCo; prompt (#29) và endpoint hỏi (#33) dùng; khớp gần, chịu gõ sai và tìm theo vector chưa làm |
| Giải thích câu "tại sao", phần toán (`service/explain/mod.rs`, `service/explain/ratio.rs`): đóng góp từng nhóm bằng số thập phân chính xác, xếp hạng chiều theo Adtributor, tách lượng và giá với giá hai chữ số làm tròn nửa xa số 0; độ đo tỷ lệ tách phần cơ cấu và phần hiệu suất, chiều xếp theo cách Effect. Đọc bảng sau khi xếp cho lời tóm tắt (`service/explain/annotate.rs`, #186), ra ghi chú nội bộ không gửi qua API và không đổi bảng phân rã: % tác động (phần của chênh lệch) và % thay đổi của từng nhóm, nhóm chỉ có ở một bên ("mới", "không còn"), nhóm khác thường (lệch % thay đổi chung từ 10 điểm và chiếm từ 1% chênh lệch, hay chỉ có ở một bên); chiều lời tóm tắt bỏ qua: mọi nhóm chỉ có ở một bên (tên khác của hai lát cắt, như hình thức thanh toán khi so trạng thái đơn) hay trùng nhóm một chiều khác (giữ chiều có giá trị chữ). Nhãn tiếng Việt của cột (`label`) lấy từ bí danh đầu tiên trong hồ sơ | Có hàm thuần kèm test; độ đo tỷ lệ chỉ nhận qua chỉ số trong metrics (form chưa có `avg`), mẫu số có NULLIF chưa nhận |
| Đường câu "tại sao" (`service/explain/run.rs`, `service/explain/form.rs`, `service/explain/check.rs`, `service/explain/sql.rs`, `service/explain/breakdown.rs`): bước phân loại (`service/ask/classify.rs`) nhận câu, luật từ khóa là đường dự phòng; mô hình điền form (độ đo hoặc chỉ số, hai kỳ tháng, quý, năm hoặc hai nhóm, bộ lọc `=`), code kiểm form với hồ sơ, `column_values` và `metrics`, sửa form một lần theo lỗi từng trường, sai lần hai thì chạy như câu thường kèm `explanation_unavailable`; code dựng bảng tổng theo kỳ, điều kiện hai lát cắt, SQL từng chiều cho mọi cột chiều từ 2 tới 50 giá trị (tối đa 30 cột ứng viên); `service/explain/dimension_pick.rs` bỏ chiều chỉ một nhóm, chiều chia y hệt một chiều khác (một câu SQL đếm cặp giá trị xác nhận, giữ cột chữ) và, khi so hai nhóm, chiều mà mọi nhóm chỉ có ở một bên (chiều bỏ vì trùng hay một bên vẫn vào ghi chú `dropped` của `service/explain/annotate.rs`, để lời tóm tắt có ý "Bỏ qua"); rồi chấm điểm và xếp các chiều còn lại, chiều giải thích rõ nhất trước (không cắt theo số chiều); và SQL lượng (một cột `key` lặp lại), mọi câu qua guard có lọc dòng theo vai dữ liệu; câu nói hướng ("giảm", "cao hơn") ngược chênh lệch tính được thì cảnh báo `premise_mismatch` (`service/explain/premise.rs`); câu so hai kỳ không nêu kỳ thì code đếm dòng theo kỳ và chọn kỳ gần nhất có đủ dữ liệu, ghi thành giả định đổi được `why:compare` (`service/explain/latest.rs`); dataset chụp từ database thì phân rã xét thêm cột chiều của các bảng nối nhiều-một qua `dataset_relations` (`service/explain/source.rs`, #227): code dựng câu con `LEFT JOIN` từ quan hệ đã ghi, chỉ đi từ bên nhiều lên bên một (tối đa 2 bước) như luật chặn cộng lặp, guard lọc dòng từng bảng được đọc, `tables_read` và `queries.read_versions` ghi cả bảng liên quan đã đọc | Có, kèm test trên Parquet tạm với mock và qua route; route hỏi gọi `run_why` và ghi form cùng `Explanation` vào `queries.explanation`; kỳ so sánh do người hỏi chọn mà không có dòng nào thì trả bảng tổng kèm `explanation_unavailable`, không gọi lại mô hình; lượng chưa lấy từ `metrics` vì bảng chưa có trường đánh dấu chỉ số lượng |
| Diễn giải bảng phân rã bằng lời (`service/ask/narrate.rs`, phần chữ ở `service/explain/mod.rs`): so hai kỳ (`comparison` là `periods`) thì mô hình viết đoạn tóm tắt từ `Explanation`, prompt có `comparison` và `main_group` (nhóm đóng góp lớn nhất cùng chiều với chênh lệch ở chiều đầu mà lời tóm tắt dùng); code kiểm mọi số trong đoạn có nguyên văn trong bảng (số kèm `%`, `‰`, "phần trăm", "percent", "pct", "gấp", "x" luôn sai, "lần" cũng vậy trừ khi độ đo đếm số lần; nhãn và giá trị nhóm chép nguyên văn được miễn), và chữ "tăng" hay "giảm" gần nhất trước mỗi chênh lệch phải đúng dấu của nó (`build_direction_check`). Lời tóm tắt do code viết (`service/explain/summary.rs`, #186): một câu chính (với hai nhóm có tên cột kèm nghĩa tiếng Việt) và các ý "Lý do chính" (số dòng hai bên khi trung bình mỗi dòng gần nhau, lượng và giá, cơ cấu và hiệu suất), "Khác thường theo <cột>" hay "Không có nơi nào đặc biệt", "Ngược chiều", "Bỏ qua <cột>", mỗi ý chỉ nói khi số xác nhận; trả trong `summary_title`, `summary_points`. Prompt narrate gửi đúng các ý này để mô hình chỉ diễn đạt lại, và gửi chiều theo ghi chú (không gửi chiều lời tóm tắt bỏ qua); phần trăm đã tính (sau dấu %) là số được phép. So hai nhóm (`groups`) hay chênh lệch bằng 0 thì không gọi mô hình, `summary` là lời do code viết (`build_template_summary`). Mô hình lỗi, quá giờ, sai số hay sai chiều thì cũng dùng câu khuôn, câu hỏi vẫn `answered` và bước `narrate` có `ok = false` kèm lý do | Có, kèm test; mock trả lời câu "tại sao" mẫu; `run_why` gọi sau bước explain, qua route hỏi |
| Đoạn trả lời bằng lời cho câu thường (`service/ask/summary.rs`, `AskResponse.answer_text`): mô hình viết một đến hai câu từ phần "Hệ thống đã hiểu là", tên cột và tối đa 10 dòng đầu của kết quả; code kiểm số như bước diễn giải câu "tại sao", sai, lỗi hay quá giờ thì dùng câu khuôn; bước hiện trong `steps` dưới tên `narrate`; tắt bằng `ANSWER_TEXT=false` | Có, kèm test; mock không có câu viết sẵn nên luôn trả câu khuôn; giao diện hiện đoạn này đầu câu trả lời, trước biểu đồ và bảng |
| Phần giải thích của câu thường theo hình dạng kết quả (`service/insight/`, `AskResponse.insight`, #227): một con số so với kỳ liền trước khi câu lọc đúng một tháng, quý hay năm; chuỗi thời gian nêu xu hướng chung, kỳ đổi mạnh nhất so với kỳ trước rồi phân rã đúng kỳ đó bằng đường của câu "tại sao" (plan dựng từ câu SQL đã chạy, không gọi mô hình điền form; tổng hai kỳ do code tính phải khớp ô của bảng, lệch thì bỏ phân rã), phân rã ở `AskResponse.explanation` và `queries.explanation`; theo nhóm nêu tỷ trọng nhóm đầu, số nhóm đạt 80% tổng, nhóm lệch hẳn mức trung bình; top N nêu tỷ trọng trên tổng (một câu tổng do code dựng, bỏ `LIMIT`) và chênh hạng đầu với hạng kế; danh sách dòng chỉ nêu số dòng và điều kiện lọc. Chênh lệch dưới 2% hay một bên dưới 30 dòng (`service/explain/significance.rs`) thì nói không có thay đổi đáng kể. Code viết câu chính và các ý; mô hình viết lại thành một đoạn trong một lần gọi thêm (`backend/prompts/insight.txt`), lời được kiểm số, chiều tăng giảm và từ chỉ nguyên nhân, sai thì dùng câu của code; `INSIGHT_TEXT=false` chỉ dùng câu của code; bước `explain` và `narrate` thứ hai trong `steps` | Có, kèm test từng hình dạng, trường hợp không đáng kể, bước kiểm lời, và trọn đường trên Parquet tạm hai bảng; chạy thử với Gemini trên database mẫu (`data/postgres-demo`): "Doanh thu theo tháng năm 2017 của thị trường Europe" nêu tháng 11/2017 giảm so với tháng 10 và nhóm Computers theo `categories.category_name`, số khớp bảng phân rã; giao diện hiện phần này trên thẻ trả lời (`components/ask/AnswerSummary.tsx`). Chưa qua cơ chế kiểm JSON có `source` của #201; chưa đo số lần gọi và thời gian thêm (#184); ngưỡng 2% và 30 dòng chưa đo trên dữ liệu; khóa nhóm và vế lọc thời gian chỉ nhận vài dạng (`date_trunc`, `strftime`, `year`, `month`, `quarter`, khoảng ngày), dạng khác thì không có phần so kỳ trước; mock không có lời viết sẵn nên luôn dùng câu của code; dòng phân rã theo cột của bảng liên quan chưa truy được dòng nguồn |
| Thẻ "Hệ thống đã hiểu là" và vai trò cột kết quả (`engine/sql_shape.rs`, `service/understanding/mod.rs`, `service/understanding/labels.rs`): đọc `sql_guarded` bằng cây cú pháp ra cột kết quả, vế lọc, khóa nhóm, thứ tự, LIMIT, bảng (đọc xuyên hộp phân quyền của guard); thẻ có nhãn tiếng Việt và đoạn SQL nguyên văn cho từng mục, cụm vế thời gian liền nhau đầu tiên thành `time_range`, điều kiện phân quyền nhãn "Phân quyền: ...", giả định là danh sách `Choice` của `service/ask/choices.rs` | Có hàm kèm test trên 15 câu SQL của mock qua guard; endpoint hỏi (#33) gọi cho câu thường đã answered, đọc cây không được thì trả lời không kèm thẻ và ghi log; câu có CTE, câu con ở FROM chỉ đọc câu ngoài cùng, UNION chỉ có thứ tự và LIMIT, `SELECT *` thì mọi cột là `other`; câu "tại sao" dựng thẻ từ form đã kiểm (`service/understanding/why.rs`): hai kỳ là `time_range`, hai nhóm là một mục lọc, mỗi đoạn SQL vẫn nguyên văn trong `sql_guarded` |
| Tổng quan dataset (#198, `service/overview/`, `GET /api/workspaces/{ws}/datasets/{id}/overview`, [API.md](API.md)): hồ sơ cột (nhãn là bí danh đầu, mô tả, vai trò, đơn vị, % ô trống, số giá trị khác nhau, số dòng theo giá trị của cột chia nhóm không nhạy cảm) và chỉ số đọc từ Postgres; biểu đồ dựng sẵn theo luật mục 6b: số dòng theo giá trị từ `column_values`, không chạy câu; tóm tắt (số dòng, ngày đầu, ngày cuối), độ đo chính (chỉ số đã xác nhận, không có thì cột số đo tiền hay số lượng) và số dòng theo ngày hay tháng là ba câu SQL do code dựng, qua guard với vai dữ liệu của người xem; kết quả giữ trong bộ nhớ (`AppState::overview_cache`, khóa là phiên bản và câu chạy, tối đa 256 kết quả), không thêm migration; người bị lọc dòng không nhận số đếm của hồ sơ | Có, kèm test phần thuần và qua route (cột nhạy cảm, lọc theo vai, chỉ số); không gọi mô hình. Giao diện: khung "Tổng quan dữ liệu" (`components/overview/`, `pages/OverviewPanel.tsx`) là nội dung của trang **Tổng quan dữ liệu**, một mục riêng trên thanh bên (`/w/:ws/overview?dataset=...`, `pages/OverviewPage.tsx`, `pages/DatasetOverviewSection.tsx`) có ô chọn dataset ở đầu trang; nút **Tổng quan** của từng dataset ở trang Dữ liệu mở trang này trên đúng dataset đó; bấm cột, giá trị, chỉ số hay biểu đồ thì mở trang cuộc trò chuyện với `?dataset=...&q=...`, câu hỏi điền sẵn vào ô hỏi, không gửi (`components/overview/questions.ts`) |
| Chọn biểu đồ theo IBCS (`service/ask/chart.rs`, mục 6b): loại số đo đọc từ hàm tổng hợp, đơn vị từ `columns.unit`, bảy luật kèm ngưỡng của IBCS | Có, kèm test phía backend; giao diện (`components/ask/ResultChart.tsx`, `chartData.ts`) vẽ thanh ngang xếp lớn trước, cột đứng (số đo không cộng được rộng 1/3), đường có điểm ở từng kỳ, cột chồng, cột đôi, mỗi số đo một biểu đồ nhỏ, phần "Khác"; chưa có thác nước cho câu "tại sao", thay vào đó là thanh hai phía cho đóng góp của từng nhóm ở chiều đầu (`components/ask/ContributionBars.tsx`); giao diện chưa có test tự động |
| Kiểm hợp lý kết quả (`service/ask/checks.rs`, `service/ask/magnitude.rs`): `empty_result`; `all_null` (xét dòng đã gửi, kết quả bị cắt thì lời ghi "trong n dòng đầu"); `filter_value_not_found` cho vế `cột = 'x'`, `cột IN (...)` và giá trị mô hình nêu trên cột chiều không nhạy cảm, gợi ý giá trị khác hoa thường, dấu, bí danh hay khác một hai chữ; `unusual_magnitude` cho tỷ lệ hai số đếm hay AVG cột 0/1 ngoài [0, 1], AVG, MIN, MAX ngoài [min, max] của hồ sơ, SUM cột không âm ra âm. Không kiểm điều kiện phân quyền; tra danh sách giá trị lỗi thì bỏ luật đó và ghi log | Có, kèm test từng luật và qua route; giao diện hiện cảnh báo phía trên kết quả; câu "tại sao" chỉ có `empty_result`; kiểm theo chuỗi thời gian và chỉ số dùng sai công thức (`build_metric_mismatches`) chưa làm |
| Pane Minh chứng của trang hỏi (`pages/AskEvidence.tsx`, `components/ask/Evidence*.tsx`), cột phải từ màn rộng, dưới khung trò chuyện khi màn hẹp hơn: bốn tab SQL (trước và sau guard), Phân rã (bảng đóng góp từng chiều, lượng và giá), Dòng nguồn (bấm số của một kỳ trong bảng phân rã hoặc một ô số đo của bảng kết quả), Các bước; SQL hiện xuống dòng theo mệnh đề, nút chép chép nguyên văn; pane chỉ hiện khi có câu trả lời, thu gọn được (nhớ trong localStorage); hai lát cắt, chênh lệch và lời giải thích chỉ ở câu trả lời, tab Phân rã không nhắc lại | Có, dựng trên hợp đồng ask; tab Dòng nguồn gọi route drill với khóa dòng phân rã hoặc khóa của ô (giá trị các cột `dimension`, `time` của dòng), ghi đang xem ô nào, tổng số dòng nguồn và phân trang theo `offset`, cột khóa và cột nguồn của số đo đứng đầu bảng dòng nguồn; `npm run dev` có dòng mẫu; điều kiện phân quyền guard thêm chưa tách riêng trong response nên chưa chỉ ra từng điều kiện |
| Cuộc trò chuyện (#164, `service/conversation/`, migration `0008_conversations.sql`): bảng `conversations`, `queries.conversation_id` và `queries.understood`; ask nhận và trả `conversation_id`; prompt câu thường và prompt form câu "tại sao" nhận khối "Hội thoại trước" (tối đa 4 lượt: câu hỏi, trạng thái, SQL, phần đã hiểu; trần 4 000 ký tự); route liệt kê và mở cuộc của chính người gọi; trang hỏi là khung trò chuyện (bong bóng câu hỏi, khối trả lời, ô hỏi dính đáy), pane Minh chứng theo lượt được chọn; danh sách cuộc cũ trong khung nổi "Lịch sử trò chuyện" mở từ thanh bên (`components/shell/ConversationFlyout.tsx`, `pages/SidebarConversations.tsx`; dataset của trang hỏi, nút dấu cộng mở cuộc mới); đường dẫn `?dataset=...&c=...` mở đúng cuộc khi tải lại và khi chọn từ khung nổi | Có, kèm test qua route với mô hình giả (khối lịch sử có trong prompt, không có giá trị kết quả; người khác nhận 404); thẻ "Hệ thống đã hiểu là" ghi "từ câu trước" cạnh mục giống lượt trước mà câu hỏi không nhắc tới (`service/conversation/carryover.rs`, trường `from_previous`); câu "tại sao" thiếu kỳ dùng kỳ gần nhất có dữ liệu làm giả định đổi được; mở lại cuộc cũ chỉ hiện câu hỏi và tóm tắt "Hệ thống đã hiểu là" của từng lượt kèm nút **Hỏi lại**, vì backend không lưu bảng kết quả |
| Chế độ demo (#46): route `POST /api/auth/demo-login` có trong đặc tả | Chưa có; route trả 501 (`not_implemented`) |
| Dữ liệu mẫu đóng gói sẵn (`service/samples.rs`, `service/samples_db.rs`, [data/README.md](https://github.com/pgiahuy/xdi-platform/blob/main/data/README.md)): compose chạy database mẫu nhỏ `demo-db` (13 bảng `sales` và `edu` nạp từ `data/samples/`, #230); thiết lập lần đầu thêm nó làm nguồn (`SAMPLE_DATABASE_URL`) và chụp các nhóm bảng của `sample_database.json` kèm quan hệ và chỉ số mẫu; không có hay chụp lỗi thì nạp `SAMPLES_DIR/dataco.csv` thành dataset `DataCo` qua đường nạp thường, rồi ghi chỉ số của `dataco.metrics.json` (`origin` `sample`, đã xác nhận) sau khi chạy thử từng chỉ số qua guard; `SEED_SAMPLE=false` tắt; `data/download.sh` tải bản đầy đủ DataCo, Online Retail II và OULAD (dữ liệu giáo dục), kiểm SHA-256 | Có, kèm test trên Postgres (chụp database mẫu kèm quan hệ và chỉ số, không tới được thì nạp file, nạp một lần, tắt, thiếu file, chỉ số chạy lỗi bị bỏ); fresh install bằng `docker compose up` đã thử trên máy dev; bản cài đã thiết lập trước đó không được nạp mẫu; Online Retail II và OULAD chưa có chỉ số mẫu |
| Bộ đánh giá câu hỏi mẫu (`service/eval.rs`, lệnh `xdi-backend eval`, `make eval`): tạo workspace tạm; lượt đầu chụp database mẫu như thiết lập lần đầu (`--sample-database`) và hỏi `eval/sales_joins.json`, `eval/edu.json` trên đúng dataset ghi ở từng câu, lượt sau nạp `data/samples/dataco.csv` và hỏi `eval/dataco.json`; mỗi câu qua đúng luồng hỏi, so tập dòng kết quả với SQL đúng viết tay (cũng qua guard), ghi `eval/result.json`, thoát mã 1 khi tỷ lệ đúng dưới `eval/baseline.json` | Có; 18 câu trên database mẫu và 14 câu trên DataCo, mock đúng cả 32; `make eval` và CI luôn chạy với mock; chạy với mô hình thật thì gọi thẳng `xdi-backend eval` kèm `LLM_PROVIDER` ([RELEASING.md](RELEASING.md)) |
| Màn Chỉ số (#185, `service/metrics/`, `api/metrics.rs`, `pages/MetricsPage.tsx`, [API.md](API.md)): mọi thành viên xem chỉ số và cột của phiên bản hiện hành; admin tạo, sửa (thêm phiên bản, `origin` `ui`, đã xác nhận), xóa chỉ số bằng chọn phép tính và cột hoặc viết SQL, và đổi cách gộp của cột số đo; công thức chạy thử qua guard và DuckDB trước khi lưu, lỗi trả theo từng trường (`ErrorBody.field`). Prompt sinh SQL ghi cột `avg` phải gộp bằng AVG; form câu "tại sao" đổi tổng của cột `avg` thành trung bình mỗi dòng và phân rã như tỷ lệ; dòng "Đo" của thẻ "Hệ thống đã hiểu là" ghi cách tính bằng lời khi chỉ số không có mô tả | Có, kèm test API (quyền, kiểm công thức, phiên bản, prompt); đã chạy một lần với Gemini trên OULAD (`studentInfo.csv`): câu "Số tín chỉ theo kết quả cuối khóa", không nói "trung bình", ra `AVG("studied_credits")`; cách gộp admin đặt chưa chép sang phiên bản nạp sau |
| Phân loại câu hỏi (`service/ask/classify.rs`, #197) và bộ câu thử (`service/eval_classify.rs`, lệnh `xdi-backend eval-classify`, `make eval-classify`): 48 câu trên DataCo trong `eval/classify.json` (20 câu hỏi tìm nguyên nhân, 22 câu thường, 6 câu tiếp nối kèm câu hỏi trước), so loại câu viết tay với luật từ khóa và với bước `classify`, in trung vị thời gian của bước `classify` | Có, kèm test với mô hình giả và qua route; không có baseline, không chạy trong CI. Đo ngày 28/10/2026 với `gemini-3.5-flash-lite`, hai lượt, nghỉ 4 giây giữa hai câu: luật từ khóa đúng 32/48, bước `classify` đúng 48/48 ở cả hai lượt, không câu nào rơi về luật từ khóa; trung vị thời gian bước `classify` 1320 ms và 1173 ms (thấp nhất 879 ms, cao nhất 12793 ms). Bước này chạy nối tiếp trước bước viết SQL hay điền form, nên thời gian của nó là phần thêm vào mỗi câu hỏi. Ví dụ trong prompt khác các câu của bộ thử, nhưng cả hai do cùng người viết |
| Đánh giá câu trả lời và câu mẫu (#212, `service/feedback/`, `api/feedback.rs`, `pages/ExamplesPage.tsx`, [API.md](API.md)): nút Tốt, Chưa đạt (lý do, ghi chú) dưới mỗi câu trả lời, hiện lại khi mở cuộc trò chuyện; admin duyệt câu Tốt làm câu mẫu, ghi người duyệt và thời điểm; chỉ câu đã duyệt vào prompt SQL (câu thường) hay prompt form (câu "tại sao"); bước `llm` ghi câu mẫu đã dùng; xuất câu mẫu thành ca ứng viên cho bộ đánh giá | Có, kèm test API cho cả hai vai và test prompt; ca xuất ra chưa được đưa vào file ca nào |
| Màn xem lịch sử câu hỏi, sửa mô tả cột và bí danh | Chưa có |
