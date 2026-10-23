# Dữ liệu

Tài liệu này ghi hệ thống nhận file dữ liệu nào, đổi chúng thành gì, giữ phiên bản ra sao và đặt tên bảng, tên cột thế
nào. Cách nạp trên giao diện và bằng `curl`: [Cài đặt](INSTALL.md), mục Nạp dữ liệu. Dữ liệu nằm ở đâu và gửi gì cho
mô hình ngôn ngữ: [Dữ liệu và quyền riêng tư](PRIVACY.md).

Nạp file: `POST /api/workspaces/{ws}/datasets`, dạng `multipart/form-data`, chỉ `admin` của workspace gọi được
([API](API.md)). Các trường: `file` (lặp lại để gửi nhiều file), `name` (tên dataset mới), `dataset_id` (thêm phiên bản
cho dataset có sẵn), `sheet` (sheet cần nạp của file Excel). Code: `backend/src/api/datasets.rs`,
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

## Gộp nhiều file và cột `_source_file`

Mỗi file (hay mỗi sheet Excel) ghi thành một phần `part-NNNN.parquet` trong thư mục của phiên bản, kèm cột
`_source_file` ở cuối, ghi tên file gốc (hay tên sheet) của từng dòng. Khi đọc, DuckDB đọc cả thư mục như một bảng
(`read_parquet('<thư mục>/*.parquet', union_by_name = true)`), ghép cột theo tên. Nhờ vậy nhiều file cùng cột (mỗi tháng
một file) thành một bảng, và truy dòng nguồn ([#34](https://github.com/pgiahuy/xdi-platform/issues/34), chưa làm)
sẽ dùng cột này để chỉ dòng đến từ file nào. Guard SQL đổi bảng logic thành đúng biểu thức
đọc này ([Guard SQL](GUARD.md)).

## Phiên bản và hash

Mỗi lần nạp thành công tạo một phiên bản mới của dataset, số phiên bản tăng dần, không ghi đè phiên bản cũ. Phiên bản
vừa nạp thành phiên bản hiện hành (`current_version_id` của dataset).

- Backend tính SHA-256 của từng file trong lúc nhận. Hash của cả lần nạp là SHA-256 của danh sách hash từng file đã sắp
  xếp, nên thứ tự chọn file không đổi kết quả. Với file Excel, hash tính thêm tên các sheet đã chọn và phiên bản cách
  đọc Excel của backend: cùng file mà chọn sheet khác là dữ liệu khác.
- Hash chỉ tính trên nội dung, không tính tên file.
- Nạp trùng: có `dataset_id` thì so với các phiên bản của dataset đó; không có thì so với mọi phiên bản trong workspace.
  Trùng thì không tạo gì, trả 200 với `created: false` và phiên bản đã có. Tạo mới thì trả 201.
- File Parquet nằm ở `<DATA_DIR>/<id workspace>/<id dataset>/<thư mục phiên bản>/`; database lưu đường dẫn tương đối
  với `DATA_DIR`. Trong lúc nạp, file nằm ở thư mục tạm `<DATA_DIR>/staging/`, xóa khi xong hay khi lỗi; backend dừng
  giữa chừng thì lần khởi động sau xóa.
- Phiên bản lưu số dòng (`row_count`) và trạng thái hồ sơ `profile_status`, hiện luôn là `pending` vì chưa lập hồ sơ
  cột.

## Tên bảng và tên cột

**Tên dataset** là trường `name`; bỏ trống thì lấy tên file đầu tiên bỏ đuôi, cắt còn 100 ký tự.

**Tên bảng logic** (`logical_table`) là tên mô hình ngôn ngữ dùng trong SQL và guard kiểm. Đặt một lần khi tạo dataset,
từ tên dataset: bỏ dấu, viết thường, mỗi dãy ký tự không phải chữ hay số thành một `_`, bỏ `_` ở hai đầu, cắt còn 48
ký tự; sau khi cắt, tên bắt đầu bằng số thì thêm `t_` ở đầu; không còn ký tự nào thì là `dataset`; trùng tên bảng khác
trong workspace thì thêm `_2`, `_3`. Ví dụ "Bán hàng 2024" thành `ban_hang_2024`, "2024 sales" thành `t_2024_sales`.

**Tên cột**:

- CSV và Parquet: giữ như DuckDB đọc từ file, backend không đổi tên, không bỏ dấu. Tên có khoảng trắng hay dấu viết
  trong nháy kép khi dùng trong SQL (`"Order Item Total"`). Guard so tên cột không phân biệt hoa thường.
- Excel: bỏ khoảng trắng hai đầu; ô tiêu đề trống lấy chữ cột Excel (`C`, `AA`); tên trùng (không phân biệt hoa thường)
  thêm `_2`, `_3`. Tên ở danh sách sheet trùng với tên cột sau khi nạp.

Kết quả nạp (`UploadResult.columns`) trả tên và kiểu DuckDB của từng cột (`BIGINT`, `DOUBLE`, `VARCHAR`, `DATE`...),
gồm cả `_source_file`. Danh sách này chưa được lưu vào Postgres.

## Chưa có

- Hồ sơ cột lớp cấu trúc: vai trò cột (chiều, độ đo, thời gian), min, max, số ô trống, giá trị mẫu, lưu vào bảng
  `columns` và `column_values` ([#22](https://github.com/pgiahuy/xdi-platform/issues/22)). Hai bảng đã có trong schema
  nhưng lúc nạp chưa ghi gì.
- Hồ sơ lớp ngữ nghĩa: mô tả cột, bí danh cột và bí danh giá trị (ví dụ "miền Nam" cho `South`)
  ([#23](https://github.com/pgiahuy/xdi-platform/issues/23)).
- So hồ sơ giữa hai phiên bản (cột mất, thêm, đổi kiểu) trước khi chuyển phiên bản hiện hành, như thiết kế ở
  [Kiến trúc](ARCHITECTURE.md), mục 7.
- Xóa dataset, chọn lại phiên bản cũ làm hiện hành.
- Nạp nguyên tử: ghi phiên bản vào Postgres và chuyển file Parquet về chỗ cuối chưa nằm trong một giao dịch, lỗi giữa
  hai bước có thể để lại dòng thừa trong Postgres ([#79](https://github.com/pgiahuy/xdi-platform/issues/79)).
- Nguồn dữ liệu là database (Postgres, MySQL, SQLite): mới có trong thiết kế.
