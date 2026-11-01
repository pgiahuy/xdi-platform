# Kiến trúc

Hệ thống chia phần thế nào và vì sao. Phần nào đã có, phần nào chưa: mục Trạng thái ngay dưới đây.

## Thuật ngữ

| Thuật ngữ | Nghĩa |
|---|---|
| Dataset | Một database đã kết nối hay một file đã nạp; bảng nằm bên trong ([SCHEMA.md](SCHEMA.md#điểm-hay-hiểu-nhầm)) |
| Câu hỏi số liệu | Câu hỏi một con số, một bảng hay một danh sách: mô hình viết SQL |
| Câu hỏi nguyên nhân | Câu hỏi vì sao một số tăng, giảm hay khác nhau giữa hai nhóm: mô hình điền form, code dựng SQL |
| Cách tính | Thẻ dưới câu trả lời: hệ thống đã đo gì, lọc gì, nhóm theo gì, kể cả điều kiện lọc theo quyền xem |
| Lời giải thích | Đoạn lời viết từ các số code đã tính; giao diện ghi "giải thích", API gọi trường `insight` |
| Phân rã | Bảng chia chênh lệch thành phần của từng nhóm; tổng các phần bằng đúng chênh lệch |
| Dòng nguồn | Các dòng dữ liệu đứng sau một con số, mở bằng cách bấm vào số đó |
| Minh chứng | Những gì cho thấy con số đến từ đâu: các ý code đã tính dưới lời, SQL, phân rã, dòng nguồn, các bước |
| Kiểm số | Code so từng số, chiều tăng giảm và từ ngữ trong lời với bảng đã tính |
| Kiểm nghĩa | Một lần gọi mô hình riêng chấm nghĩa của lời so với các số đã khai |

## Trạng thái

| Phần | Trạng thái |
|---|---|
| Tài khoản, phiên, mã thiết lập, quản trị hệ thống (mục 5) | Có |
| Workspace, vai `admin` và `member` (mục 5) | Có |
| Vai dữ liệu: luật lọc dòng, nhiều vai mỗi người, API và màn Vai dữ liệu (mục 5) | Có |
| Nạp file CSV, Parquet, Excel ([DATA.md](DATA.md)) | Có; chưa xóa riêng một phiên bản hay chọn lại phiên bản cũ |
| Kết nối Postgres, chụp mọi bảng đọc được, trần dung lượng, chụp lại theo lịch ([DATA.md](DATA.md)) | Có; chưa có MySQL, SQLite, truy vấn thẳng lúc hỏi |
| Hồ sơ cột lớp cấu trúc và lớp ngữ nghĩa, cột nhạy cảm ([DATA.md](DATA.md)) | Có; chưa có màn sửa mô tả, bí danh, vai trò cột |
| Hỏi trên mọi bảng của database, chọn bảng gửi chi tiết (mục 7) | Có; chọn bảng theo embedding chưa làm |
| Guard SQL, nối bảng chỉ theo khóa ngoại ([GUARD.md](GUARD.md)) | Có |
| Chặn cộng lặp khi nối bảng ([GUARD.md](GUARD.md#kiểm-cộng-lặp-sau-guard)) | Có; chưa xét `COUNT(*)` và bảng tạm đã nối |
| Câu hỏi số liệu: bảng, biểu đồ, Cách tính, lời giải thích qua kiểm số và kiểm nghĩa | Có |
| Câu hỏi nguyên nhân: form, phân rã, lượng và giá, tỷ lệ, lời qua kiểm số và kiểm nghĩa | Có; chưa có biểu đồ thác nước |
| Cuộc trò chuyện, lịch sử của chính người hỏi | Có; chưa có màn xem lịch sử của cả workspace |
| Minh chứng: SQL, phân rã, dòng nguồn kể cả câu nối bảng, các bước | Có; chưa truy được câu có HAVING, hàm cửa sổ, bảng tạm gộp nhóm |
| Chọn biểu đồ theo IBCS (mục 8) | Có |
| Tổng quan dataset, Chỉ số, Khái niệm ngành ([DOMAINS.md](DOMAINS.md)) | Có |
| Đánh giá câu trả lời, câu mẫu, nhật ký lỗi, bài học ([API.md](API.md)) | Có |
| Báo cáo tự gom và báo cáo tự động, xuất PDF và Excel | Có; chưa chia sẻ báo cáo cho người khác |
| Mô hình `mock`, Gemini (kèm khóa dự phòng), Ollama ([INSTALL.md](INSTALL.md)) | Có |
| OpenAPI sinh từ code, kiểu TypeScript sinh từ đặc tả (mục 10) | Có |
| Chạy thật: compose prod, cookie `Secure`, header bảo mật, sao lưu ([INSTALL.md](INSTALL.md)) | Có; chưa có role Postgres riêng cho Row Level Security |
| Bộ đánh giá ([eval/README.md](https://github.com/pgiahuy/xdi-platform/blob/main/eval/README.md)) | Có; CI chỉ chạy phần DataCo với `mock` |
| Chế độ demo (`POST /api/auth/demo-login`) | Chưa có; route trả 501 |

## 1. Sơ đồ tổng thể

```
TRÌNH DUYỆT  React, Vite, Tailwind, shadcn/ui, Recharts; kiểu TypeScript sinh từ openapi.json
     |  REST JSON, cùng địa chỉ (/api)
BACKEND  Rust, Axum, một container, phục vụ cả giao diện
     api/      handler, request/response, kiểm phiên, OpenAPI
     service/  ask, explain, insight, drill, sources, report, ...
     engine/   store/ (sqlx)  -------------------->  POSTGRES: dữ liệu ứng dụng (SCHEMA.md)
               analytics.rs, query.rs (DuckDB nhúng) ->  VOLUME /data/<workspace>/<dataset>/<bảng>/<phiên bản>/*.parquet
               guard/ (sqlparser, danh sách được phép, lọc dòng)
               llm/ (trait LlmProvider)  ----------->  Gemini API | Ollama | mock
     auth/     Argon2, mã phiên, mã thiết lập, WorkspaceId

Postgres của tổ chức: chỉ đọc, lúc kết nối và chụp bảng
```

Mặc định có hai container, `backend` và `postgres`; `docker compose --profile demo` thêm `demo-db` (database mẫu,
[data/postgres-demo](https://github.com/pgiahuy/xdi-platform/blob/main/data/postgres-demo/README.md)). Giao diện cùng
địa chỉ với API nên không cần CORS. DuckDB là thư viện trong backend, không có tiến trình riêng.

## 2. Luồng một câu hỏi

Sơ đồ đủ 16 bước: [images/luong-cau-hoi.png](images/luong-cau-hoi.png). Tóm tắt:

```
1. api        kiểm phiên, lấy workspace, vai trong workspace, dataset và bảng bắt đầu
2. context    đọc hồ sơ cột, giá trị phân loại, quan hệ, chỉ số, câu mẫu và bài học đã duyệt, vai dữ liệu (Postgres)
3. classify   mô hình nói câu hỏi số liệu hay câu hỏi nguyên nhân, kèm cột và bảng cần; lỗi thì dùng luật từ khóa
4. llm        câu hỏi số liệu: mô hình trả JSON {terms, assumptions, clarify, unanswerable, sql} trên tên bảng logic
              câu hỏi nguyên nhân: mô hình điền form JSON, code kiểm form và tự dựng mọi SQL
5. guard      chỉ một SELECT chỉ đọc, bảng, cột, hàm được phép, nối đúng khóa ngoại, lọc dòng theo vai, trần dòng
6. run        DuckDB chạy trên Parquet của phiên bản đang dùng; lỗi hay guard từ chối thì mô hình sửa một lần
7. explain    câu hỏi nguyên nhân: phân rã; câu hỏi số liệu: code chọn ý theo hình dạng kết quả, phân rã khi cần;
              mô hình viết lời giải thích, qua kiểm số và kiểm nghĩa
8. record     ghi queries: câu hỏi, SQL trước và sau guard, các bước, phiên bản mọi bảng đã đọc, lời đã hiện
9. api        trả một JSON: kết quả, Cách tính, biểu đồ, cảnh báo, phân rã, lời, các bước
```

Lúc hỏi, mô hình ngôn ngữ làm các việc sau, mỗi việc là một lần gọi:

- Phân loại câu hỏi (`service/ask/classify.rs`). Mô hình lỗi, quá `CLASSIFY_TIMEOUT_SECS` hay trả JSON không đọc được
  thì luật từ khóa quyết định và bước `classify` ghi lý do.
- Viết SQL kèm cách hiểu từng cụm chữ, hoặc điền form của câu hỏi nguyên nhân. Câu trả lời là JSON theo schema gửi
  kèm; prompt có luật của guard sinh từ chính guard.
- Gửi lại một lần khi cần: SQL chạy lỗi, guard từ chối, câu trả lời không đọc được, form sai
  (`service/ask/repair.rs`); SQL đọc bảng mới có tên trong danh sách gọn (`service/ask/missing_tables.rs`); giá trị lọc
  không có trong danh sách đã gửi (`service/ask/value_retry.rs`).
- Viết lời giải thích từ các số code đã tính: phân rã của câu hỏi nguyên nhân (`service/ask/narrate.rs`), các ý của
  câu hỏi số liệu (`service/insight/narrate.rs`, tắt bằng `INSIGHT_TEXT=false`).
- Kiểm nghĩa lời đó (`service/ask/judge.rs`). Trượt kiểm số hay kiểm nghĩa thì viết lại một lần; vẫn trượt thì dùng
  câu của code. Lời qua kiểm cũng là đoạn trả lời ngắn ở đầu câu trả lời (`service/ask/summary.rs`).

Mô hình không nhận dòng của bảng kết quả, trừ kết quả một dòng mà mọi ô là số tổng hợp hay giá trị của cột phân loại,
thời gian không nhạy cảm. Nó không biết vai dữ liệu của người hỏi; prompt chỉ nói hệ thống tự lọc theo quyền. Danh sách
đủ những gì gửi đi ở từng lần gọi: [PRIVACY.md](PRIVACY.md).

Chữ cố định của prompt nằm trong `backend/prompts/`, mỗi prompt một file, nhúng vào binary; `engine/prompts.rs` điền
chỗ trống `{{tên}}`, mỗi prompt có file mẫu so từng byte trong `backend/src/service/testdata/`. Riêng prompt viết lời câu
hỏi nguyên nhân dựng trong `service/explain/narrate_prompt.rs`. Bản tiếng Việt cho cả đội đọc ở `backend/prompts/vi/`,
không gửi cho mô hình.

## 3. Ba tầng, phụ thuộc một chiều

```
backend/src/
  main.rs     khởi động: đọc config, mở Postgres, dựng AppState và router; hoặc chạy lệnh quản trị (cli.rs),
              tiến trình con đọc Excel (engine/excel_worker.rs)
  config.rs   biến môi trường
  api/        tầng HTTP: handler, request/response struct, lỗi thành mã HTTP
  service/    tầng nghiệp vụ, không biết gì về HTTP: ask/, explain/, insight/, drill/, sources/, catalog/,
              conversation/, report/, data_roles/, metrics/, semantic/, understanding/, overview/, feedback/,
              lessons/, domains/, cùng ingest.rs, samples*.rs, eval*.rs
  engine/     tầng hạ tầng: analytics.rs, query.rs (DuckDB), excel.rs, store/ (Postgres, chia theo bảng),
              guard/ (GUARD.md), llm/, prompts.rs
  auth/       mật khẩu, mã phiên, mã thiết lập, vai, kiểu WorkspaceId
  error.rs    một enum lỗi chung
```

- `api` chỉ gọi `service`. `service` gọi `engine` và `auth`. Không có chiều ngược.
- Trait duy nhất là `LlmProvider` (`name`, `complete`, `complete_with_usage` trả kèm số token). Implementation chọn lúc
  khởi động theo `LLM_PROVIDER`: `gemini`, `ollama` hay `mock`. Mock trả câu viết sẵn cho bộ câu hỏi mẫu, dùng khi
  không có khóa và trong test.
- Postgres và DuckDB không bọc trait. Service nhận struct `Store`; câu SQL của nó viết bằng `sqlx::query_as` (không
  macro `query!`, build không cần Postgres) và được kiểm bằng test trên Postgres thật. Mỗi lần nạp hay chạy câu mở một
  kết nối DuckDB trong bộ nhớ.
- Trạng thái chia sẻ qua `AppState`: config, `Arc<dyn LlmProvider>`, `Store`, bộ nhớ đệm giá trị cột, các semaphore
  giới hạn việc chạy cùng lúc. Không biến toàn cục.
- Test ở tầng service và qua route (`api/*_tests.rs`), với mock LLM, DuckDB trong bộ nhớ và Postgres thật.

## 4. Hai kho dữ liệu

| Kho | Chứa | Vì sao |
|---|---|---|
| Postgres (`backend/migrations/`) | Dữ liệu ứng dụng: tài khoản, workspace, dataset và bảng, phiên bản, hồ sơ cột, chỉ số, vai dữ liệu, lịch sử câu hỏi, báo cáo ([SCHEMA.md](SCHEMA.md)) | Nhiều ghi nhỏ, nhiều kết nối, đúng việc của database giao dịch |
| DuckDB nhúng | Không giữ gì bền; đọc Parquet trên volume theo từng câu SQL | Engine phân tích theo cột, SQL chuẩn, đọc thẳng Parquet, không thêm tiến trình |

Image Postgres là `pgvector/pgvector`, có sẵn `unaccent`, `pg_trgm`, `vector`.

## 5. Workspace và phân quyền

Một bản cài phục vụ một tổ chức. Workspace là phòng ban trong tổ chức đó, có dataset, chỉ số và vai riêng, vì các
phòng thường tính chỉ số khác nhau. Lần thiết lập đầu tạo workspace đầu tiên, người thiết lập là admin của nó. Tài khoản
tạo sau chỉ vào được workspace khi admin thêm, hoặc tự tạo workspace riêng.

| Tầng | Câu hỏi | Ví dụ |
|---|---|---|
| Workspace | Thuộc phòng ban nào | Người phòng kinh doanh không thấy dataset, chỉ số, lịch sử của phòng tài chính |
| Vai trong workspace | Được làm gì | `admin` nạp dữ liệu, quản lý thành viên; `member` xem và hỏi |
| Vai dữ liệu | Thấy dòng nào | Thành viên mang vai "Châu Âu" chỉ thấy `orders.market = Europe`; guard thêm điều kiện vào SQL |

**Cách chặn giữa workspace.** Mọi bảng nghiệp vụ có `workspace_id`. Mọi hàm trong `engine/store/` nhận kiểu
`WorkspaceId`, chỉ tạo được từ phiên đăng nhập đã kiểm, nên quên lọc theo workspace là lỗi biên dịch. Khóa ngoại giữa
các bảng nghiệp vụ gồm cả `workspace_id`, nên một dòng không trỏ được sang workspace khác. Postgres Row Level Security
là lớp thứ ba dự định thêm sau. File Parquet nằm dưới `/data/<workspace>/`; prompt chỉ chứa hồ sơ của workspace đang hỏi.

**Vai dữ liệu.** Một vai (`data_roles`) có tên, mô tả và các luật (`rls_policies`: một bảng, một cột chiều, các giá trị
được thấy); một thành viên mang được nhiều vai (`member_data_roles`). Admin đặt qua API `/data-roles` ([API.md](API.md))
hay màn **Vai dữ liệu**. Luật ghép:

- Trong một vai, các luật trên một bảng và điều kiện lan theo khóa ngoại nối AND. Giữa các vai nối OR, chỉ tính vai có
  điều kiện trên bảng đang đọc hay bảng nó trỏ tới (vai chỉ có luật trên `products` không mở thêm dòng `customers`).
- Bảng trỏ tới một bảng bị lọc nhận điều kiện lan theo khóa ngoại ([GUARD.md](GUARD.md#điều-kiện-lan-theo-khóa-ngoại)).
- Không mang vai nào, hay không vai nào có điều kiện trên bảng, thì thấy mọi dòng. `admin` không bị lọc dù mang vai.
- Chỉ gán được vai đã có luật; vai đang gán không xóa được và không được bỏ hết luật; hai vai không được trùng đúng bộ
  luật. Vai và luật đọc lại mỗi request, nên đổi có hiệu lực ngay request kế tiếp.

Lọc áp ở mọi chỗ chạy SQL trên dữ liệu (`load_row_filters` trong `service/ask/row_filters.rs`): hỏi, phân rã, truy dòng
nguồn, tổng quan, báo cáo. Thẻ Cách tính ghi điều kiện đó với nhãn "Lọc theo quyền xem". Người bị lọc không nhận danh
sách giá trị cột trong prompt, vì danh sách lấy từ hồ sơ của cả cột.

**Đăng nhập** luôn bật. Lần chạy đầu backend in ra log một mã thiết lập; nhập đúng mã thì tạo được tài khoản quản trị
kèm workspace đầu tiên. Mật khẩu băm bằng Argon2id. Phiên lưu trong Postgres (`sessions` giữ SHA-256 của mã phiên), mã
phiên trong cookie httpOnly; không dùng JWT để đăng xuất, khóa tài khoản, đổi vai có hiệu lực ngay request kế tiếp.

**Quản trị hệ thống** (`users.is_system_admin`) là thuộc tính tài khoản: tạo, khóa, mở tài khoản, đặt lại mật khẩu,
xem mọi workspace kèm thành viên. Họ không tự vào được dữ liệu của workspace mình không thuộc (404), nhưng chỉ định
được admin cho bất kỳ workspace nào, kể cả chính mình; mỗi lần chỉ định ghi một dòng log cảnh báo, chưa lưu vào
database. Quản trị duy nhất quên mật khẩu thì chạy `xdi-backend reset-password --email <email>` trên máy chủ.

**Vai trong workspace.** Người tạo workspace là `admin`; workspace còn ít nhất một `admin`, kể cả khi hai admin hạ vai
nhau cùng lúc. Admin thêm thành viên theo email (tài khoản phải đang hoạt động), nên người đăng nhập biết được một email
có tài khoản hay không; chấp nhận vì tài khoản là của người trong cùng tổ chức, người bị thêm tự rời được.

| Việc | `admin` | `member` |
|---|---|---|
| Xem dataset, hỏi, xem minh chứng, xem lại cuộc trò chuyện của mình | Có | Có |
| Xem danh sách thành viên, chỉ số, khái niệm ngành; xem vai dữ liệu của mình | Có | Có |
| Đánh giá Tốt, Chưa đạt câu trả lời của mình; tạo báo cáo của mình | Có | Có |
| Tự rời workspace | Có, nếu còn admin khác | Có |
| Nạp file, kết nối và chụp database, xóa bảng và nguồn | Có | Không (403) |
| Đổi tên, xóa workspace; thêm, đổi vai, xóa thành viên | Có | Không (403) |
| Tạo, sửa, xóa chỉ số; đổi cách gộp cột số đo; sửa khái niệm ngành | Có | Không (403) |
| Tạo vai dữ liệu, đặt luật, gán vai | Có | Không (403) |
| Duyệt câu mẫu, xem nhật ký lỗi, viết và bật bài học | Có | Không (403) |

Việc chỉ admin làm được gọi `member.require_admin()` ở đầu service hay handler, kèm test cho cả hai vai.

## 6. Dataset, bảng và nạp dữ liệu

Một dataset là một database đã kết nối hay một file đã nạp; bảng nằm bên trong. Trong Postgres dòng `sources` là dataset
và dòng `datasets` là bảng ([SCHEMA.md](SCHEMA.md#điểm-hay-hiểu-nhầm)). Gắn theo dataset: tên, gói ngành, tên chỉ số
(không trùng trong một dataset), câu mẫu, bài học. Gắn theo bảng: phiên bản, hồ sơ cột, quan hệ, luật lọc dòng, lịch sử
câu hỏi, nên câu hỏi cũ và dòng nguồn trỏ đúng phiên bản bảng đã đọc.

Mô hình viết SQL trên tên bảng logic; guard đổi tên đó thành thư mục Parquet của phiên bản đang dùng
(`<DATA_DIR>/<workspace>/<dataset>/<bảng>/<phiên bản>/`, mỗi file gốc một `part-NNNN.parquet`, cột `_source_file` ghi
file hay sheet gốc). Bảng Postgres được chụp thành Parquet như file nạp, nên guard, prompt và lịch sử không phân biệt hai
nguồn:

- Nạp file: CSV, Parquet, Excel (sheet đổi sang CSV bằng `calamine` trong một tiến trình con có trần RAM và CPU). Nhiều
  file cùng cột gộp thành một bảng.
- Chụp database: mặc định DuckDB đọc thẳng Postgres bằng extension `postgres` đóng gói sẵn (`SOURCE_SNAPSHOT_MODE=direct`);
  không nạp được extension thì dùng `COPY` ra CSV (`copy`). Mọi bảng đọc trong một giao dịch `REPEATABLE READ`. Bảng quá
  trần dung lượng chỉ lưu cấu trúc. Khóa ngoại khai báo giữa hai bảng đã chụp ghi vào `dataset_relations`; không đoán
  quan hệ theo tên cột và không nối thành bảng phẳng (cột phía một sẽ lặp trên mọi dòng phía nhiều và bị cộng nhầm).

Định dạng nhận, trần, cách chụp, phiên bản, tên cột, hồ sơ cột: [DATA.md](DATA.md). Truy vấn thẳng database ngoài lúc
hỏi chưa làm, cần quyết định riêng.

## 7. Ngữ cảnh và hỏi trên nhiều bảng

**Phạm vi.** Câu hỏi trên dataset database đọc được mọi bảng đã chụp, không giới hạn số bước theo quan hệ; dataset
file chỉ có một bảng. Câu vẫn có một bảng bắt đầu: người hỏi ghim (`table_id`), không thì `pick_start_table`
(`service/ask/scope.rs`) chọn bảng giữ cột số đo câu hỏi gọi tên, rồi bảng khớp tên, rồi khớp mô tả, rồi bảng của cuộc
đang tiếp.

**Bảng gửi chi tiết** (`service/ask/table_pick.rs`). Tổng metadata cột mọi bảng tới nửa trần prompt thì gửi đủ mọi bảng
(hai database mẫu ở trường hợp này). Vượt thì gửi chi tiết bảng bắt đầu, bảng khớp chữ, bảng lượt phân loại chọn, bảng
giữa trên đường khóa ngoại ngắn nhất, rồi bảng nối một bước khi còn dưới trần. Bảng còn lại và bảng chưa chụp chỉ có tên
và một dòng mô tả trong danh sách gọn. SQL đọc bảng chỉ có trong danh sách gọn thì gửi lại một lần kèm chi tiết bảng đó.
Bước `context` ghi bảng nào đã gửi chi tiết và vì sao.

**Cột và giá trị** (`service/ask/context.rs`). Mọi cột của các bảng gửi chi tiết có metadata đủ; không bỏ cột nào khi
prompt dài, chỉ rút cột không liên quan về dòng gọn. Danh sách giá trị của cột chiều không nhạy cảm gửi đủ khi tổng còn
dưới trần, vượt thì chỉ cột liên quan. Một cụm khớp tên nhiều cột thì không hỏi lại: dùng cột điểm cao nhất và ghi các
cột còn lại thành giả định đổi được. Các trần cụ thể ghi ở chú thích đầu `context.rs`, `prompt.rs`; số gửi đi ở
[PRIVACY.md](PRIVACY.md).

**Nối bảng.** Prompt có khối cột mỗi bảng, danh sách quan hệ và luật nối (`backend/prompts/sql_joins.txt`). Guard chỉ
nhận phép nối đúng cột của một khóa ngoại đã ghi ([GUARD.md](GUARD.md#bước-2-bảng-hàm-cột-theo-danh-sách-được-phép)).
Sau guard, `service/ask/fanout.rs` bắt câu SUM hay AVG cột của bảng phía một sau khi nối bảng phía nhiều; câu đó vào
vòng sửa, sửa vẫn cộng lặp thì bị từ chối. Câu không trả lời được mà hai bảng không nối được qua chuỗi khóa ngoại nào
thì lời báo nói hai bảng không có quan hệ. Response trả các bảng đã đọc (`tables_read`), lịch sử ghi phiên bản của
chúng (`queries.read_versions`).

**Câu hỏi nguyên nhân.** Form chỉ nêu cột của bảng bắt đầu; phân rã xét thêm cột chiều của bảng nối nhiều-một, tới hai
bước (`BREAKDOWN_MAX_HOPS`, `service/explain/source.rs`).

**Câu mẫu và bài học.** Tối đa 3 câu mẫu admin đã duyệt có từ khóa trùng với câu hỏi, và tối đa 3 bài học đã bật, vào
prompt.

## 8. Chọn biểu đồ

Luật theo IBCS (International Business Communication Standards) và *Say It With Charts* của Gene Zelazny. Chọn đúng
phải biết bốn điều về kết quả: so sánh gì, số đo cộng được không, bao nhiêu nhóm hay kỳ, các số đo cùng đơn vị không.
`service/ask/chart.rs` đọc vai trò cột, hàm tổng hợp trong SQL đã chạy và `columns.unit`; giao diện vẽ từ chính các
dòng của kết quả. Biểu đồ của trang tổng quan đi qua cùng hàm.

1. Một khóa là nhóm: thanh ngang, xếp lớn trước. Quá 15 nhóm thì vẽ 15 nhóm lớn nhất, số đo cộng được và đủ dòng thì
   phần còn lại gộp "Khác". IBCS không đặt số cho thanh đơn; 15 chọn ở đây.
2. Một khóa là thời gian: thời gian trên trục ngang. Tới 10 kỳ là cột đứng, hơn 10 kỳ là đường có điểm ở từng kỳ.
3. Một dòng: không vẽ, bảng đã hiện con số.
4. Thời gian và một nhóm: số đo cộng được và tới 25 kỳ thì cột chồng, tối đa 6 phần (5 nhóm lớn nhất cộng "Khác"). Số đo
   không cộng được hay quá 25 kỳ thì mỗi nhóm một đường khi không quá 5 nhóm; nhiều hơn thì chỉ bảng. Không dùng biểu
   đồ tròn.
5. Câu hỏi nguyên nhân: vẽ từ phân rã, không qua `chart`. Giao diện hiện thanh hai phía cho đóng góp từng nhóm; thác
   nước chưa làm.
6. Nhiều số đo khác đơn vị hay chưa biết đơn vị: mỗi số đo một biểu đồ nhỏ, chung trục, không dùng hai trục.
7. Nhiều số đo cùng đơn vị: cạnh nhau trên một thang đo.

Chung: cột và thanh bắt đầu từ 0; số đo không cộng được vẽ cột hẹp và không gộp "Khác"; tối đa 4 số đo; hai khóa đều là
nhóm, hay ba khóa trở lên, thì chỉ bảng.

## 9. Phiên bản

Mỗi lần nạp file, hay chụp lại một bảng có nội dung đổi, là một phiên bản mới, không ghi đè. Nội dung trùng hash thì
không tạo phiên bản. `queries` ghi phiên bản của mọi bảng đã đọc, nên câu hỏi cũ, dòng nguồn và mục báo cáo chạy lại
đúng dữ liệu lúc hỏi. Nguồn database chụp lại khi admin bấm hay theo lịch (`SOURCE_RESNAPSHOT_HOURS`).

Chưa có: so schema giữa hai phiên bản (cột mất, thêm, đổi kiểu) trước khi đổi phiên bản hiện hành, và cảnh báo khi luật
lọc dòng trỏ tới giá trị không còn. Hiện phiên bản mới thành hiện hành ngay.

## 10. API và hợp đồng

REST và JSON, dưới `/api`. API dữ liệu nằm dưới workspace trong đường dẫn (`/api/workspaces/{ws}/catalog/...` cho
dataset, `/api/workspaces/{ws}/datasets/{id}/...` cho một bảng), nên mỗi tab trình duyệt làm việc độc lập. Chưa đăng
nhập: 401; không thuộc workspace hay workspace không có: cùng một 404. Câu hỏi không trả lời được vẫn trả 200, lý do
nằm ở `status` (`answered`, `needs_choice`, `rejected`, `failed`), để giao diện hiện cùng minh chứng. Route, trường và
mã lỗi: [API.md](API.md).

OpenAPI 3.1 sinh từ code bằng `utoipa`; router đăng ký handler qua `OpenApiRouter`, nên route có trong router thì có trong
đặc tả. Đặc tả ở `/api/openapi.json`, Swagger UI (kèm sẵn trong binary) ở `/api/docs`.

| File hợp đồng | Sinh từ | Kiểm lệch |
|---|---|---|
| `openapi.json` | `xdi-backend openapi` | Test `openapi_json_is_up_to_date` trong `make check-fast` |
| `frontend/src/api/schema.ts` | `openapi.json`, bằng `openapi-typescript` | `npm run api:check` trong `make check-frontend` |
| `frontend/src/api/examples/ask.json` | `xdi-backend ask-examples` (`service/ask/examples.rs`) | Test `ask_examples_json_is_up_to_date` |

Kiểu request và response của hỏi dữ liệu và truy dòng nguồn ở `service/ask/contract.rs`, `service/drill/contract.rs`
(service dựng response mà không gọi ngược lên `api/`). Đổi API: sửa handler hay struct, chạy `make openapi`, commit cả ba
file.

**Truy dòng nguồn** (`service/drill/`). Request gửi `query_id` và khóa của chỗ được bấm, không gửi SQL. Backend dựng câu
từ `sql_guarded` đã lưu: giữ FROM, JOIN, WHERE, thêm điều kiện khóa của ô, bỏ GROUP BY; cho qua guard với vai dữ liệu
của người đang bấm, chạy trên phiên bản câu hỏi đã đọc. Câu nối bảng trả dòng của bảng mà độ đo cộng, kèm cột khóa và cột
lọc của bảng nối; mỗi độ đo tính lại trên mọi dòng nguồn để so với ô. Câu có phép tập hợp, HAVING, hàm cửa sổ, bảng tạm
gộp nhóm trả 400 kèm lý do.

**Báo cáo** (`service/report/`). Mục báo cáo trỏ tới một câu hỏi đã trả lời; mở báo cáo hay xuất Excel thì chạy lại SQL
đã lưu qua guard với quyền của người xem, trên phiên bản câu hỏi đã đọc. Báo cáo tự động dựng SQL bằng code (độ đo chính
theo thời gian và theo hai cột chiều), lời qua kiểm số và kiểm nghĩa. PDF là bản in của trang báo cáo trên trình duyệt.

## 11. Hướng phát triển

Bản hiện tại nhắm dự án cỡ nhỏ và vừa, một instance.

| Giới hạn | Nguyên nhân | Đường nâng cấp |
|---|---|---|
| Một instance backend cho phần phân tích | DuckDB nhúng trong tiến trình | Tách engine phân tích thành service đọc Parquet trên object storage |
| Phiên đăng nhập tự quản | Không có identity provider | OIDC (Zitadel, Keycloak); `auth/` là module riêng |
| Dataset trên đĩa local | Volume Docker | S3 hay MinIO, đổi phần ghi file trong `engine/analytics.rs` |
| Một crate | Chưa có gì để tách | Rút engine hay guard thành crate trong workspace khi cần dùng lại |
| Không cache, không queue | Chưa có tải | Cache câu hỏi, queue cho nạp file lớn, ở tầng service |
| Nguồn là file hay bản chụp Postgres | Chụp giữ được bằng chứng tái lập | MySQL, SQLite; truy vấn thẳng database ngoài lúc hỏi |
| Khớp chữ, chưa khớp theo nghĩa | Cột `embedding` có sẵn trên ba bảng nhưng chưa dùng | Tìm theo vector trong cùng Postgres |
| Câu SQL chạy trong tiến trình backend | Phần đổi kết quả nằm ngoài trần bộ nhớ DuckDB ([GUARD.md](GUARD.md#hạn-chế-đã-biết)) | Chạy câu trong tiến trình con có trần RAM của hệ điều hành |

Ba điều giữ cho các nâng cấp trên không phải viết lại: state đi qua `AppState`; hợp đồng API là tài liệu và kiểu dữ
liệu; mọi lời gọi ra ngoài đi qua một chỗ để thêm retry, timeout, cache.
