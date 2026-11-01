# Dữ liệu

Trang này nói hệ thống nhận file nào, chụp database Postgres thế nào, giữ phiên bản ra sao, đặt tên bảng, tên cột và
lập hồ sơ cột thế nào. Cách làm trên giao diện: [Dùng lần đầu](USAGE.md); nạp bằng `curl`: [Cài đặt](INSTALL.md).
Dữ liệu nằm ở đâu và gửi gì cho mô hình ngôn ngữ: [Dữ liệu và quyền riêng tư](PRIVACY.md).

Đã có: nạp CSV, Parquet, Excel; chụp mọi bảng đọc được của một database Postgres thành Parquet; phiên bản theo bảng;
hồ sơ cột lớp cấu trúc và lớp ngữ nghĩa. Phần chưa có ở mục cuối.

## Dataset và bảng

Một dataset là một database đã kết nối hay một file đã nạp; bảng nằm trong dataset. Một file là một dataset có một
bảng; mỗi bảng chụp từ database là một bảng của dataset đó. Phiên bản, hồ sơ cột và chỉ số theo từng bảng. Trong
Postgres, dataset là bảng `sources`, bảng là bảng `datasets` ([SCHEMA.md](SCHEMA.md)); vì vậy ở các route và trường cũ
(`dataset_id`, `name` của lần nạp), "dataset" nghĩa là bảng.

## Nạp file

`POST /api/workspaces/{ws}/datasets`, dạng `multipart/form-data`, chỉ `admin` gọi được. Các trường: `file` (lặp lại để
gửi nhiều file), `name` (tên dataset mới), `dataset_id` (thêm phiên bản cho bảng có sẵn), `sheet` (sheet của file
Excel). Code: `backend/src/api/datasets/`, `backend/src/service/ingest.rs`, `backend/src/engine/analytics.rs`.

### Định dạng nhận

Định dạng đoán theo đuôi tên file, không phân biệt hoa thường; đuôi khác bị từ chối (400). Mọi file của một lần nạp
cùng định dạng; file Excel nạp từng file một.

| Định dạng | Đuôi | Cách đọc |
|---|---|---|
| CSV | `.csv` | DuckDB đọc, dòng đầu là tiêu đề, tự dò dấu ngăn cột và kiểu từng cột |
| Parquet | `.parquet` | DuckDB đọc, giữ cột và kiểu trong file |
| Excel | `.xlsx` | Crate `calamine` đọc trong một tiến trình con; mỗi sheet được chọn ghi thành CSV rồi đi tiếp như CSV |

### Mã hóa của CSV

Code: `backend/src/engine/csv_encoding.rs`.

- Đọc theo UTF-8 trước. File không phải UTF-8 thì đổi sang UTF-8 theo bảng mã windows-1252 rồi đọc lại; file latin-1
  cũng đọc đúng vì hai bảng mã chỉ khác ở byte 0x80 tới 0x9F.
- Byte lạ nằm sâu trong file (sau phần DuckDB xem trước) cũng được đổi, khi ghi Parquet.
- File có chữ UTF-8 lẫn byte không phải UTF-8 thì báo lỗi kèm tên file và số dòng đầu tiên có byte hỏng, vì đổi lúc đó
  làm sai các chữ UTF-8 đang đúng. Lưu lại file dạng UTF-8 rồi nạp lại.
- UTF-16 không được hỗ trợ.

### File Excel

Code: `backend/src/service/excel.rs`, `backend/src/engine/excel.rs`, `backend/src/engine/excel_worker.rs`.

- `POST /api/workspaces/{ws}/datasets/sheets` nhận một file `.xlsx`, trả từng sheet: tên, có bị ẩn không, tên cột, số
  dòng ước lượng (theo vùng dữ liệu file tự ghi). File không được lưu lại.
- Chọn sheet bằng trường `sheet`, lặp lại để chọn nhiều sheet. Không chọn thì file phải có đúng một sheet đang hiện.
  Các sheet chọn cùng lúc phải cùng tên cột và gộp thành một bảng.
- Dòng đầu có dữ liệu là dòng tiêu đề; cột tính từ ô tiêu đề trái nhất đến phải nhất, ô có dữ liệu ngoài các cột đó thì
  báo lỗi. Dòng trống bị bỏ; ô lỗi công thức (`#DIV/0!`) thành ô trống.
- Ô ngày giờ ghi dạng `2009-12-01 07:45:00` (chỉ ngày `2009-12-01`, chỉ giờ `07:45:00`), để DuckDB nhận `TIMESTAMP`,
  `DATE`, `TIME`. Cột có cả ô chỉ ngày lẫn ô có giờ là `TIMESTAMP`; cột trộn ô chỉ giờ với ô ngày thành cột chữ.
- Ô gộp và tiêu đề lệch dòng chưa hỗ trợ. File `.xls` bị từ chối vì sai đuôi; file có mật khẩu (hay `.xls` đổi đuôi)
  bị từ chối kèm hướng dẫn bỏ mật khẩu hay lưu lại dạng `.xlsx`.

calamine tin các con số file tự khai, nên một file nhỏ cố ý làm sai có thể khiến nó cấp phát rất nhiều bộ nhớ hay đọc
rất lâu. Vì vậy file Excel được đọc trong tiến trình con `xdi-backend excel-worker`, mỗi lúc một tiến trình, với các
trần dưới đây. Vượt trần thì chỉ request đó lỗi: ba trần kích thước giải nén, bảng chuỗi dùng chung và dung lượng CSV
trả 413, các trần khác trả 400. Một file chạm trần giữ lượt đọc Excel tới khi bị dừng, người khác nạp Excel lúc đó phải
chờ.

| Trần | Giá trị |
|---|---|
| RAM ảo của tiến trình con | 2 GB (`setrlimit`, chỉ trên hệ Unix) |
| Thời gian CPU của tiến trình con | 5 phút (`setrlimit`, chỉ trên hệ Unix) |
| Thời gian chờ phía backend | 6 phút |
| Tổng kích thước giải nén của file | 10 lần `MAX_UPLOAD_MB` |
| Bảng chuỗi dùng chung (nằm hết trong RAM) | 64 MB, hoặc `MAX_UPLOAD_MB` nếu nhỏ hơn |
| Tổng dung lượng CSV ghi ra cho một lần nạp | 10 lần `MAX_UPLOAD_MB` |
| Số sheet | 100 |
| Độ dài tên sheet | 100 ký tự |
| Số cột | 16 384 (giới hạn của Excel) |
| Độ dài một dòng | 16 MB |
| Tổng tên sheet và tên cột trong danh sách sheet | 1 MB |

### Giới hạn của một lần nạp

| Giới hạn | Giá trị | Vượt thì |
|---|---|---|
| Tổng dung lượng mọi trường (file và chữ) | `MAX_UPLOAD_MB`, mặc định 200 | 413 |
| Số trường multipart | 1000 | 400 |
| Độ dài tên file | 255 ký tự; phần thư mục trong tên bị bỏ | 400 |
| Độ dài tên dataset (`name`) | 100 ký tự | 400 |

`MAX_UPLOAD_MB` không phải số dương thì backend cảnh báo trong log và dùng 200. File được ghi xuống đĩa từng đoạn
trong lúc nhận, không giữ cả file trong bộ nhớ.

Bị từ chối (400), lời báo ghi tên file trừ hai trường hợp cuối (chỉ lộ ra khi đọc cả bảng đã gộp):

- Hai file cùng nội dung, hay cùng tên, trong một lần nạp. File rỗng (0 byte).
- CSV có dòng thiếu hay thừa cột làm DuckDB không tách được cột.
- Các file không cùng tên cột (phân biệt hoa thường; thứ tự được khác); lời báo ghi cột thiếu và cột thừa.
- File đã có cột tên `_source_file` (không phân biệt hoa thường).
- Cùng tên cột mà kiểu không gộp được (số với ngày). Kiểu gộp được thì DuckDB nâng kiểu: số nguyên và số thập phân
  thành `DOUBLE`.
- Bảng đã gộp không có dòng nào. File chỉ có tiêu đề nạp cùng file có dữ liệu thì không bị báo.

## Nguồn database (Postgres)

`admin` thêm một database Postgres làm nguồn (trang **Dữ liệu**, khung **Kết nối database**, hay
`POST /api/workspaces/{ws}/sources`), rồi chụp các bảng, view của nó. Mỗi database là một dataset, tên theo tên
database. Bản này chỉ có Postgres và chỉ chụp: câu hỏi chạy trên bản chụp Parquet, không truy vấn thẳng database nguồn.
Code: `backend/src/service/sources/`; route ở [API.md](API.md#nguồn-database).

- Kết nối luôn chỉ đọc (`default_transaction_read_only`, `statement_timeout`; với cách chép `direct` thêm
  `ATTACH ... READ_ONLY`). Máy chủ phải nằm trong `SOURCE_ALLOWED_HOSTS`.
- Mật khẩu và chứng chỉ CA mã hóa bằng `SOURCE_SECRET_KEY` (hay file khóa tự sinh trong `DATA_DIR`); response chỉ ghi
  `custom_ca`.
- Xóa nguồn chỉ được khi không còn bảng nào chụp từ nó; xóa bảng không xóa nguồn.

### TLS

Bằng rustls, nghĩa `sslmode` như libpq:

| `sslmode` | TLS | Kiểm chứng chỉ máy chủ |
|---|---|---|
| `disable` | Không | Không |
| `prefer` (mặc định) | Có nếu máy chủ nhận, không thì kết nối thường. Máy chủ nhận TLS mà bắt tay lỗi (chỉ có TLS 1.0, 1.1, chứng chỉ quá yếu) thì báo lỗi, không quay về kết nối thường như libpq | Không |
| `require` | Bắt buộc | Không: chống nghe lén, không chống máy giả mạo |
| `verify-full` | Bắt buộc | Có: do CA tin cậy cấp và đúng tên máy đã nhập (`host`) |

CA tin cậy là kho CA của hệ thống (image Docker cài `ca-certificates`), cộng chứng chỉ CA dạng PEM admin dán vào ô
**Chứng chỉ CA** (`ssl_root_cert`, chỉ nhận với `verify-full`). Với cách chép `direct`, phần chép đi qua libpq của
extension với cùng `sslmode`; khi đã dán CA thì libpq chỉ tin CA đó, không cộng CA của hệ thống. Chưa có `verify-ca` và
chứng chỉ phía client.

### Chụp mọi bảng đọc được

Phạm vi dữ liệu do quyền của tài khoản nguồn quyết định: bảng, view nào tài khoản có quyền `SELECT` thì xdi thấy và mặc
định chụp; muốn giới hạn dữ liệu thì cấp quyền hẹp. Code: `catalog.rs` (danh sách), `plan.rs` (trần), `copy.rs` và
`direct.rs` (chép), `snapshot.rs` (nạp, quan hệ), `schedule.rs` (lịch).

- **Danh sách bảng** (`GET .../sources/{id}/tables`): mọi bảng, view đọc được, kèm số dòng ước lượng và dung lượng theo
  thống kê của Postgres (`pg_class.reltuples`, `pg_table_size`; null khi bảng chưa `ANALYZE` hay với view), comment,
  số cột, khóa ngoại ra (`references`) và vào (`referenced_by`), `is_referenced` (bảng danh mục: bỏ tick thì bảng khác
  mất quan hệ), `over_limit` khi riêng bảng đã quá trần, và `state` (`snapshotted` kèm `dataset_id`, `metadata_only`
  kèm `reason`, hay null). Chỉ đọc catalog, không đọc dòng nào.
- **Chụp** (`POST .../sources/{id}/snapshot`): thân rỗng chụp mọi bảng đọc được; `tables` là đúng các bảng được tick;
  một tên là chụp riêng bảng đó. Không giới hạn số bảng. Bảng đã chụp từ nguồn này thì thêm phiên bản vào đúng bảng
  đó, nên bảng chiều dùng chung chỉ có một bảng. Bảng không đổi nội dung thì không tạo phiên bản (`created: false`).
- **Một ảnh chụp nhất quán:** mọi bảng của một lần chụp đọc trong một giao dịch `REPEATABLE READ READ ONLY`, nên các
  bảng cùng là ảnh của một thời điểm: không có dòng hàng trỏ tới đơn chưa có trong bản chụp `orders`. Đổi lại, suốt
  bước chép database nguồn không dọn được dòng cũ (`VACUUM`) mà giao dịch còn thấy. Đo ngày 01/11/2026 trên một máy
  ([PERFORMANCE.md](PERFORMANCE.md#chụp-bảng-lớn)): một bảng 1 860 MB giữ giao dịch khoảng 19 giây, năm bảng 2 625 MB
  cùng lần khoảng 15 giây. Database nguồn bận ghi thì chụp bảng lớn riêng, hay hạ `SOURCE_SNAPSHOT_MAX_MB`.
- **Lỗi giữa chừng:** các bảng nạp lần lượt sau khi chép xong; một bảng nạp lỗi thì các bảng trước đó đã có phiên bản
  mới, còn quan hệ của lần này chưa ghi. Chụp lại là đủ.

**Trần.** Tính bằng dung lượng Postgres báo (`pg_table_size`), xét trước khi chép, nên bảng quá trần không có file nào
được ghi. `MAX_UPLOAD_MB` không áp cho chụp bảng. Số đo để đặt các mặc định: [PERFORMANCE.md](PERFORMANCE.md#chụp-bảng-lớn).

| Biến | Mặc định | Nghĩa |
|---|---|---|
| `SOURCE_TABLE_MAX_MB` | 2048 | Một bảng lớn hơn thì chỉ lưu cấu trúc |
| `SOURCE_SNAPSHOT_MAX_MB` | 4096 | Tổng một lần chụp; bảng xếp từ nhỏ tới lớn và lấy dần tới trần, bảng còn lại chỉ lưu cấu trúc |
| `SOURCE_MAX_ROWS` | trống, không có trần dòng | Đặt thì mỗi bảng được đếm dòng; bảng quá trần chỉ lưu cấu trúc |
| `SOURCE_TIMEOUT_SECS` | 300 | Cả bước chép; quá thì bỏ cả lần chụp: câu đang đọc bị ngắt, file tạm bị xóa, không tạo phiên bản |

View không biết cỡ trước (tính 0): file chép ra của một view quá `SOURCE_TABLE_MAX_MB` thì cả lần chụp bị từ chối
(413).

**Bảng chỉ có cấu trúc.** Bảng quá trần vẫn lưu cột, kiểu Postgres, khóa chính, comment bảng và cột (bảng
`source_metadata_tables`), có trong `metadata_only` của response và của dataset kèm lý do (`cause` là `size_limit`
hay `row_limit`). Câu hỏi không trả lời được mà cần bảng đó thì lời báo nêu đúng tên bảng, nói bảng chưa được chụp và
cách chụp (`backend/src/service/ask/unsnapshotted.rs`). Chụp riêng bảng đó sau (`tables` một tên); chụp được thì dòng
cấu trúc bị xóa. Bảng đã có dữ liệu từ trước mà lần này quá trần thì giữ phiên bản cũ.

**Chụp lại.** Nút **Chụp lại** của dataset (`POST .../catalog/{id}/snapshot`) chụp lại mọi bảng dataset đang giữ, gồm
bảng chỉ có cấu trúc, không gồm bảng admin bỏ tick lúc kết nối; một giao dịch, cùng các trần. Bảng tài khoản không còn
đọc được (đã xóa, mất quyền) ghi trong `unreadable` và giữ phiên bản cũ. `SOURCE_RESNAPSHOT_HOURS` (mặc định tắt) chạy
đúng việc này cho mọi nguồn Postgres theo lịch, lần lượt từng nguồn; vòng đầu chạy sau một khoảng lịch, không chạy lúc
khởi động. Thêm bảng sau này: chụp riêng bảng đó ở màn kết nối. Bỏ một bảng: xóa bảng đó.

### Cách chép: direct và copy

Chọn bằng `SOURCE_SNAPSHOT_MODE`. Chụp là chép một lần rồi đi đúng đường nạp file (Parquet, hồ sơ cột, phiên bản), nên
hồ sơ, minh chứng, dòng nguồn và câu hỏi cũ trỏ đúng phiên bản đã chụp.

- `direct` (mặc định): DuckDB đọc thẳng bảng bằng extension `postgres` qua giao thức `COPY` nhị phân và ghi Parquet một
  bước, không có file CSV tạm. Extension là file đóng gói sẵn đúng bản DuckDB của backend, nạp từ đĩa
  (`SOURCE_POSTGRES_EXTENSION`), không tải lúc chạy. Mật khẩu và CA ghi ra một file tạm quyền 0600 cho libpq đọc, xóa
  ngay sau lần chụp. Không nạp được extension (thiếu file, lệch bản) hay libpq không kết nối được thì log ghi cảnh báo
  và lần chụp dùng `copy`.
- `copy` (dự phòng, hay đặt tay): backend đọc bảng bằng `COPY (SELECT * FROM schema.bảng) TO STDOUT (FORMAT csv)` ra
  file CSV tạm, DuckDB đọc lại và ghi Parquet. File CSV có thể lớn hơn bảng nên cũng theo trần `SOURCE_TABLE_MAX_MB`.

Hai cách cho cùng kiểu cột và cùng dữ liệu (test `both_modes_give_the_same_types_and_rows`). Nội dung file khác nhau,
nên lần chụp lại đầu tiên sau khi đổi cách chép tạo phiên bản mới dù bảng không đổi. Số đo thời gian, đĩa tạm, bộ
nhớ: [PERFORMANCE.md](PERFORMANCE.md).

### Kiểu cột và mô tả

- Kiểu cột lấy từ Postgres, không đoán như CSV: số nguyên thành `INTEGER` hay `BIGINT`, `real` và `double precision`
  thành `DOUBLE`, `numeric(p,s)` thành `DECIMAL(p,s)` (`numeric` không khai báo hay rộng hơn 38 chữ số thành `DOUBLE`,
  giữ chính xác khoảng 15 chữ số), `boolean`, `date`, `timestamp`, `timestamptz` giữ kiểu tương ứng. Kiểu khác (`text`,
  `varchar`, `uuid`, `json`, mảng, enum) thành chữ, nên mã như `00123` giữ số 0 ở đầu. Chuỗi rỗng và `NULL` vẫn khác
  nhau.
- Cùng lần chụp, trong cùng giao dịch, backend đọc `COMMENT ON TABLE` và `COMMENT ON COLUMN` từ catalog
  (`backend/src/service/sources/comments.rs`). Comment rỗng coi như không có; xuống dòng và khoảng trắng liền nhau gộp
  thành một dấu cách, ký tự vô hình bị bỏ, quá 300 ký tự thì cắt ở khoảng trắng gần nhất và thêm `...`.
- Comment của cột thành mô tả cột, nguồn `database` (`description_source`); lớp ngữ nghĩa không ghi đè mô tả này. Cột
  không có comment nhận mô tả do mô hình viết (nguồn `model`). Comment của bảng ghi trên phiên bản.
- Bảng có comment thì comment tính vào hash của bản chụp: chỉ sửa comment rồi chụp lại cũng ra phiên bản mới.

### Quan hệ

Mỗi bảng nguồn thành một bảng của dataset; bảng không nối thành bảng phẳng, nên không có cột lặp để cộng nhầm. Mọi
khóa ngoại khai báo ở database nguồn giữa hai bảng đã chụp, khi ít nhất một đầu là bảng vừa chụp và cột được trỏ tới
là khóa duy nhất (khóa chính, `UNIQUE`), ghi vào `dataset_relations`, kể cả khóa nhiều cột. Không đoán quan hệ từ tên
cột.

- Mỗi quan hệ có bảng phía nhiều (giữ khóa ngoại) và cột của nó, bảng phía một và cột của nó, tên ràng buộc, và
  `cardinality` (`one_to_one` khi cột khóa ngoại cũng là khóa chính hay `UNIQUE`, còn lại `many_to_one`).
- Lần chụp sau thay toàn bộ quan hệ giữa các bảng vừa chụp; quan hệ giữa hai bảng không có trong lần này giữ nguyên.
  Xóa một bảng thì quan hệ của nó mất theo. `GET .../catalog` trả quan hệ cho mọi thành viên.
- Câu hỏi đọc được mọi bảng đã chụp của dataset, guard chỉ nhận `JOIN` theo các quan hệ này, và code chặn câu nối làm
  cộng lặp: [Kiến trúc](ARCHITECTURE.md), [Guard SQL](GUARD.md).

Database mẫu để thử: `sales_demo` (DataCo) và `edu_demo` (OULAD) trong một container
([data/postgres-demo](../data/postgres-demo/README.md)), kèm sơ đồ ER và mô tả từng bảng, cột sinh bằng tbls.

## Phiên bản, hash và file

Mỗi lần nạp hay chụp có thay đổi tạo một phiên bản mới của bảng, số phiên bản tăng dần, không ghi đè phiên bản cũ.
Phiên bản vừa tạo thành phiên bản hiện hành (`current_version_id`).

- Backend tính SHA-256 của từng file trong lúc nhận; hash của lần nạp là SHA-256 của danh sách hash từng file đã sắp
  xếp, nên thứ tự chọn file không đổi kết quả. Với Excel, hash tính thêm tên các sheet đã chọn và phiên bản cách đọc
  Excel. Hash không tính tên file.
- Nạp trùng: có `dataset_id` thì so với các phiên bản của bảng đó; không có thì so với mọi phiên bản trong workspace.
  Bảng mới chụp từ database không so cả workspace: cùng một database kết nối hai lần thì mỗi dataset có bảng riêng.
  Trùng thì trả 200 với `created: false` và phiên bản đã có; tạo mới thì 201.
- File Parquet nằm ở `<DATA_DIR>/<id workspace>/<id dataset>/<id bảng>/<thư mục phiên bản>/`; Postgres lưu đường dẫn
  tương đối với `DATA_DIR`. Trong lúc nạp, file nằm ở `<DATA_DIR>/staging/`, xóa khi xong hay khi lỗi; backend dừng
  giữa chừng thì lần khởi động sau xóa.
- Mỗi file (hay mỗi sheet Excel) là một phần `part-NNNN.parquet` của phiên bản, kèm cột `_source_file` ở cuối ghi tên
  file gốc (hay tên sheet) của từng dòng; bảng chụp từ database có `_source_file` là
  `postgres://máy/database/schema.bảng`, không có tài khoản hay mật khẩu. DuckDB đọc cả thư mục như một bảng
  (`read_parquet('<thư mục>/*.parquet', union_by_name = true)`), nên nhiều file cùng cột (mỗi tháng một file) thành một
  bảng, và truy dòng nguồn chỉ được dòng đến từ file nào.
- Phiên bản lưu số dòng và `profile_status`: `structure` khi có hồ sơ cột, `semantic` khi xong lớp ngữ nghĩa, `pending`
  khi chưa, `failed` khi lớp cấu trúc lỗi do dữ liệu.
- Xóa bảng (`DELETE .../datasets/{id}`, chỉ `admin`) xóa mọi phiên bản cùng hồ sơ cột, chỉ số, lịch sử câu hỏi trong
  Postgres, rồi xóa thư mục Parquet trên đĩa ([API](API.md#dataset-và-bảng)). File xóa sau khi giao dịch Postgres xong:
  lỗi lúc xóa file chỉ để lại file thừa.

## Tên bảng và tên cột

**Tên dataset**: với database là tên database; với file là trường `name`, bỏ trống thì lấy tên file đầu tiên bỏ đuôi,
cắt còn 100 ký tự.

**Tên bảng logic** (`logical_table`) là tên mô hình dùng trong SQL và guard kiểm. Đặt một lần khi tạo bảng, từ tên
bảng nguồn hay tên dataset file: bỏ dấu, viết thường, mỗi dãy ký tự không phải chữ hay số thành một `_`, bỏ `_` ở hai
đầu, cắt còn 48 ký tự; tên bắt đầu bằng số thì thêm `t_`; không còn ký tự nào thì là `dataset`; trùng tên bảng khác
trong cùng dataset thì thêm `_2`, `_3`. Hai dataset khác nhau được có bảng cùng tên, vì một câu hỏi chỉ đọc bảng của
một dataset. Ví dụ "Bán hàng 2024" thành `ban_hang_2024`, "2024 sales" thành `t_2024_sales`. Bảng tạo trước khi có
dataset nhiều bảng có thể giữ tên như `orders_2`, vì SQL đã lưu dùng tên đó.

**Tên cột**:

- CSV và Parquet: giữ như DuckDB đọc, không đổi tên, không bỏ dấu. Tên có khoảng trắng hay dấu viết trong nháy kép
  trong SQL (`"Order Item Total"`). Guard so tên cột không phân biệt hoa thường.
- Excel: bỏ khoảng trắng hai đầu; ô tiêu đề trống lấy chữ cột Excel (`C`, `AA`); tên trùng (không phân biệt hoa
  thường) thêm `_2`, `_3`. Tên ở danh sách sheet trùng với tên cột sau khi nạp.

Kết quả nạp (`UploadResult.columns`) trả tên và kiểu DuckDB của từng cột, gồm `_source_file`. Hồ sơ đầy đủ đọc qua
[tổng quan bảng](API.md#tổng-quan-bảng) và API chỉ số.

## Hồ sơ cột

### Lớp cấu trúc

Do code tính trên Parquet, không gọi mô hình (chạy như nhau với `LLM_PROVIDER=mock`). Phiên bản và hồ sơ ghi trong một
giao dịch, xong trước khi route nạp trả 201; ghi lỗi thì không còn phiên bản nào, nạp lại được như lần đầu. Phiên bản
còn `pending` được lập hồ sơ nền mỗi lần backend khởi động: lỗi do dữ liệu đặt `failed`, lỗi có thể tự hết giữ
`pending`.

Mỗi cột, kể cả `_source_file`, là một dòng `columns`: kiểu DuckDB, số ô trống, số giá trị khác nhau đếm chính xác, min
và max dạng chữ, vai trò, và `time_format` với cột ngày dạng chữ. Cột có từ 1 tới 200 giá trị khác nhau có thêm mỗi giá
trị một dòng `column_values` kèm số dòng. Cột có giá trị dài quá 1000 byte hay có ký tự NUL thì không ghi
`column_values`; tên cột như vậy làm lần nạp bị từ chối (400).

**Cột ngày dạng chữ.** DuckDB tự nhận một số dạng ngày lúc đọc CSV, nhưng cột ngày vẫn có thể là `VARCHAR` (DataCo ghi
`1/31/2018 22:56`). Với cột `VARCHAR`, hồ sơ thử một danh sách ngắn định dạng `strptime` (`%Y-%m-%d`, `%d/%m/%Y`,
`%m/%d/%Y`, có hay không kèm giờ); một định dạng chỉ được tính khi đọc được mọi ô không trống với năm từ 1900 tới 2100.
Định dạng chọn được ghi vào `time_format`, min và max là thời điểm đã đọc. Đọc được cả ngày/tháng lẫn tháng/ngày thì
giữ định dạng của cột cùng tên ở phiên bản hiện hành nếu còn đọc được, không thì theo ngày/tháng (backend ghi log). SQL
dùng `strptime("cột", '<time_format>')`.

**Vai trò cột**: thử từng luật theo thứ tự, luật đầu khớp thắng. Luật nhìn kiểu và số đo; tên cột chỉ được đọc ở luật 1
và luật 2b.

| Thứ tự | Điều kiện | Vai trò |
|---|---|---|
| 1 | Cột toàn ô trống, hoặc là `_source_file` | `ignore` |
| 2 | Kiểu `DATE`, `TIMESTAMP`, `TIMESTAMP WITH TIME ZONE`, hoặc cột chữ có `time_format` | `time` |
| 2b | Số nguyên có tên là mốc thời gian đếm bằng số (xem dưới bảng) | `dimension` khi có tối đa 200 giá trị khác nhau, còn lại `text` |
| 3 | Bảng hơn 2 dòng; số nguyên, không ô trống, mỗi dòng một giá trị khác nhau | `key` |
| 4 | Bảng hơn 2 dòng; số có tối đa 2 giá trị khác nhau, như cột cờ 0/1 | `dimension` |
| 5 | Số còn lại | `measure` |
| 6 | `BOOLEAN`, hoặc chữ có tối đa 200 giá trị khác nhau và có `column_values` | `dimension` |
| 7 | Còn lại: chữ nhiều giá trị, chữ không ghi được `column_values`, `TIME`, `UUID`, `INTERVAL`, `JSON`, `BLOB`, kiểu lồng nhau | `text` |

Trên DataCo: `Market`, `Order Country` (164 nước) là `dimension`, `Order City` (3597 thành phố) là `text`,
`Order Item Id` là `key`, hai cột `... (DateOrders)` là `time`. Số nguyên lặp lại như `Order Id` ra `measure` theo
luật; lớp ngữ nghĩa sửa được một số ca như vậy.

**Mốc thời gian dạng số** (luật 2b, `backend/src/service/column_roles.rs`): cột số nguyên mà tên (bỏ dấu, tách từ như cột
nhạy cảm) có một từ `date`, `day`, `week`, `month`, `year`, `ngày`, `tuần`, `tháng`, và không có từ chỉ một lượng (`số`,
`count`, `num`, `number`, `length`, `duration`, `total`, `tổng`). Ví dụ `edu_demo`: `student_daily_clicks.date` (số
ngày tính từ đầu học phần, 295 giá trị) là `text`, `vle_materials.week_from` (30 giá trị) là `dimension`,
`registrations.date_registration` là `text`. Cộng các cột này ra số vô nghĩa, và chúng không phải kỳ: câu hỏi nguyên
nhân chỉ lấy kỳ từ cột `time`. Vẫn nhóm theo, lọc theo số và lấy trung bình được (đường câu hỏi nguyên nhân nhận trung
bình trên mọi cột số không phải `key`). Dạng số nhiều là một lượng, lấy trung bình có nghĩa, nên giữ luật số đo:
`days_for_shipping_real` vẫn là `measure`.

**Khóa của database nguồn**: bảng chụp từ database đọc thêm khóa chính và cột khóa ngoại (`pg_constraint`) trong cùng
giao dịch chụp, rồi đặt `key` cho cột số nguyên thuộc khóa chính hay khóa ngoại, và cho cột đứng một mình làm khóa
chính dù kiểu gì. Cột chữ của khóa nhiều cột hay khóa ngoại (`code_module`, 7 môn) giữ vai trò theo luật, vì đó là nhóm
người dùng hỏi theo; cột mốc thời gian trong khóa chính (`student_daily_clicks.date`) giữ luật 2b. Nhờ vậy mã như
`order_items.product_id` hay `student_daily_clicks.id_student` là `key` ngay lúc chụp, không phải chờ lớp ngữ nghĩa. Chỉ
áp cho phiên bản chụp mới; phiên bản đã có giữ vai trò cũ.

**Cột nhạy cảm** (`is_sensitive`) được đánh dấu trong lượt lập hồ sơ cấu trúc, trước khi phiên bản dùng được: tên cột
có một cụm như email, mật khẩu, số điện thoại, số giấy tờ, địa chỉ, họ tên (`SENSITIVE_NAME_PHRASES` ở
`backend/src/service/profile.rs`), hay phần lớn giá trị của cột có danh sách giá trị trông như email hoặc số điện
thoại. Tên được bỏ dấu, chữ thường, tách từ (cả camelCase và chỗ chữ liền số) rồi so theo từ, nên "Hotel" không khớp
"tel". Luật nghiêng về đánh dấu nhầm hơn bỏ sót. Phiên bản cũ được quét lại mỗi lần khởi động (chỉ bật cờ, không tắt).
Giá trị của cột nhạy cảm không vào prompt nào.

### Lớp ngữ nghĩa

Nạp xong (route đã trả 201), backend lập hồ sơ ngữ nghĩa chạy nền (`backend/src/service/semantic/`): mô hình viết mô
tả và bí danh cột, đơn vị và cách gộp của cột số đo, bí danh giá trị, câu hỏi gợi ý, đề xuất thêm cột nhạy cảm (chỉ
chiều bật), và sửa vai trò cột trong phạm vi hẹp. Mỗi phiên bản ba prompt song song; tối đa `SEMANTIC_JOBS` phiên bản
cùng lúc (mặc định 2). Người nạp không phải chờ và không nhận lỗi; xong thì `profile_status` là `semantic`.

- Lỗi hết hạn mức của Gemini (429, mọi khóa và mô hình dự phòng đều hết): phần đó chờ rồi gọi lại trong cùng lượt, tối
  đa 3 lần. Thời gian chờ là số giây Gemini dặn ("Please retry in 24s"), không có thì 30 giây, tối đa 60 giây. Sau một
  lần hết hạn mức, mọi phần của mọi lượt đang chạy chờ cùng mốc đó rồi đi lần lượt, cách nhau ít nhất 5 giây, trong 2
  phút kể từ lần hết hạn mức gần nhất (`backend/src/service/semantic/quota.rs`). Hết 3 lần vẫn lỗi thì như lỗi tạm bên
  dưới.
- Câu trả lời không dùng được (JSON hỏng, không mục nào hợp lệ) hay lỗi `BadRequest` của provider (nội dung bị chặn,
  `mock` không có bảng) thì đánh dấu đã thử kèm tên provider (`semantic_attempted_with`): khởi động không gọi lại với
  cùng provider, đổi `LLM_PROVIDER` thì thử lại, nạp lại đúng bộ file cũ là đường thử lại tường minh. Lỗi khác (quá
  tải, quá giờ, thiếu cấu hình, Postgres lỗi) giữ `structure`, lần khởi động sau thử lại.
- Prompt chỉ có siêu dữ liệu: tên dataset, tên bảng, số dòng, mô tả từ database; mỗi cột tên, kiểu, vai trò, tỷ lệ ô
  trống, số giá trị khác nhau; min, max chỉ của cột `time` và cột `measure` số thực không nhạy cảm; giá trị chỉ của cột
  `dimension` không nhạy cảm, kèm số dòng, tối đa 50 giá trị một cột (nhiều hơn thì 20 giá trị nhiều dòng nhất), tổng
  tối đa 400, bỏ giá trị dài quá 200 byte. Prompt lập hồ sơ không có giá trị của cột `text`, `key` hay cột nhạy cảm.
- Câu trả lời được kiểm từng mục trước khi ghi, mục sai bỏ riêng (`build_profile_answer`): cột và giá trị phải có
  thật, mô tả tối đa 300 ký tự, tối đa 8 bí danh mỗi mục (mỗi bí danh tối đa 60 ký tự), tối đa 5 câu gợi ý (mỗi câu
  tối đa 200 ký tự). Đổi vai trò chỉ nhận cột số nguyên `measure` thành `key` hay `dimension` (khi không quá 200 giá
  trị), và cột `dimension` thành `text`.
- Câu hỏi gợi ý: prompt xin hai câu hỏi nguyên nhân (chênh lệch giữa hai kỳ của cột `time` hay hai giá trị có trong
  danh sách, không nói chiều vì mô hình không thấy số), một câu tỷ lệ, trung bình hay top N, còn lại là câu tổng hay
  phân rã đơn giản. Số dòng gọi theo thứ một dòng của bảng ghi. Câu gợi ý lưu theo phiên bản và trả ra trong danh sách
  dataset, chi tiết dataset, cùng `current_profile_status` và `semantic_unavailable` (provider đang cấu hình đã thử mà
  không lập được).
- Trước khi lưu, code kiểm từng câu gợi ý, không gọi mô hình (`backend/src/service/semantic/questions.rs`). Câu hỏi
  nguyên nhân phải nêu đúng hai giá trị của cùng một cột `dimension` (đọc như đường câu hỏi nguyên nhân đọc nhóm câu
  hỏi nêu tên), hoặc kỳ viết dạng "năm 2017", "tháng 11/2016", "quý 3/2017", "ngày 31/5/2017" nằm trong khoảng min, max
  của một cột `time`: hai kỳ, hay một kỳ mà kỳ liền trước cũng có dữ liệu (đường câu hỏi nguyên nhân so với kỳ liền
  trước). Câu đếm "số <danh từ>" mà danh từ không có trong tên, mô tả bảng, tên, mô tả, bí danh cột hay giá trị cột thì
  bỏ; luật này chỉ chạy khi bảng có mô tả cột để đối chiếu. Câu bị bỏ ghi log kèm lý do. Ví dụ bị bỏ:
  "Vì sao số đơn ngày 10 thay đổi so với ngày 0" trên bảng lượt bấm `student_daily_clicks`.

### Đơn vị và cách gộp

- **Đơn vị** (`columns.unit`) chỉ cho cột `measure`, một trong năm chữ: `money`, `quantity`, `percent`, `duration`,
  `other`; chưa có thì null. Luật chọn biểu đồ dùng đơn vị để vẽ các số đo chung một thang hay mỗi số đo một biểu đồ
  (`backend/src/service/ask/chart.rs`; [Kiến trúc](ARCHITECTURE.md)).
- **Cách gộp** (`columns.aggregation`) là `sum` hay `avg`, chưa ai chọn thì đọc là `sum`. Mô hình đề xuất `sum` khi
  tổng có nghĩa (tiền, số lượng), `avg` khi mỗi giá trị mô tả chính dòng của nó (tuổi, điểm, giá, tỷ lệ). Lượt nền chỉ
  điền cột chưa có cách gộp, nên cách gộp admin đặt trước đó được giữ. Admin đổi ở màn **Chỉ số** cho phiên bản hiện
  hành.
- Ba nơi đọc cách gộp: prompt viết SQL ghi "aggregate it with AVG, never SUM" cạnh cột `avg`; form câu hỏi nguyên nhân
  đổi `sum` trên cột `avg` thành trung bình mỗi dòng và phân rã thành phần cơ cấu và phần hiệu suất; luật câu gợi ý
  (không bao giờ là tổng của cột `avg`).

## Dataset mẫu

Thiết lập lần đầu tới được database mẫu (`SAMPLE_DATABASE_URL`) thì thêm mỗi database làm một dataset và chụp mọi bảng
tài khoản `analyst` đọc được, như admin kết nối mà không bỏ tick bảng nào, cộng chỉ số mẫu của bảng có file
`<bảng>.metrics.json` (`backend/src/service/samples_db.rs`). Một database lỗi thì giữ database còn lại.
Câu gợi ý của database mẫu chọn tay, ghi trong `<SAMPLES_DIR>/suggested_questions.json` theo tên database (8 câu mỗi
database, đã hỏi thử với Gemini): backend đọc file lúc khởi động, chỉ giữ tên là database trong `SAMPLE_DATABASE_URL`,
và trả trong `suggested_questions` của dataset ([API](API.md#dataset-và-bảng)). Trang hỏi hiện các câu này khi chưa ghim
bảng nào; ghim một bảng thì hiện câu gợi ý của bảng đó. Không có
database mẫu nào chụp được thì nạp `dataco.csv` trong `SAMPLES_DIR` thành dataset `DataCo`. Tắt cả hai bằng
`SEED_SAMPLE=false`. Chi tiết, nguồn và giấy phép: [data/README.md](../data/README.md).

## Chưa có

- Màn và API xem, sửa hồ sơ cột (vai trò, cột nhạy cảm, mô tả, bí danh); màn Chỉ số mới sửa được cách gộp.
- Chép cách gộp admin đã đặt sang phiên bản sau của cùng bảng.
- So hồ sơ giữa hai phiên bản (cột mất, thêm, đổi kiểu) trước khi chuyển phiên bản hiện hành; chọn lại phiên bản cũ,
  xóa riêng một phiên bản.
- Nạp nguyên tử: ghi phiên bản vào Postgres và chuyển file Parquet về chỗ cuối chưa nằm trong một giao dịch, lỗi giữa
  hai bước có thể để lại dòng thừa trong Postgres.
- Nguồn MySQL, SQLite; truy vấn thẳng database ngoài lúc hỏi; `sslmode=verify-ca` và chứng chỉ phía client.
- Câu hỏi nguyên nhân có độ đo ở bảng khác bảng bắt đầu (form chỉ nêu cột của bảng bắt đầu; phân rã xét thêm cột của
  bảng nối nhiều-một tới hai bước).
- Truy dòng nguồn của câu cộng trên bảng tạm hay câu con đã gộp nhóm, và các dạng liệt kê ở
  [API.md](API.md#truy-dòng-nguồn).
