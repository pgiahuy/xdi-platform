# Dữ liệu

Tài liệu này ghi hệ thống nhận file dữ liệu nào, đổi chúng thành gì, giữ phiên bản ra sao và đặt tên bảng, tên cột thế
nào. Cách nạp trên giao diện và bằng `curl`: [Cài đặt](INSTALL.md), mục Nạp dữ liệu. Dữ liệu nằm ở đâu và gửi gì cho
mô hình ngôn ngữ: [Dữ liệu và quyền riêng tư](PRIVACY.md).

Một dataset là một database đã kết nối hay một file đã nạp; bảng nằm bên trong dataset (#257). Một file là một dataset
có một bảng; mỗi bảng chụp từ một database là một bảng của dataset đó. Phiên bản theo từng bảng. Cách lưu trong
Postgres và vì sao giữ tên cũ: [Kiến trúc](ARCHITECTURE.md), mục 5. Dưới đây "dataset" ở các route và trường cũ
(`dataset_id`, `name` của lần nạp) là bảng.

Nạp file: `POST /api/workspaces/{ws}/datasets`, dạng `multipart/form-data`, chỉ `admin` của workspace gọi được
([API](API.md)). Các trường: `file` (lặp lại để gửi nhiều file), `name` (tên dataset mới), `dataset_id` (thêm phiên bản
cho dataset có sẵn), `sheet` (sheet cần nạp của file Excel). Code: `backend/src/api/datasets/`,
`backend/src/service/ingest.rs`, `backend/src/engine/analytics.rs`.

## Định dạng nhận

Định dạng đoán theo đuôi tên file, không phân biệt hoa thường. Đuôi khác bị từ chối (400).

| Định dạng | Đuôi | Cách đọc |
|---|---|---|
| CSV | `.csv` | DuckDB đọc, dòng đầu là tiêu đề; DuckDB tự dò dấu ngăn cột (phẩy, chấm phẩy...) và kiểu từng cột |
| Parquet | `.parquet` | DuckDB đọc, giữ cột và kiểu trong file |
| Excel | `.xlsx` | Crate `calamine` đọc trong một tiến trình con; mỗi sheet được chọn ghi thành CSV rồi đi tiếp như CSV |

Mọi file của một lần nạp phải cùng định dạng: toàn CSV hoặc toàn Parquet. File Excel nạp từng file một.

### Mã hóa của CSV

`backend/src/engine/csv_encoding.rs`.

- Đọc theo UTF-8 trước. DuckDB báo file không phải UTF-8 thì backend đổi file sang UTF-8 theo bảng mã windows-1252 rồi
  đọc lại. windows-1252 là bảng mã của file xuất từ phần mềm Windows cũ; ở mọi byte ngoài khoảng 0x80 đến 0x9F nó cho
  cùng ký tự với latin-1, nên file latin-1 cũng đọc đúng.
- Byte lạ nằm sâu trong file (sau phần DuckDB xem trước để dò cột) cũng được đổi, khi ghi Parquet.
- File có chữ UTF-8 lẫn byte không phải UTF-8 thì không đổi mà báo lỗi kèm tên file và số dòng đầu tiên có byte hỏng:
  đổi theo windows-1252 lúc đó sẽ làm sai các chữ UTF-8 đang đúng. Lưu lại file dạng UTF-8 rồi nạp lại.
- UTF-16 không được hỗ trợ.

### File Excel

`backend/src/service/excel.rs`, `backend/src/engine/excel.rs`, `backend/src/engine/excel_worker.rs`.

- `POST /api/workspaces/{ws}/datasets/sheets` nhận một file `.xlsx`, trả từng sheet: tên, sheet có bị ẩn không, tên
  cột, số dòng ước lượng (theo vùng dữ liệu file tự ghi, có thể tính cả dòng trống). File không được lưu lại.
- Chọn sheet bằng trường `sheet`, lặp lại để chọn nhiều sheet. Không chọn thì file phải có đúng một sheet đang hiện.
  Các sheet chọn cùng lúc phải cùng tên cột và gộp thành một bảng; sheet khác cột thì nạp riêng từng lần, mỗi lần một
  dataset.
- Dòng đầu có dữ liệu là dòng tiêu đề; các cột tính từ ô tiêu đề trái nhất đến ô tiêu đề phải nhất. Ô có dữ liệu nằm
  ngoài các cột đó thì báo lỗi. Dòng trống bị bỏ qua; ô lỗi công thức (`#DIV/0!`) thành ô trống.
- Ô ngày giờ ghi dạng `2009-12-01 07:45:00` (chỉ ngày: `2009-12-01`, chỉ giờ: `07:45:00`) để DuckDB nhận kiểu
  `TIMESTAMP`, `DATE`, `TIME`. Cột có cả ô chỉ ngày lẫn ô có giờ được đọc là `TIMESTAMP`; cột trộn ô chỉ giờ với ô ngày
  thành cột chữ.
- Ô gộp và tiêu đề lệch dòng chưa hỗ trợ.
- File `.xls` bị từ chối vì sai đuôi, như mọi đuôi không nhận. File có mật khẩu (hay file `.xls` đổi đuôi thành `.xlsx`)
  bị từ chối với lời báo hướng dẫn bỏ mật khẩu hoặc lưu lại dạng `.xlsx`.

calamine tin những con số file tự khai, nên một file nhỏ cố ý làm sai có thể khiến nó cấp phát rất nhiều bộ nhớ hay đọc
rất lâu. Vì vậy file Excel được đọc trong tiến trình con `xdi-backend excel-worker`, mỗi lúc một tiến trình, với các
trần sau. Vượt trần thì chỉ request đó lỗi, backend vẫn phục vụ người khác: ba trần về kích thước giải nén, bảng chuỗi
dùng chung và dung lượng CSV trả 413, các trần khác trả 400.

| Trần | Giá trị |
|---|---|
| RAM ảo của tiến trình con | 2 GB (đặt bằng `setrlimit`, chỉ trên hệ Unix) |
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

Một file chạm trần vẫn giữ lượt đọc Excel tới khi bị dừng, nên người khác nạp Excel lúc đó phải chờ.

## Giới hạn của một lần nạp

| Giới hạn | Giá trị | Vượt thì |
|---|---|---|
| Tổng dung lượng mọi trường (file và chữ) | `MAX_UPLOAD_MB`, mặc định 200 | 413 |
| Số trường multipart | 1000 | 400 |
| Độ dài tên file | 255 ký tự; phần thư mục trong tên bị bỏ | 400 |
| Độ dài tên dataset (`name`) | 100 ký tự | 400 |

`MAX_UPLOAD_MB` đặt sai (không phải số dương) thì backend cảnh báo trong log và dùng 200. File được ghi xuống đĩa từng
đoạn trong lúc nhận, không giữ cả file trong bộ nhớ.

Các trường hợp sau bị từ chối (400). Lời báo ghi tên file, trừ hai trường hợp cuối, vì chúng chỉ lộ ra khi đọc cả bảng
đã gộp:

- Hai file cùng nội dung, hoặc hai file cùng tên, trong một lần nạp.
- File rỗng (0 byte).
- CSV có dòng thiếu hoặc thừa cột làm DuckDB không tách được cột.
- Các file không cùng tên cột (phân biệt hoa thường); lời báo ghi cột thiếu và cột thừa. Thứ tự cột được phép khác
  nhau.
- File đã có cột tên `_source_file` (không phân biệt hoa thường).
- Cùng tên cột nhưng kiểu không gộp được, ví dụ file này là số, file kia là ngày. Kiểu gộp được thì DuckDB nâng kiểu:
  số nguyên ở file này và số thập phân ở file kia thành `DOUBLE`.
- Bảng đã gộp không có dòng dữ liệu nào, ví dụ file duy nhất chỉ có dòng tiêu đề. File chỉ có tiêu đề nạp cùng file có
  dữ liệu thì không bị báo.

## Nguồn database (Postgres)

`admin` của workspace thêm được một database Postgres làm nguồn (trang Dữ liệu, "Kết nối database", hay
`POST /api/workspaces/{ws}/sources`), rồi chụp một bảng hay view thành dataset. Bản này chỉ có Postgres; MySQL, SQLite
và truy vấn thẳng database ngoài lúc hỏi để sau ([#165](https://github.com/pgiahuy/xdi-platform/issues/165)).
Code: `backend/src/service/sources/`, `backend/src/api/sources.rs`.

- Chụp là chép một lần, rồi đi đúng đường nạp ở trên (Parquet, hồ sơ cột, phiên bản). Hồ sơ, bằng chứng, dòng nguồn và
  câu hỏi cũ trỏ đúng phiên bản đã chụp. Hai cách chép, chọn bằng `SOURCE_SNAPSHOT_MODE` (#246):
  - `direct` (mặc định): DuckDB đọc thẳng bảng bằng extension `postgres` của nó, qua giao thức `COPY` nhị phân của
    Postgres, và ghi Parquet một bước, không có file CSV tạm. Extension là file đóng gói sẵn đúng bản DuckDB của backend,
    nạp từ đĩa, không tải lúc chạy ([INSTALL.md](INSTALL.md), mục 2). Không nạp được (thiếu file, lệch bản) hay libpq
    của extension không kết nối được thì log ghi cảnh báo và lần chụp dùng `copy`.
  - `copy` (dự phòng, hoặc đặt tay): backend đọc bảng bằng `COPY (SELECT * FROM schema.bảng) TO STDOUT (FORMAT csv)` ra file CSV
    tạm, DuckDB đọc lại file đó và ghi Parquet.

  Hai cách cho cùng kiểu cột và cùng dữ liệu (test `both_modes_give_the_same_types_and_rows` so từng dòng trên một bảng
  đủ loại kiểu); số đo thời gian, đĩa tạm, bộ nhớ ở [PERFORMANCE.md](PERFORMANCE.md). Đổi cách chép làm nội dung file
  khác đi, nên lần chụp lại đầu tiên sau khi đổi tạo một phiên bản mới dù bảng không đổi. "Chụp lại" tạo phiên bản mới của cùng dataset, đọc lại đúng bảng của lần trước; bảng không đổi thì trả
  `created: false` như nạp trùng.
- Kiểu cột lấy từ Postgres, DuckDB không đoán như với file CSV nạp tay: số nguyên thành `INTEGER` hay `BIGINT`,
  `real` và `double precision` thành `DOUBLE`, `numeric(p,s)` thành `DECIMAL(p,s)` (`numeric` không khai báo, hay
  rộng hơn 38 chữ số, thành `DOUBLE`, chỉ giữ chính xác khoảng 15 chữ số), `boolean`, `date`, `timestamp`,
  `timestamptz` giữ kiểu tương ứng. Mọi kiểu khác (`text`, `varchar`, `uuid`, `json`, mảng, enum) thành chữ, nên mã
  như `00123` giữ nguyên số 0 ở đầu. Chuỗi rỗng và `NULL` vẫn khác nhau sau khi chụp.
- Mô tả có sẵn trong database (#245): cùng lần chụp, trong cùng giao dịch chỉ đọc, backend đọc comment của bảng
  (`COMMENT ON TABLE`, qua `obj_description`) và của từng cột (`COMMENT ON COLUMN`, qua `col_description`) từ catalog,
  không đọc thêm dòng dữ liệu nào (`backend/src/service/sources/comments.rs`). Comment rỗng hay chỉ có khoảng trắng coi
  như không có; xuống dòng và khoảng trắng liền nhau gộp thành một dấu cách, ký tự vô hình bị bỏ, quá 300 ký tự thì cắt
  ở khoảng trắng gần nhất và thêm `...`. Comment của cột thành mô tả cột trong hồ sơ, ghi nguồn `database`
  (`description_source`); comment của bảng ghi trên phiên bản (`description` của phiên bản và của tổng quan dataset).
  Cột không có comment vẫn nhận mô tả do mô hình viết (nguồn `model`) như cũ. Lớp ngữ nghĩa không viết lại và không ghi
  đè mô tả nguồn `database` (mục Hồ sơ lớp ngữ nghĩa). Comment tính vào hash của bản chụp khi bảng có ít nhất một
  comment: chỉ sửa comment rồi chụp lại cũng ra phiên bản mới mang comment mới; bảng không có comment giữ hash như trước.
- Giới hạn: `SOURCE_MAX_ROWS` dòng cho mỗi bảng (đếm mọi bảng trước khi chép, quá thì từ chối cả lần chụp, không chụp
  thiếu), `MAX_UPLOAD_MB` cho file chép ra của mỗi bảng (CSV với `copy`, Parquet nén với `direct`, nên cùng một bảng
  `direct` chịu được bảng lớn hơn), tối đa 10 bảng một lần chụp, `SOURCE_TIMEOUT_SECS` cho cả lần chụp.
- Kết nối luôn chỉ đọc (`default_transaction_read_only`, `statement_timeout`; với `direct` thêm `ATTACH ... READ_ONLY`),
  mọi bảng của một lần chụp đọc trong một giao dịch `REPEATABLE READ`. Máy chủ phải nằm trong `SOURCE_ALLOWED_HOSTS`.
- **TLS** (#230), bằng rustls, nghĩa `sslmode` như libpq:

  | `sslmode` | TLS | Kiểm chứng chỉ máy chủ |
  |---|---|---|
  | `disable` | Không | Không |
  | `prefer` (mặc định) | Có nếu máy chủ nhận, không thì kết nối thường. Máy chủ nhận TLS mà bắt tay lỗi (chỉ có TLS 1.0, 1.1, chứng chỉ quá yếu) thì báo lỗi, không quay về kết nối thường như libpq | Không |
  | `require` | Bắt buộc | Không: chống nghe lén trên đường truyền, không chống máy giả mạo |
  | `verify-full` | Bắt buộc | Có: do CA tin cậy cấp và đúng tên máy đã nhập (`host`) |

  Với `direct`, phần chép dữ liệu đi qua libpq của extension với cùng `sslmode`; khác ở `verify-full` có dán CA: libpq
  chỉ tin CA dán vào, không cộng thêm CA của hệ thống (không dán CA thì dùng CA của hệ thống). Mật khẩu và CA ghi ra
  file tạm quyền 0600 cho libpq đọc, xóa ngay sau lần chụp.

  CA tin cậy là kho CA của hệ thống (image Docker cài `ca-certificates`), cộng chứng chỉ CA dạng PEM admin dán vào ô
  "Chứng chỉ CA" (trường `ssl_root_cert`) khi máy chủ dùng CA riêng của tổ chức; ô này chỉ nhận với `verify-full`. Chứng
  chỉ CA được mã hóa cùng thông tin kết nối, response chỉ ghi `custom_ca`. Chưa có `verify-ca` và chứng chỉ phía
  client. Đã thử trên máy dev với một Postgres 17 bật `ssl=on` và chứng chỉ do CA tự tạo cấp cho `localhost`:
  `require` kết nối được; `verify-full` không dán CA bị từ chối (CA lạ), dán CA thì kết nối được, nối bằng `127.0.0.1`
  bị từ chối (sai tên máy); `require` tới máy chủ không bật TLS bị từ chối kèm lời báo chọn `prefer` hay `disable`.
- Không dùng extension `postgres` của DuckDB vì `INSTALL postgres` tải extension từ Internet lúc chạy, đúng bản DuckDB
  đã biên dịch; image Docker không có thư mục home cho user chạy backend, và máy cài có thể không ra được Internet.
- Xóa nguồn chỉ được khi không còn dataset nào chụp từ nó; xóa dataset không xóa nguồn.

Database mẫu để thử: hai database Postgres nhiều bảng trong cùng một container, `sales_demo` dựng từ DataCo và
`edu_demo` dựng từ OULAD, chạy trên máy bằng Docker
([data/postgres-demo](https://github.com/pgiahuy/xdi-platform/tree/main/data/postgres-demo)). Sơ đồ ER và mô tả từng
bảng, cột (sinh bằng tbls):
[sales_demo](https://github.com/pgiahuy/xdi-platform/blob/main/data/postgres-demo/docs/sales_demo/README.md),
[edu_demo](https://github.com/pgiahuy/xdi-platform/blob/main/data/postgres-demo/docs/edu_demo/README.md).

### Chụp nhiều bảng liên quan

Database của tổ chức thường tách dữ liệu ra nhiều bảng: `order_items` (mỗi dòng một dòng hàng) trỏ tới `orders`,
`products`; `orders` trỏ tới `customers`. Một lần chụp lấy một bảng cùng các bảng admin chọn kèm; mỗi bảng thành một
dataset riêng, khóa ngoại giữa chúng ghi thành quan hệ giữa các dataset (bảng `dataset_relations`). Không nối thành một
bảng phẳng, nên không có cột lặp lại trên nhiều dòng để cộng nhầm. Code: `backend/src/service/sources/relations.rs`
(tìm quan hệ), `snapshot.rs` (chụp), `backend/src/engine/store/relations.rs` (lưu).

- **Bảng liên quan** (`POST .../sources/{id}/relations`): đọc khóa ngoại trong `pg_catalog`, kể cả khóa nhiều cột,
  một bước quanh bảng đang chọn, theo cả hai chiều: bảng nó trỏ tới (`many_to_one`) và bảng trỏ vào nó (`one_to_many`).
  Một bảng chụp kèm được khi tài khoản có quyền đọc nó và cột được trỏ tới là khóa chính hay một ràng buộc `UNIQUE`
  (chỉ mục `UNIQUE` không kèm ràng buộc không tính); không thì có `reason`. Bảng xa hơn một bước thì chọn từ bảng đã
  thêm.
- **Một ảnh chụp nhất quán:** mọi bảng đếm dòng và chép trong một giao dịch `REPEATABLE READ READ ONLY` trên một kết
  nối, nên các bảng cùng là ảnh của một thời điểm (MVCC của Postgres): không có dòng hàng trỏ tới đơn chưa có trong bản
  chụp `orders`.
- **Một database, một dataset:** mọi bảng chụp từ một nguồn là bảng của cùng một dataset, tên theo tên database
  (`GET .../catalog`). "Chụp lại" cả dataset (`POST .../catalog/{id}/snapshot`) chép lại mọi bảng đã chụp, tối đa 30
  bảng, trong một giao dịch như chụp một nhóm, rồi thay mọi quan hệ giữa chúng.
- **Một bảng nguồn, một bảng của dataset:** bảng đã chụp từ nguồn này (tra theo `sources.config.tables`) thì lần chụp
  sau thêm phiên bản vào đúng dataset đó. Bảng chiều dùng chung, ví dụ `orders` được cả `order_items` và `returns` trỏ
  tới, nên chỉ có một dataset dù chụp trong hai nhóm khác nhau. Bảng không đổi thì không tạo phiên bản mới (`created:
  false`).
- **Quan hệ:** ghi khóa ngoại nối hai bảng trong cùng lần chụp, chỉ khi cột được trỏ tới là khóa chính hay `UNIQUE`.
  Mỗi quan hệ có dataset phía nhiều (giữ khóa ngoại) và cột của nó, dataset phía một và cột của nó, tên ràng buộc, và
  `cardinality` (`one_to_one` khi cột khóa ngoại cũng là khóa chính hay `UNIQUE`, còn lại `many_to_one`). Lần chụp sau
  của cùng nhóm thay toàn bộ quan hệ giữa các bảng trong nhóm; quan hệ với dataset ngoài nhóm giữ nguyên. Xóa một
  dataset thì quan hệ của nó mất theo. API trả quan hệ trong `relations` của mỗi dataset cho mọi thành viên.
- **Chụp lại** (`dataset_id`): chụp lại dataset đó cùng mọi dataset cùng nguồn nối với nó qua quan hệ, đi hết các bước.
- **Lỗi giữa chừng:** các bảng nạp lần lượt sau khi chép xong. Một bảng nạp lỗi thì các bảng nạp trước đó đã có phiên
  bản mới, còn quan hệ của lần này chưa ghi. Chụp lại là đủ: bảng không đổi trùng hash nội dung nên không tạo phiên bản
  trùng.
- **Hỏi:** câu thường trên một dataset của nhóm được đọc thêm các dataset nối với nó qua quan hệ (hai chiều, tối đa 2
  bước và 8 bảng), mô hình viết `JOIN` theo đúng các quan hệ đó; khung Minh chứng ghi các bảng câu đã đọc. Câu "tại
  sao" và truy dòng nguồn vẫn chỉ một bảng. Cách chọn bảng và luật cho người bị lọc dòng:
  [ARCHITECTURE.md](ARCHITECTURE.md), mục 6.

## Gộp nhiều file và cột `_source_file`

Mỗi file (hay mỗi sheet Excel) ghi thành một phần `part-NNNN.parquet` trong thư mục của phiên bản, kèm cột
`_source_file` ở cuối, ghi tên file gốc (hay tên sheet) của từng dòng. Khi đọc, DuckDB đọc cả thư mục như một bảng
(`read_parquet('<thư mục>/*.parquet', union_by_name = true)`), ghép cột theo tên. Nhờ vậy nhiều file cùng cột (mỗi tháng
một file) thành một bảng, và truy dòng nguồn ([#34](https://github.com/pgiahuy/xdi-platform/issues/34), route `.../drill`)
trả cột này cùng mỗi dòng để chỉ dòng đến từ file nào. Dataset chụp từ database ghi
`postgres://máy/database/schema.bảng`, không có tài khoản hay mật khẩu. Guard SQL đổi bảng logic thành đúng biểu thức
đọc này ([Guard SQL](GUARD.md)).

## Phiên bản và hash

Mỗi lần nạp thành công tạo một phiên bản mới của dataset, số phiên bản tăng dần, không ghi đè phiên bản cũ. Phiên bản
vừa nạp thành phiên bản hiện hành (`current_version_id` của dataset).

- Backend tính SHA-256 của từng file trong lúc nhận. Hash của cả lần nạp là SHA-256 của danh sách hash từng file đã sắp
  xếp, nên thứ tự chọn file không đổi kết quả. Với file Excel, hash tính thêm tên các sheet đã chọn và phiên bản cách
  đọc Excel của backend: cùng file mà chọn sheet khác là dữ liệu khác.
- Hash chỉ tính trên nội dung, không tính tên file.
- Nạp trùng: có `dataset_id` thì so với các phiên bản của bảng đó; không có thì so với mọi phiên bản trong workspace.
  Bảng mới chụp từ database không so cả workspace: cùng một database kết nối hai lần thì mỗi dataset có bảng riêng.
  Trùng thì không tạo gì, trả 200 với `created: false` và phiên bản đã có. Tạo mới thì trả 201.
- File Parquet nằm ở `<DATA_DIR>/<id workspace>/<id dataset>/<id bảng>/<thư mục phiên bản>/`; database lưu đường dẫn
  tương đối với `DATA_DIR`, nên phiên bản ghi trước #257 (ở `<id workspace>/<id bảng>/<thư mục phiên bản>/`) vẫn đọc
  được mà không phải chuyển file. Trong lúc nạp, file nằm ở thư mục tạm `<DATA_DIR>/staging/`, xóa khi xong hay khi lỗi;
  backend dừng giữa chừng thì lần khởi động sau xóa.
- Phiên bản lưu số dòng (`row_count`) và trạng thái hồ sơ `profile_status`: `structure` khi đã có hồ sơ cột (mục
  dưới), `semantic` khi xong lớp ngữ nghĩa, `pending` khi chưa, `failed` khi lớp cấu trúc lỗi do dữ liệu.
- Xóa dataset (`DELETE /api/workspaces/{ws}/datasets/{id}`, chỉ `admin`) xóa mọi phiên bản cùng hồ sơ cột, chỉ số và
  lịch sử câu hỏi trong Postgres, rồi xóa thư mục Parquet của từng phiên bản trên đĩa ([API](API.md), mục Xóa
  dataset). File xóa sau khi giao dịch Postgres xong: lỗi lúc xóa file chỉ để lại file thừa trên đĩa, không làm lệch
  dữ liệu.

## Dataset mẫu

Thiết lập lần đầu tạo xong workspace đầu tiên mà tới được database mẫu (`SAMPLE_DATABASE_URL`, nhiều chuỗi cách nhau
bằng dấu phẩy; compose trỏ vào `sales_demo` và `edu_demo` của dịch vụ `demo-db` trong
[data/postgres-demo](../data/postgres-demo/README.md) khi bật `--profile demo`, `make dev` trỏ vào hai database đó ở
`127.0.0.1:5440`) thì thêm mỗi database làm một nguồn Postgres và chụp các nhóm bảng ghi cho database đó (trường
`database`) trong `SAMPLES_DIR/sample_database.json`, đúng như admin làm ở mục Nguồn database phía trên: hai dataset
(`sales_demo` 6 bảng, `edu_demo` 6 bảng) kèm 11 quan hệ, cộng chỉ số mẫu của bảng có file `<bảng>.metrics.json`
(`backend/src/service/samples_db.rs`).
Một database lỗi thì giữ database còn lại. Không có database mẫu, hay không database nào chụp được, thì nạp `dataco.csv`
trong `SAMPLES_DIR` thành dataset `DataCo` qua đường nạp file, kèm các chỉ số mẫu (`backend/src/service/samples.rs`).
Tắt cả hai bằng `SEED_SAMPLE=false`. Chi tiết, nguồn và giấy phép: [data/README.md](../data/README.md).

## Tên bảng và tên cột

**Tên dataset** là trường `name`; bỏ trống thì lấy tên file đầu tiên bỏ đuôi, cắt còn 100 ký tự.

**Tên bảng logic** (`logical_table`) là tên mô hình ngôn ngữ dùng trong SQL và guard kiểm. Đặt một lần khi tạo dataset,
từ tên dataset: bỏ dấu, viết thường, mỗi dãy ký tự không phải chữ hay số thành một `_`, bỏ `_` ở hai đầu, cắt còn 48
ký tự; sau khi cắt, tên bắt đầu bằng số thì thêm `t_` ở đầu; không còn ký tự nào thì là `dataset`; trùng tên bảng khác
trong cùng dataset thì thêm `_2`, `_3`. Hai dataset khác nhau được có bảng cùng tên: hai database cùng có `orders`, hay
file `orders.csv` cạnh database có bảng `orders`, đều giữ `orders`, vì một câu hỏi chỉ đọc bảng của một dataset. Bảng
đặt tên trước #257 giữ tên cũ (có thể là `orders_2`), vì SQL đã lưu của câu hỏi cũ dùng tên đó.
Ví dụ "Bán hàng 2024" thành `ban_hang_2024`, "2024 sales" thành `t_2024_sales`.

**Tên cột**:

- CSV và Parquet: giữ như DuckDB đọc từ file, backend không đổi tên, không bỏ dấu. Tên có khoảng trắng hay dấu viết
  trong nháy kép khi dùng trong SQL (`"Order Item Total"`). Guard so tên cột không phân biệt hoa thường.
- Excel: bỏ khoảng trắng hai đầu; ô tiêu đề trống lấy chữ cột Excel (`C`, `AA`); tên trùng (không phân biệt hoa thường)
  thêm `_2`, `_3`. Tên ở danh sách sheet trùng với tên cột sau khi nạp.

Kết quả nạp (`UploadResult.columns`) trả tên và kiểu DuckDB của từng cột (`BIGINT`, `DOUBLE`, `VARCHAR`, `DATE`...),
gồm cả `_source_file`. Hồ sơ đầy đủ của từng cột lưu ở bảng `columns` (mục dưới), đọc qua route tổng quan dataset
([API](API.md), mục Tổng quan dataset); API chỉ số (`GET .../datasets/{id}/metrics`) trả tên, kiểu, vai trò, mô tả và
cách gộp của cột phiên bản hiện hành.

## Hồ sơ cột

Mỗi phiên bản có hồ sơ cột lớp cấu trúc, do code tính trên Parquet, không gọi mô hình ngôn ngữ (chạy như nhau với
`LLM_PROVIDER=mock`). Phiên bản và hồ sơ ghi trong một giao dịch, xong trước khi route nạp trả 201, với `profile_status`
là `structure`; ghi lỗi thì không còn phiên bản nào (dataset vừa tạo cho lần nạp đó cũng bị xóa), nạp lại được như lần
đầu. Nạp trùng không lập lại hồ sơ. Phiên bản nạp trước khi có bước này (`profile_status` còn `pending`) được lập hồ sơ
nền mỗi lần backend khởi động; lỗi ở một phiên bản chỉ ghi log: lỗi do dữ liệu (Parquet hỏng, tên cột không lưu được)
đặt `failed`, lỗi có thể tự hết (thiếu thư mục Parquet, Postgres lỗi) giữ `pending` để lần khởi động sau thử lại.

Mỗi cột, kể cả `_source_file`, là một dòng `columns`: kiểu DuckDB, số ô trống (`null_count`), số giá trị khác nhau đếm
chính xác (`distinct_count`), giá trị nhỏ nhất và lớn nhất dạng chữ, vai trò, và `time_format` với cột ngày dạng chữ.
Cột có từ 1 tới 200 giá trị khác nhau có thêm mỗi giá trị một dòng `column_values` kèm số dòng mang giá trị đó
(`frequency`). Postgres không lưu được ký tự NUL (mã 0) và từ chối giá trị quá dài trong chỉ mục, nên cột có giá trị
dài quá 1000 byte hoặc có ký tự NUL thì không ghi `column_values`; min, max bỏ ký tự NUL. Tên cột dài quá 1000 byte hoặc
có ký tự NUL thì lần nạp bị từ chối (400).

**Cột ngày dạng chữ.** Lúc đọc CSV (sheet Excel cũng được đổi sang CSV trước), DuckDB tự nhận một số dạng ngày và đổi
cột thành `DATE` hay `TIMESTAMP`. Dạng nào được nhận còn tùy cả file, nên cột ngày vẫn có thể là `VARCHAR`; DataCo ghi
`1/31/2018 22:56` và hai cột ngày của nó là `VARCHAR`. Với cột `VARCHAR`, hồ sơ thử một danh sách ngắn định dạng
`strptime` (`%Y-%m-%d`, `%d/%m/%Y`, `%m/%d/%Y`, có hoặc không kèm giờ). Một định dạng chỉ được tính khi đọc được mọi ô
không trống với năm từ 1900 tới 2100 (`1/2/3` hay `12/3/45` không phải ngày). Định dạng chọn được ghi vào `time_format`,
còn min và max là thời điểm đã đọc theo định dạng đó (`2015-01-01 00:00:00`). Giá trị đọc được cả ngày/tháng lẫn
tháng/ngày (mọi ngày đều không quá 12) thì giữ định dạng của cột cùng tên ở phiên bản hiện hành của dataset nếu định
dạng đó vẫn đọc được, không thì theo ngày/tháng; backend ghi log mỗi lần phải chọn như vậy. Kiểu cột trong Parquet không
đổi: SQL dùng `strptime("cột", '<time_format>')`.

**Vai trò cột** thử từng luật theo thứ tự, luật đầu tiên khớp thắng. Luật chỉ nhìn kiểu và số đo, không nhìn tên cột:

| Thứ tự | Điều kiện | Vai trò |
|---|---|---|
| 1 | Cột toàn ô trống, hoặc là `_source_file` | `ignore` |
| 2 | Kiểu `DATE`, `TIMESTAMP`, `TIMESTAMP WITH TIME ZONE`, hoặc cột chữ có `time_format` | `time` |
| 3 | Bảng hơn 2 dòng; số nguyên, không ô trống, mỗi dòng một giá trị khác nhau | `key` |
| 4 | Bảng hơn 2 dòng; số (nguyên, thực, `DECIMAL`) có tối đa 2 giá trị khác nhau, như cột cờ 0/1 | `dimension` |
| 5 | Số còn lại | `measure` |
| 6 | `BOOLEAN`, hoặc chữ có tối đa 200 giá trị khác nhau và có `column_values` | `dimension` |
| 7 | Còn lại: chữ nhiều giá trị, chữ ít giá trị mà không ghi được `column_values`, `TIME`, `UUID`, `INTERVAL`, `JSON`, `BLOB`, kiểu lồng nhau | `text` |

Bảng một hai dòng thì cột số nào cũng mỗi dòng một giá trị và có tối đa 2 giá trị, nên luật 3 và 4 không áp dụng.

Trên DataCo: `Market`, `Order Country` (164 nước) là `dimension`, `Order City` (3597 thành phố) là `text`,
`Order Item Id` là `key`, hai cột `... (DateOrders)` là `time`. Số nguyên lặp lại như `Order Id` hay `Customer Id` ra
`measure` theo luật; lớp ngữ nghĩa (mục dưới) sửa được một số ca như vậy trong phạm vi hẹp.

**Cột nhạy cảm** (`is_sensitive`) được đánh dấu ngay trong lượt lập hồ sơ cấu trúc, cùng giao dịch ghi hồ sơ, trước
khi phiên bản dùng được: tên cột có một cụm như email, mật khẩu, số điện thoại, số giấy tờ, địa chỉ, họ tên (danh
sách ở `SENSITIVE_NAME_PHRASES`, service/profile.rs) hay bắt đầu bằng "điện thoại", hoặc phần lớn giá trị của cột
(chỉ xét cột có tối đa 200 giá trị khác nhau, tức cột có danh sách giá trị) trông như email hay số điện thoại (dạng
`0912 345 678`, `(028) 3823 4567`, `+84 912 345 678`, `+1 415 555 0100`, `84912345678`). Tên cột được bỏ dấu,
chuyển chữ thường và tách từ (cả camelCase và chỗ chữ liền số, như `Address1`) rồi so theo từ, nên "Hotel" không khớp
"tel", "Dòng điện thoại" không thành nhạy cảm. Luật cố ý đơn giản và nghiêng về đánh dấu nhầm còn hơn bỏ sót. Phiên
bản nạp trước khi có luật này được quét đánh dấu lại mỗi lần khởi động (chỉ bật cờ, không tắt; đồng bộ trước khi
server nhận request). Giá trị của cột nhạy cảm không vào prompt của lớp ngữ nghĩa lẫn ngữ cảnh hỏi (#28), ngay từ khi
phiên bản ở `structure`.

## Hồ sơ lớp ngữ nghĩa

Sau khi nạp xong (route đã trả 201), backend gọi mô hình ngôn ngữ một lần cho phiên bản mới, chạy nền
(`service/semantic/mod.rs`, tối đa một lượt mỗi lúc): mô hình đề xuất mô tả và bí danh cột, đơn vị và cách gộp của
cột số đo, bí danh giá trị, câu hỏi gợi ý, cột cần đánh dấu nhạy cảm thêm, và sửa vai trò cột trong phạm vi hẹp. Xong thì
`profile_status` là `semantic`; người nạp không nhận lỗi. Nạp phiên bản mới của cùng dataset gọi lại mô hình, vì cột
của phiên bản là dòng mới. Câu trả lời không dùng được (JSON hỏng, không mục nào hợp lệ) hay lỗi `BadRequest` của
provider (Gemini chặn prompt vì nội dung, mock không có bảng) thì đánh dấu đã thử kèm tên provider
(`semantic_attempted_with`): việc khởi động không gọi lại với cùng provider, đổi `LLM_PROVIDER` thì thử lại, và nạp
lại đúng bộ file cũ luôn là đường thử lại tường minh. Mọi lỗi khác (mô hình quá tải, timeout, thiếu cấu hình,
Postgres lỗi) giữ `structure` và không đánh dấu gì: lần khởi động sau thử lại. Gemini báo câu trả lời bị cắt ở
`MAX_TOKENS`, bị dừng vì `SAFETY` và mã 400 không phải khóa sai cũng bằng lỗi tạm, nên các lỗi này được thử lại mỗi
lần khởi động dù gọi lại có thể vẫn hỏng.

Prompt chỉ gửi siêu dữ liệu, không gửi dòng dữ liệu: tên dataset, tên bảng logic, số dòng, mô tả bảng từ database
nếu có; mỗi cột gửi tên, kiểu, vai trò, tỷ lệ ô trống, số giá trị khác nhau, mô tả từ database nếu có; min, max chỉ kèm cột `time` và `measure` số thực không nhạy cảm (min, max
của cột chữ hay mã số nguyên như mã đơn, số điện thoại là dữ liệu thật); danh sách giá trị chỉ gửi cho cột `dimension`
không nhạy cảm, mỗi giá trị kèm số dòng, tối đa 50 giá trị một cột (cột nhiều hơn gửi 20 giá trị nhiều dòng nhất),
tổng tối đa 400 giá trị, bỏ giá trị dài quá 200 byte. Tên cột và giá trị in dạng chuỗi JSON nên dữ liệu không chèn
được dòng giả vào prompt. Giá trị của cột `text`, `key` hay cột nhạy cảm không bao giờ vào prompt.

Câu hỏi gợi ý: prompt xin câu khó trước, gồm hai câu "vì sao" giải thích chênh lệch của một số đo giữa hai kỳ (trong
khoảng min, max của cột thời gian) hay giữa hai giá trị có trong danh sách, rồi một câu tỷ lệ, trung bình hoặc top N,
còn lại là câu tổng hay phân rã đơn giản. Mô hình không thấy số liệu nên câu "vì sao" không được nói chiều ("tăng",
"giảm", "cao hơn"). Số đo của nó là số dòng, tổng của cột mô hình ghi cách gộp `sum`, hoặc trung bình của cột ghi
`avg` (gọi tên là trung bình); không bao giờ là tổng của cột `avg`.

Cột nhạy cảm (`is_sensitive`) do lớp cấu trúc đánh dấu ngay lúc nạp (mục trên); trong câu trả lời của lớp ngữ nghĩa,
mô hình đề xuất đánh dấu thêm qua `sensitive_columns` (chỉ chiều đặt, không bao giờ gỡ, có hiệu lực từ các lần sau).

Câu trả lời được kiểm từng mục trước khi ghi, mục sai bỏ riêng: cột và giá trị phải có thật (giá trị phải nằm trong
`column_values`), mô tả tối đa 300 ký tự, tối đa 8 bí danh mỗi mục và mỗi bí danh tối đa 60 ký tự, tối đa 5 câu gợi
ý và mỗi câu tối đa 200 ký tự; câu gợi ý còn chỗ trống của khuôn trong prompt (có `<` hay `>`) bị bỏ. Chuỗi rỗng sau
trim hay có ký tự điều khiển (NUL, xuống dòng) bị bỏ: một chuỗi như vậy nếu qua sẽ làm Postgres từ chối cả giao dịch
ghi. Bí danh không được trùng (sau khi bỏ dấu, chữ thường) với tên bất kỳ cột nào (kể cả chính cột đó), giá trị khác
cùng cột, hay bí danh đã nhận của cùng mục. Đổi vai trò chỉ nhận hai chiều: cột số nguyên đang là `measure` thành
`key` (mã lặp lại như `Order Id`) hoặc thành `dimension` khi có không quá 200 giá trị khác nhau (năm, tháng dạng số;
nhiều hơn thì cột chiều mới sẽ không có danh sách giá trị), và cột `dimension` thành `text` (đường dẫn ảnh, mô tả
dài). Mỗi đổi vai trò được ghi log (tên cột, vai trò cũ, mới). Cột thành `text` thì giá trị của nó không đi vào
prompt về sau, vì các bước sau chọn giá trị theo vai trò.

Cột đã có mô tả từ database (`description_source` là `database`, mục Nguồn database) thì mô tả đó đi vào prompt làm ngữ
cảnh cho bí danh, đơn vị, cách gộp và câu gợi ý, và luật phần cột dặn mô hình không viết mô tả cho cột đó. Mô tả mô hình
vẫn viết cho cột này bị bỏ khi kiểm câu trả lời, và câu `UPDATE` của store giữ mô tả nguồn `database` dù nhận gì; mô
tả mô hình viết ghi nguồn `model`. Chưa có màn cho người dùng sửa mô tả cột.

Đơn vị (`columns.unit`) chỉ nhận cho cột `measure` và chỉ một trong năm chữ: `money` (tiền), `quantity` (số lượng
hàng), `percent` (tỷ lệ), `duration` (số ngày, số giờ), `other`. Danh sách cố định để so được bằng dấu bằng: luật
biểu đồ sau này cần biết hai số đo cùng hay khác đơn vị, mà chữ tự do ("ngày", "days") cùng nghĩa vẫn khác chữ. Đơn
vị sai hay đặt cho cột không phải `measure` thì bỏ riêng, phần còn lại của mục vẫn giữ; chưa có đơn vị thì null.
Luật biểu đồ hiện tại ([Kiến trúc](ARCHITECTURE.md), mục 6) chưa đọc đơn vị: chỉ theo vai trò cột, và biểu đồ đường
chỉ vẽ độ đo đầu.

**Cách gộp mặc định** (`columns.aggregation`, migration `0010_column_aggregation.sql`, #185) là `sum` hay `avg`, chưa ai
chọn thì đọc là `sum`. Mô hình đề xuất cho cột `measure`: `sum` khi tổng có nghĩa (tiền, số lượng hàng), `avg` khi mỗi giá trị mô
tả chính dòng của nó (tuổi, điểm, giá, tỷ lệ, số tín chỉ, số ngày). Đề xuất sai hay cho cột khác vai trò thì bỏ riêng,
giữ `sum`. Lượt chạy nền của mô hình chỉ điền cột chưa có cách gộp, nên cách gộp admin đặt trong lúc mô hình chưa
chạy xong được giữ. Admin đổi ở màn **Chỉ số** ([API](API.md), mục Chỉ số và cách gộp) cho phiên bản hiện hành; phiên bản nạp sau
nhận đề xuất mới của mô hình, không chép từ phiên bản trước. Ba nơi đọc cách gộp: prompt sinh SQL ghi "one value per row:
aggregate it with AVG, never SUM" cạnh cột `avg` (cột `sum` không thêm chữ gì); form câu "tại sao" đổi `sum` trên cột
`avg` thành trung bình mỗi dòng (tử `SUM(cột)`, mẫu `COUNT(cột)`, như `AVG`) và phân rã thành phần cơ cấu và phần hiệu
suất; luật câu gợi ý ở trên.

Câu hỏi gợi ý lưu ở `dataset_versions.suggested_questions` (đi theo phiên bản) và trả ra trong
`GET /api/workspaces/{ws}/datasets` và `.../datasets/{id}` (của phiên bản hiện hành), cùng
`current_profile_status` để giao diện biết lớp ngữ nghĩa xong chưa và `semantic_unavailable` khi provider đang cấu
hình đã thử mà không lập được; trang hỏi hiện câu gợi ý và dòng báo theo hai trường này.

## Chưa có

- API và màn xem, sửa hồ sơ cột (vai trò, cột nhạy cảm, mô tả, bí danh); màn Chỉ số mới sửa được cách gộp.
- Chép cách gộp admin đã đặt sang phiên bản nạp sau của cùng dataset.
- So hồ sơ giữa hai phiên bản (cột mất, thêm, đổi kiểu) trước khi chuyển phiên bản hiện hành, như thiết kế ở
  [Kiến trúc](ARCHITECTURE.md), mục 7.
- Chọn lại phiên bản cũ làm hiện hành, xóa riêng một phiên bản.
- Nạp nguyên tử: ghi phiên bản vào Postgres và chuyển file Parquet về chỗ cuối chưa nằm trong một giao dịch, lỗi giữa
  hai bước có thể để lại dòng thừa trong Postgres ([#79](https://github.com/pgiahuy/xdi-platform/issues/79)).
- Nguồn dữ liệu là MySQL, SQLite, hay truy vấn thẳng database ngoài lúc hỏi; TLS tới Postgres.
- Câu "tại sao" và truy dòng nguồn qua nhiều dataset chụp cùng nhau; chặn cộng lặp khi `JOIN` bằng code
  ([#214](https://github.com/pgiahuy/xdi-platform/issues/214)).
