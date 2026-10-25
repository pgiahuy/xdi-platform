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
- Phiên bản lưu số dòng (`row_count`) và trạng thái hồ sơ `profile_status`: `structure` khi đã có hồ sơ cột (mục
  dưới), `semantic` khi xong lớp ngữ nghĩa, `pending` khi chưa, `failed` khi lớp cấu trúc lỗi do dữ liệu.

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
gồm cả `_source_file`. Hồ sơ đầy đủ của từng cột lưu ở bảng `columns` (mục dưới), chưa có API đọc.

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

## Hồ sơ lớp ngữ nghĩa

Sau khi nạp xong (route đã trả 201), backend gọi mô hình ngôn ngữ một lần cho phiên bản mới, chạy nền
(`service/semantic.rs`, tối đa một lượt mỗi lúc): mô hình đề xuất mô tả và bí danh cột, bí danh giá trị, câu hỏi gợi
ý, cột cần đánh dấu nhạy cảm thêm, và sửa vai trò cột trong phạm vi hẹp. Xong thì `profile_status` là `semantic`; mô
hình lỗi hay trả lời hỏng thì giữ `structure`, ghi log và đánh dấu đã thử (`semantic_attempted_at`), người nạp không
nhận lỗi. Nạp phiên bản mới của cùng dataset gọi lại mô hình, vì cột của phiên bản là dòng mới. Hai đường thử lại cho
phiên bản còn ở `structure`: mỗi lần khởi động, backend chạy lớp ngữ nghĩa cho các phiên bản chưa từng thử (gồm cả
phiên bản nạp trước khi có bước này); và nạp lại đúng bộ file cũ thì gọi lại một lần nữa. Phiên bản đã thử mà lỗi
không bị việc khởi động gọi lại, để lỗi vĩnh viễn (thiếu khóa, mock không có bảng) không lặp mỗi lần mở máy.

Prompt chỉ gửi siêu dữ liệu, không gửi dòng dữ liệu: mỗi cột gửi tên, kiểu, vai trò, tỷ lệ ô trống, số giá trị khác
nhau; min, max chỉ kèm cột `time` và `measure` số thực (min, max của cột chữ hay mã số nguyên như mã đơn, số điện
thoại là dữ liệu thật); danh sách giá trị chỉ gửi cho cột `dimension` không nhạy cảm, tối đa 50 giá trị một cột (cột
nhiều hơn gửi 20 giá trị nhiều dòng nhất), tổng tối đa 400 giá trị, bỏ giá trị dài quá 200 byte. Tên cột và giá trị
in dạng chuỗi JSON nên dữ liệu không chèn được dòng giả vào prompt. Giá trị của cột `text`, `key` hay cột nhạy cảm
không bao giờ vào prompt.

Cột nhạy cảm (`is_sensitive`) được đặt ở hai nơi: trước khi dựng prompt, code đánh dấu theo luật đơn giản — tên cột
(bỏ dấu, chữ thường) chứa một cụm như email, mật khẩu, số điện thoại, số giấy tờ, địa chỉ, họ tên (danh sách ở
`SENSITIVE_NAME_PARTS`), hoặc phần lớn giá trị của cột trông như email hay số điện thoại — và ghi ngay vào Postgres,
không phụ thuộc mô hình chạy được không; trong câu trả lời, mô hình đề xuất đánh dấu thêm qua `sensitive_columns`
(chỉ chiều đặt, không bao giờ gỡ, có hiệu lực từ các lần sau). Luật tên cố ý đơn giản và nghiêng về đánh dấu nhầm còn
hơn bỏ sót.

Câu trả lời được kiểm từng mục trước khi ghi, mục sai bỏ riêng: cột và giá trị phải có thật (giá trị phải nằm trong
`column_values`), mô tả tối đa 300 ký tự, tối đa 8 bí danh mỗi mục và mỗi bí danh tối đa 60 ký tự, tối đa 5 câu gợi
ý và mỗi câu tối đa 200 ký tự. Chuỗi rỗng sau trim hay có ký tự điều khiển (NUL, xuống dòng) bị bỏ — một chuỗi như
vậy nếu qua sẽ làm Postgres từ chối cả giao dịch ghi. Bí danh không được trùng (sau khi bỏ dấu, chữ thường) với tên
cột khác, giá trị khác cùng cột, hay bí danh đã nhận của cùng mục. Đổi vai trò chỉ nhận hai chiều: cột số nguyên đang
là `measure` thành `key` (mã lặp lại như `Order Id`) hoặc thành `dimension` khi có không quá 200 giá trị khác nhau
(năm, tháng dạng số; nhiều hơn thì cột chiều mới sẽ không có danh sách giá trị), và cột `dimension` thành `text`
(đường dẫn ảnh, mô tả dài). Mỗi đổi vai trò được ghi log (tên cột, vai trò cũ, mới). Cột thành `text` thì giá trị của
nó không đi vào prompt về sau, vì các bước sau chọn giá trị theo vai trò.

Câu hỏi gợi ý lưu ở `dataset_versions.suggested_questions` (đi theo phiên bản) và trả ra trong
`GET /api/workspaces/{slug}/datasets` và `.../datasets/{id}` (của phiên bản hiện hành), cùng
`current_profile_status` để giao diện biết lớp ngữ nghĩa xong chưa; trang hỏi hiện chúng ở
[#50](https://github.com/pgiahuy/xdi-platform/issues/50).

## Chưa có

- API và màn xem, sửa hồ sơ cột (vai trò, cột nhạy cảm, mô tả, bí danh).
- So hồ sơ giữa hai phiên bản (cột mất, thêm, đổi kiểu) trước khi chuyển phiên bản hiện hành, như thiết kế ở
  [Kiến trúc](ARCHITECTURE.md), mục 7.
- Xóa dataset, chọn lại phiên bản cũ làm hiện hành.
- Nạp nguyên tử: ghi phiên bản vào Postgres và chuyển file Parquet về chỗ cuối chưa nằm trong một giao dịch, lỗi giữa
  hai bước có thể để lại dòng thừa trong Postgres ([#79](https://github.com/pgiahuy/xdi-platform/issues/79)).
- Nguồn dữ liệu là database (Postgres, MySQL, SQLite): mới có trong thiết kế.
