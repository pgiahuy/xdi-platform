# Schema Postgres của ứng dụng

Trang này nói từng bảng trong Postgres của xdi-platform dùng để làm gì, cột nào quan trọng, ai ghi và lúc nào, kèm ví
dụ vài dòng. Schema hiện có 20 bảng, dựng bởi các migration `0001_init.sql` tới `0018_source_metadata_tables.sql` trong
[`backend/migrations/`](https://github.com/pgiahuy/xdi-platform/tree/main/backend/migrations); backend tự chạy migration
lúc khởi động.

Postgres chỉ giữ dữ liệu của ứng dụng: tài khoản, workspace, mô tả của dataset và bảng, hồ sơ cột, chỉ số, lịch sử câu
hỏi, nhật ký lỗi, bài học. Dữ liệu để phân tích (các dòng của file nạp lên hay của bảng chụp từ database) nằm ở file
Parquet trong `DATA_DIR`, DuckDB đọc trực tiếp ([ARCHITECTURE.md](ARCHITECTURE.md), mục 4). Database của tổ chức mà
admin kết nối chỉ được đọc (lúc kết nối, chọn bảng và chụp), xdi-platform không ghi gì vào đó.

Sơ đồ ER đầy đủ, sinh tự động từ một database vừa migrate: [images/schema.svg](images/schema.svg). Sơ đồ có mọi cột và
mọi khóa ngoại; ràng buộc `CHECK`, chỉ mục và hành vi khi xóa (`CASCADE`, `SET NULL`) đọc trong file migration.

Các ví dụ dưới đây là minh họa, lấy hình dạng từ database thử đã chạy thiết lập lần đầu với database mẫu
(`data/postgres-demo`: `sales_demo` và `edu_demo`). UUID được rút gọn (`0085…`), JSON dài được cắt (`…`), không có chuỗi
băm mật khẩu, mã phiên hay thông tin kết nối thật.

## Nhìn chung

- Mọi bảng nghiệp vụ có `workspace_id`. Khóa ngoại giữa các bảng nghiệp vụ gồm cả `workspace_id`, nên một dòng không
  trỏ được sang dòng của workspace khác. Ba bảng không có `workspace_id`: `workspaces`, `users`, `sessions`.
- Khóa chính là `uuid` sinh bằng `gen_random_uuid()`, trừ `memberships`, `settings`, `sessions` và
  `source_metadata_tables` dùng khóa tự nhiên.
- Xóa một workspace thì mọi dòng của nó mất theo (`ON DELETE CASCADE`). Xóa một bảng dữ liệu (dòng `datasets`) thì mất
  phiên bản, hồ sơ cột, quan hệ, chỉ số, luật lọc dòng, lịch sử câu hỏi, cuộc trò chuyện và bài học của bảng đó; nhật
  ký lỗi còn lại, chỉ đặt null `dataset_id`.
- Bảng `_sqlx_migrations` là sổ sách của sqlx (migration nào đã chạy, checksum), không thuộc dữ liệu ứng dụng và không
  có trong sơ đồ.

## Tài khoản và quyền

### users

Một tài khoản của cả bản cài, không thuộc riêng workspace nào; một người vào nhiều workspace qua `memberships`.

| Cột | Ý nghĩa |
|---|---|
| `email` | Email đăng nhập, duy nhất không phân biệt hoa thường |
| `password_hash` | Mật khẩu băm Argon2id dạng PHC; không lưu mật khẩu gốc |
| `is_active` | `false` khi quản trị hệ thống khóa tài khoản |
| `is_system_admin` | Quản trị hệ thống: quản lý tài khoản của cả bản cài, không tự có quyền đọc dữ liệu workspace |

Ai ghi: màn thiết lập lần đầu tạo tài khoản quản trị đầu tiên; sau đó quản trị hệ thống tạo, khóa, mở, đặt lại mật
khẩu; người dùng tự đổi mật khẩu; lệnh `xdi-backend reset-password` trên máy chủ.

| id | email | display_name | password_hash | is_active | is_system_admin |
|---|---|---|---|---|---|
| `4b1e…` | `admin@example.com` | Admin | (chuỗi Argon2, không in) | true | true |

### sessions

Một phiên đăng nhập. Mã phiên chỉ nằm trong cookie; bảng giữ SHA-256 của mã, nên đọc được bảng cũng không đăng nhập
được. Phiên lưu trong Postgres thay cho JWT để đăng xuất, khóa tài khoản, đổi vai có hiệu lực ngay ở request kế tiếp.

| Cột | Ý nghĩa |
|---|---|
| `token_hash` | Khóa chính, SHA-256 của mã phiên, 64 ký tự hex |
| `user_id` | Tài khoản của phiên |
| `expires_at` | Hạn, 30 ngày sau lúc đăng nhập; phiên hết hạn bị dọn |

Ai ghi: backend, khi đăng nhập (thêm), đăng xuất, khóa tài khoản, đặt lại mật khẩu (xóa).

| token_hash | user_id | created_at | expires_at |
|---|---|---|---|
| (64 ký tự hex, không in) | `4b1e…` | 2026-10-31 09:12 | 2026-11-30 09:12 |

### workspaces

Một phòng ban (`team`), một workspace cá nhân (`personal`), hay workspace tạm của phiên demo (`demo_session`, bắt buộc có
`expires_at`; chế độ demo chưa làm). Mỗi workspace có dataset, chỉ số, thành viên riêng.

| Cột | Ý nghĩa |
|---|---|
| `slug` | Tên trong URL (`/w/<slug>/...`), chữ thường, số, gạch nối, duy nhất |
| `kind` | `team`, `personal`, `demo_session` |

Ai ghi: thiết lập lần đầu tạo workspace đầu tiên; người dùng tạo workspace mới; admin của workspace đổi tên, xóa.

| id | name | slug | kind | expires_at |
|---|---|---|---|---|
| `6a71…` | Demo | `demo` | team | null |

### memberships

Ai thuộc workspace nào, với vai gì. Hai cột vai tách riêng: `role` quyết định được làm gì, `data_role` quyết định thấy
dòng nào (qua `rls_policies`).

| Cột | Ý nghĩa |
|---|---|
| `role` | `admin` (nạp, xóa dữ liệu, quản lý thành viên, chỉ số, bài học) hay `member` (xem, hỏi) |
| `data_role` | Tên vai dữ liệu, chữ tự do; null là không bị lọc dòng |

Khóa chính là cặp (`workspace_id`, `user_id`). Ai ghi: người tạo workspace thành `admin`; admin thêm thành viên theo
email, đổi vai, xóa; thành viên tự rời. Workspace luôn còn ít nhất một admin. Gán `data_role` mới có hàm trong store,
chưa có API hay màn nào gọi (#45).

| workspace_id | user_id | role | data_role |
|---|---|---|---|
| `6a71…` | `4b1e…` | admin | null |
| `6a71…` | `9c20…` | member | `mien_nam` |

### rls_policies

Luật lọc dòng theo vai dữ liệu trên một bảng: thành viên có `data_role` này chỉ thấy dòng mà `column_name` nằm trong
`allowed_values`. Guard thêm điều kiện đó vào mọi câu SQL chạy cho người đó (hỏi, truy dòng nguồn, tổng quan), kèm
điều kiện lan qua khóa ngoại sang bảng liên quan ([GUARD.md](GUARD.md), bước 3). Admin không bị lọc.

| Cột | Ý nghĩa |
|---|---|
| `dataset_id` | Bảng áp luật (dòng `datasets`) |
| `data_role` | Vai dữ liệu, khớp `memberships.data_role` |
| `column_name`, `allowed_values` | Cột của bảng và các giá trị được thấy; mảng rỗng là không thấy dòng nào |

Ai ghi: hiện chỉ test ghi bảng này. Store đã có hàm thay bộ luật của một bảng, chưa có API hay màn (#45), nên ngoài test
bảng luôn rỗng. Ví dụ dưới đây là dòng giả định:

| dataset_id | data_role | column_name | allowed_values |
|---|---|---|---|
| `831c…` (orders) | `mien_nam` | `region` | `{South}` |

### settings

Cấu hình theo workspace dạng khóa và giá trị JSON, khóa chính (`workspace_id`, `key`). Bảng có từ migration đầu, hiện
chưa có code nào đọc hay ghi; mọi cấu hình đang nằm ở biến môi trường ([INSTALL.md](INSTALL.md)).

## Nguồn, dataset, bảng

### sources

Một dòng là một **dataset** theo nghĩa của giao diện: một database đã kết nối (`kind = postgres`) hay một file đã nạp
(`kind = local_file`, đúng một bảng). Tên bảng giữ là `sources` từ trước khi đổi nghĩa (#266), xem mục Điểm hay hiểu
nhầm.

| Cột | Ý nghĩa |
|---|---|
| `name` | Tên dataset: tên database, hay tên bảng của file |
| `kind` | `postgres` hay `local_file`; `mysql`, `sqlite` có trong ràng buộc, chưa dùng |
| `domain` | Gói khái niệm ngành áp dụng (khóa của `domain_packs`, ví dụ `retail`); null là không có |
| `config` | JSON để hiển thị: máy chủ, cổng, database, tài khoản, `sslmode`, và `tables` (id bảng tới `schema.bảng` nguồn) |
| `secret_encrypted` | Toàn bộ thông tin kết nối kể cả mật khẩu, mã hóa AES-256-GCM bằng `SOURCE_SECRET_KEY`; không bao giờ trả qua API |

Ai ghi: admin kết nối database hay nạp file ở trang Dữ liệu; thiết lập lần đầu thêm mỗi database mẫu làm một nguồn
(`SAMPLE_DATABASE_URL`); xóa nguồn bị chặn khi còn bảng (`ON DELETE RESTRICT` từ `datasets`).

| id | name | kind | domain | config (cắt) | secret_encrypted |
|---|---|---|---|---|---|
| `0085…` | `sales_demo` | postgres | null | `{"host": "127.0.0.1", "port": 5440, "database": "sales_demo", "user": "analyst", "sslmode": "prefer", "tables": {"6734…": "sales.order_items", …}}` | (bản mã) |
| `affc…` | `edu_demo` | postgres | null | `{"database": "edu_demo", "tables": {"56f3…": "edu.students", …}, …}` | (bản mã) |

### datasets

Một dòng là một **bảng** bên trong dataset (`source_id` trỏ về `sources`). Mọi thứ gắn theo bảng (phiên bản, hồ sơ
cột, quan hệ, lịch sử câu hỏi) trỏ về đây qua `dataset_id`.

| Cột | Ý nghĩa |
|---|---|
| `source_id` | Dataset chứa bảng |
| `logical_table` | Tên bảng mô hình viết trong SQL; engine đổi sang file Parquet. Duy nhất trong một dataset, không phân biệt hoa thường |
| `current_version_id` | Phiên bản đang dùng để trả lời; không xóa được phiên bản đang là hiện hành |

Ai ghi: backend, khi chụp một bảng lần đầu hay nạp một file.

| id | source_id | name | logical_table | current_version_id |
|---|---|---|---|---|
| `6734…` | `0085…` (sales_demo) | order_items | `order_items` | `1c80…` |
| `831c…` | `0085…` (sales_demo) | orders | `orders` | `f263…` |
| `56f3…` | `affc…` (edu_demo) | students | `students` | `6994…` |

### dataset_versions

Mỗi lần chụp lại một bảng hay nạp lại một file là một phiên bản mới, không ghi đè. Câu hỏi cũ ghi phiên bản đã đọc nên
tái lập được.

| Cột | Ý nghĩa |
|---|---|
| `version` | Số thứ tự trong bảng, từ 1 |
| `source_kind` | `postgres`, `csv`, `parquet`, `xlsx` |
| `file_hash` | Hash nội dung; cùng hash thì không tạo phiên bản mới |
| `parquet_path` | Thư mục Parquet, tương đối với `DATA_DIR`: workspace, dataset, bảng, phiên bản |
| `profile_status` | `pending`, `structure` (đã có hồ sơ cấu trúc), `semantic` (mô hình đã viết mô tả), `failed` |
| `suggested_questions` | Tối đa 5 câu hỏi gợi ý do mô hình viết |
| `description` | Mô tả bảng (`COMMENT ON TABLE` ở database nguồn) lúc chụp |

Ai ghi: backend, lúc chụp hay nạp; lớp ngữ nghĩa chạy nền điền `suggested_questions` và đổi `profile_status`.

| id | dataset_id | version | source_kind | row_count | profile_status | suggested_questions (cắt) |
|---|---|---|---|---|---|---|
| `1c80…` | `6734…` (order_items) | 1 | postgres | 180519 | semantic | `["Doanh thu theo phân khúc khách hàng", "Top 5 danh mục có doanh thu cao nhất", …]` |

### source_metadata_tables

Bảng của một nguồn database chỉ có cấu trúc, chưa có dữ liệu: quá `SOURCE_MAX_ROWS` dòng (`row_limit`) hay cộng vào thì
quá `SOURCE_SNAPSHOT_MAX_MB` (`size_limit`). Không có dòng `datasets` cho các bảng này. Câu hỏi cần bảng đó thì luồng hỏi
nêu đúng tên bảng và nói nó chưa được chụp ([DATA.md](DATA.md), mục Chụp mọi bảng đọc được).

| Cột | Ý nghĩa |
|---|---|
| `table_name` | `schema.bảng` ở database nguồn; khóa chính cùng `source_id` |
| `cause`, `reason` | Mã lý do và lời giải thích cho admin |
| `columns` | JSON: tên, kiểu, nullable, khóa chính, mô tả của từng cột |

Ai ghi: backend, lúc chụp. Admin chụp riêng bảng đó sau (khi đã tăng trần) thì dòng ở đây bị xóa và bảng thành một dòng
`datasets` như mọi bảng.

| source_id | table_name | cause | row_count | reason (cắt) | columns (cắt) |
|---|---|---|---|---|---|
| `affc…` | `edu.student_daily_clicks` | row_limit | 1808119 | Bảng edu.student_daily_clicks có 1808119 dòng, quá giới hạn 1000000 dòng … | `[{"name": "code_module", "data_type": "text", "primary_key": true, …}, …]` |

### dataset_relations

Quan hệ khóa ngoại giữa hai bảng chụp từ cùng một database. Phía `from_*` là bảng giữ khóa ngoại (phía nhiều), phía
`to_*` là bảng được trỏ tới (khóa chính hay `UNIQUE`). Prompt sinh SQL dùng danh sách này để nối bảng, guard chỉ nhận
phép nối đúng cột của một dòng ở đây.

| Cột | Ý nghĩa |
|---|---|
| `from_dataset_id`, `from_columns` | Bảng và các cột khóa ngoại |
| `to_dataset_id`, `to_columns` | Bảng và các cột được trỏ tới, cùng số cột |
| `constraint_name` | Tên ràng buộc ở database nguồn |
| `cardinality` | `many_to_one`, hay `one_to_one` khi cột khóa ngoại cũng là khóa của bảng giữ nó |

Ai ghi: backend, lúc chụp; chụp lại cả dataset thì thay toàn bộ quan hệ của nó.

| from_dataset_id | from_columns | to_dataset_id | to_columns | constraint_name | cardinality |
|---|---|---|---|---|---|
| `6734…` (order_items) | `{order_id}` | `831c…` (orders) | `{order_id}` | `order_items_order_id_fkey` | many_to_one |
| `831c…` (orders) | `{customer_id}` | `a828…` (customers) | `{customer_id}` | `orders_customer_id_fkey` | many_to_one |

## Hồ sơ cột

### columns

Hồ sơ của một cột trong một phiên bản. Phần cấu trúc (kiểu, số ô trống, min, max) do code tính lúc nạp; mô tả, bí danh,
vai trò, đơn vị, cách gộp do lớp ngữ nghĩa (mô hình) đề xuất sau. Đây là phần chính của ngữ cảnh gửi mô hình khi hỏi.

| Cột | Ý nghĩa |
|---|---|
| `version_id`, `position`, `name`, `data_type` | Cột thứ mấy của phiên bản nào, tên và kiểu DuckDB |
| `role` | `dimension`, `measure`, `time`, `key`, `text`, `ignore` |
| `description`, `description_source` | Mô tả; nguồn `database` (từ `COMMENT ON`, mô hình không viết lại) hay `model` |
| `aliases` | Bí danh tiếng Việt để khớp câu hỏi |
| `unit` | `money`, `quantity`, `percent`, `duration`, `other`; luật biểu đồ dùng để biết hai số đo cùng thang hay không |
| `aggregation` | Cách gộp mặc định của số đo, `sum` hay `avg`; null đọc như `sum`; admin đổi ở màn Chỉ số |
| `is_sensitive` | Cột nhạy cảm: không đưa giá trị mẫu vào prompt |
| `time_format` | Định dạng `strptime` của cột ngày lưu dạng chữ |

Ai ghi: backend lúc nạp hay chụp (cấu trúc, mô tả từ database); lớp ngữ nghĩa chạy nền (mô tả, bí danh, vai trò, đơn
vị, cách gộp); admin đổi cách gộp. Màn sửa mô tả và bí danh chưa có. Cột `embedding` để dành, chưa dùng.

| name | data_type | role | description (cắt) | description_source | aliases | unit | null_count | distinct_count |
|---|---|---|---|---|---|---|---|---|
| `item_total` | DOUBLE | measure | Thành tiền sau giảm giá (Order Item Total; …) | database | `{"thành tiền","tổng tiền"}` | money | 0 | 2927 |
| `segment` | VARCHAR | dimension | Phân khúc khách: Consumer (cá nhân), Corporate … | database | `{"phân khúc khách hàng"}` | null | 0 | 3 |

### column_values

Các giá trị của một cột phân loại kèm số dòng và bí danh, để ánh xạ chữ trong câu hỏi sang đúng giá trị trong dữ liệu
("khách lẻ" thành `Consumer`). Có chỉ mục trigram trên giá trị đã bỏ dấu để tìm gần đúng.

| Cột | Ý nghĩa |
|---|---|
| `column_id`, `value` | Cột và một giá trị của nó, duy nhất theo cặp |
| `frequency` | Số dòng có giá trị đó |
| `aliases` | Cách người dùng hay gọi giá trị đó |

Ai ghi: backend lúc lập hồ sơ cấu trúc, cho cột có tối đa 200 giá trị khác nhau; lớp ngữ nghĩa thêm bí danh.

| column_id | value | frequency | aliases |
|---|---|---|---|
| `e2d0…` (customers.segment) | Consumer | 10695 | `{"khách lẻ","khách cá nhân"}` |
| `e2d0…` (customers.segment) | Corporate | 6239 | `{"khách doanh nghiệp"}` |

### metrics

Chỉ số do tổ chức định nghĩa, ví dụ "Doanh thu" là tổng thành tiền sau giảm giá. Prompt sinh SQL và form câu "tại sao"
dùng đúng công thức này thay vì để mô hình tự đoán. Sửa một chỉ số là thêm phiên bản mới (`version` tăng), câu hỏi cũ
vẫn tái lập được.

| Cột | Ý nghĩa |
|---|---|
| `source_id` | Dataset của chỉ số; tên chỉ số không trùng trong một dataset |
| `dataset_id` | Bảng mà công thức đọc cột |
| `expression`, `filter`, `time_column` | Biểu thức tổng hợp, điều kiện lọc và cột thời gian (nếu có) |
| `status` | `confirmed` hay `unconfirmed` |
| `origin` | `ui` (admin viết ở màn Chỉ số), `sample` (chỉ số mẫu), `formula_table`, `llm_suggested` |

Ai ghi: admin ở màn Chỉ số (công thức được chạy thử qua guard trước khi lưu); thiết lập lần đầu ghi chỉ số mẫu.

| name | source_id | dataset_id | expression | status | origin | version |
|---|---|---|---|---|---|---|
| Doanh thu | `0085…` (sales_demo) | `6734…` (order_items) | `SUM(item_total)` | confirmed | sample | 1 |
| Điểm trung bình | `affc…` (edu_demo) | `6e13…` (student_assessments) | `AVG(score)` | confirmed | sample | 1 |

### domain_packs

Gói khái niệm ngành của một workspace, chép từ `data/domains/*.json` ([DOMAINS.md](DOMAINS.md)). `key` trùng tên file
gói và là giá trị của `sources.domain`.

Ai ghi: backend chép gói và khái niệm còn thiếu vào workspace lần đầu cần đọc (màn Khái niệm ngành, hay câu hỏi trên
dataset có `domain`), không ghi đè bản admin đã sửa.

| id | key | name | description (cắt) |
|---|---|---|---|
| `d71a…` | `retail` | Bán lẻ và chuỗi cung ứng | Khái niệm chung của doanh nghiệp bán lẻ có giao hàng … |

### domain_concepts

Một khái niệm trong gói: tên, từ đồng nghĩa, các cách tính, chiều hay dùng. Khái niệm đang bật vào prompt sinh SQL và form
câu "tại sao" của dataset dùng gói đó.

| Cột | Ý nghĩa |
|---|---|
| `key` | Id của khái niệm trong file gói; null với khái niệm admin thêm |
| `synonyms`, `calculations`, `dimensions`, `sanity_checks` | Cùng dạng với file gói |
| `enabled` | Tắt thì không vào prompt, vẫn còn để bật lại |
| `position` | Thứ tự như trong file; khái niệm admin thêm đứng sau |

Ai ghi: backend khi chép gói; admin thêm, sửa, tắt ở màn Khái niệm ngành (`updated_at`, `updated_by`).

| pack_id | key | name | synonyms (cắt) | calculations (cắt) | enabled |
|---|---|---|---|---|---|
| `d71a…` (retail) | `revenue` | Doanh thu | `{"doanh thu","doanh số","tiền bán hàng",…}` | `[{"name": "Tổng tiền dòng hàng", …}, …]` | true |

## Hỏi đáp và học từ phản hồi

### conversations

Một cuộc trò chuyện trên một bảng: câu sau hiểu theo các câu trước. Chỉ người tạo đọc và hỏi tiếp được. Các lượt là dòng
`queries` có `conversation_id`.

Ai ghi: backend, khi người dùng gửi câu đầu của một cuộc mới; tiêu đề là câu hỏi đầu.

| id | dataset_id | created_by | title |
|---|---|---|---|
| `33e6…` | `6734…` (order_items) | `4b1e…` | Doanh thu theo phân khúc khách hàng |

### queries

Lịch sử câu hỏi, mỗi lượt hỏi một dòng, kể cả câu bị guard từ chối hay chạy lỗi. Bảng kết quả không lưu; xem lại số liệu
bằng cách hỏi lại. Câu được admin duyệt làm câu mẫu cũng nằm ở đây.

| Cột | Ý nghĩa |
|---|---|
| `dataset_id`, `version_id`, `read_versions` | Bảng đang hỏi, phiên bản đã đọc, và mọi bảng, phiên bản đã đọc khi nối bảng |
| `sql_generated`, `sql_guarded` | SQL mô hình viết và SQL sau guard (thêm `LIMIT`, lọc dòng) |
| `status`, `error` | `ok`, `rejected` (guard từ chối), `error` (chạy lỗi) |
| `timings` | JSON các bước (`context`, `classify`, `llm`, `guard`, `run`, `explain`, `narrate`) kèm thời gian |
| `understood` | Phần "Hệ thống đã hiểu là" dạng chữ ngắn, gửi lại mô hình ở lượt sau |
| `answer` | Lời trả lời đã hiện kèm kết luận của hai cổng kiểm, để mở lại cuộc trò chuyện thấy đúng lời đó |
| `explanation` | Với câu "tại sao": form mô hình điền và bảng phân rã đã tính |
| `feedback`, `feedback_reason`, `feedback_note` | Người hỏi bấm Tốt (`correct`) hay Chưa đạt (`incorrect`), lý do, ghi chú |
| `example_approved_by`, `example_approved_at` | Admin duyệt câu Tốt làm câu mẫu; chỉ câu đã duyệt vào prompt |

Ai ghi: backend, sau mỗi câu hỏi; người hỏi đánh giá; admin duyệt câu mẫu ở màn Câu mẫu.

```json
{
  "id": "42eb…",
  "conversation_id": "33e6…",
  "dataset_id": "6734…",
  "question": "Doanh thu theo phân khúc khách hàng",
  "sql_guarded": "SELECT c.segment, SUM(oi.item_total) AS revenue FROM order_items oi JOIN orders o ON oi.order_id = o.order_id JOIN customers c ON o.customer_id = c.customer_id GROUP BY c.segment ORDER BY revenue DESC LIMIT 100000",
  "status": "ok",
  "row_count": 3,
  "timings": {"steps": [{"name": "context", "ok": true, "duration_ms": 8}, {"name": "classify", …}, …]},
  "read_versions": [{"dataset_id": "6734…", "version_id": "1c80…"}, {"dataset_id": "831c…", …}, …],
  "answer": {"text": "Consumer có tổng thành tiền sau giảm giá cao nhất: khoảng 17,2 triệu (17.164.278,37), …", "verdict": null, "by_model": false, "warnings": []},
  "feedback": "correct",
  "example_approved_at": "2026-10-31 10:05"
}
```

### error_log

Nhật ký chỗ mô hình hay hệ thống sai, để admin xem và viết bài học. Ghi tự động, không ai sửa. Có câu hỏi và câu mô hình
trả nên chỉ admin của workspace đọc được ([PRIVACY.md](PRIVACY.md)).

| Cột | Ý nghĩa |
|---|---|
| `step` | `guard` (guard từ chối), `run` (SQL lỗi khi chạy), `fanout` (chặn cộng lặp khi nối bảng), `why_form` (form câu "tại sao" sai), `narrate` (lời bị bộ kiểm chặn), `feedback` (người hỏi bấm Chưa đạt) |
| `error` | Lời báo lỗi |
| `model_output` | Câu mô hình trả ở bước đó (SQL, form JSON, lời viết); null khi không có |
| `query_id` | Dòng lịch sử, nếu có |

Ai ghi: backend, tự động ở các bước trên.

| step | question | error | model_output |
|---|---|---|---|
| guard | Xóa các đơn hàng bị hủy | Chỉ cho phép câu SELECT để đọc dữ liệu | `DELETE FROM orders WHERE order_status = 'CANCELED'` |

### lessons

Bài học: một câu ngắn admin viết, tay hay từ một dòng nhật ký lỗi. Mới tạo là bản nháp; chỉ bài đã duyệt và đang bật vào
prompt của các bước dùng mô hình. `dataset_id` null là bài của cả workspace; có giá trị thì câu hỏi trên mọi bảng cùng
dataset với bảng đó nhận bài.

| Cột | Ý nghĩa |
|---|---|
| `text` | Nội dung bài học |
| `enabled` | Bật thì vào prompt; bật bắt buộc có `approved_at` |
| `error_id` | Dòng nhật ký bài được viết từ, nếu có |
| `approved_by`, `approved_at` | Ai bật và lúc nào |

Ai ghi: chỉ admin, ở màn Bài học (viết tay, hay từ một dòng của khung Nhật ký lỗi).

| dataset_id | text | enabled | error_id | approved_at |
|---|---|---|---|---|
| `6734…` (order_items) | Doanh thu là tổng item_total (sau giảm giá), không phải sales | true | null | 2026-10-31 10:20 |

## Điểm hay hiểu nhầm

- **`sources` là dataset, `datasets` là bảng.** Từ #266, một dataset là một database đã kết nối hay một file; bảng nằm
  bên trong. Tên bảng Postgres giữ như cũ để không phải sửa mọi câu SQL: dòng `sources` là dataset, dòng `datasets` là
  một bảng của nó, nối qua `datasets.source_id`. Trong code, kiểu `Dataset` và mọi `dataset_id` cũ là bảng; kiểu của
  API `/catalog` dùng nghĩa mới ([ARCHITECTURE.md](ARCHITECTURE.md), mục 5).
- **Phiên bản theo từng bảng.** Mỗi bảng có dãy phiên bản riêng; không có phiên bản chung của dataset. Chụp lại cả
  database là chụp lại từng bảng. Hồ sơ cột gắn với phiên bản; câu hỏi ghi phiên bản đã đọc.
- **Quan hệ chỉ từ khóa ngoại đã khai báo.** `dataset_relations` chép khóa ngoại của database nguồn lúc chụp. Không đoán
  quan hệ theo tên cột; file nạp lên không có quan hệ. Guard từ chối phép nối không khớp một quan hệ.
- **Câu mẫu không có bảng riêng.** Câu mẫu là dòng `queries` có `feedback = 'correct'` và `example_approved_at` khác
  null. Người hỏi tự bấm Tốt chưa đủ, phải có admin duyệt.
- **Nhật ký lỗi tự động, bài học do admin.** `error_log` do backend ghi, không ai sửa. `lessons` chỉ admin viết, và chỉ
  bài `enabled = true` (đã duyệt) vào prompt.
- **Chỉ số theo dataset, công thức theo bảng.** Tên chỉ số không trùng trong một dataset (`metrics.source_id`), còn
  `metrics.dataset_id` là bảng mà công thức đọc cột.
- **Lọc dòng chưa bật được ngoài test.** `memberships.data_role` và `rls_policies` có sẵn, guard đã áp, nhưng chưa có
  API để đặt (#45).
- **Dữ liệu phân tích không nằm trong Postgres.** Các dòng dữ liệu là file Parquet dưới `DATA_DIR`
  (`dataset_versions.parquet_path`). Postgres chỉ có hồ sơ: số dòng, min, max, danh sách giá trị của cột phân loại.

## Sinh lại sơ đồ

Sơ đồ [images/schema.svg](images/schema.svg) sinh bằng [tbls](https://github.com/k1LoW/tbls) (giấy phép MIT, không nằm
trong repo) và Graphviz, qua `scripts/schema-docs.sh`. Cần một database đã chạy đủ migration của nhánh, dùng database
riêng, không dùng `xdi` của `make dev`:

```bash
docker compose exec -T postgres createdb -U xdi xdi_schema
# Backend migrate lúc khởi động; lệnh quản trị list-admins migrate xong thì thoát. Chạy trong backend/ như make dev.
(cd backend && DATABASE_URL=postgres://xdi:xdi@localhost:5433/xdi_schema cargo run -- list-admins)
TBLS=/đường/dẫn/tới/tbls DATABASE_URL=postgres://xdi:xdi@localhost:5433/xdi_schema make schema-docs
docker compose exec -T postgres dropdb -U xdi xdi_schema
```

Thêm migration thì sửa trang này (bảng mới, cột quan trọng mới, ai ghi) và chạy lại `make schema-docs` trong cùng PR.
