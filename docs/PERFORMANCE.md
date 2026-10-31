# Thời gian và số lượt gọi mô hình

Đo ngày 31/10/2026 với Gemini `gemini-3.5-flash-lite`, trên database mẫu đầy đủ (`data/postgres-demo`). Kết quả
chính:

- Một câu hỏi thường mất khoảng 4 đến 4,5 giây, gồm 3 lượt gọi mô hình chạy nối tiếp (phân loại, viết SQL, viết lời
  giải thích), thêm một lượt khi SQL phải sửa. Từ #241, lời do mô hình viết còn qua một lượt chấm: đo lại ngày 31/10
  (mục Câu hỏi), lượt chấm mất trung vị 1,04 giây và khoảng 0,0003 USD mỗi câu. Câu "vì sao" trên code hiện tại (có cổng chấm của #240) gồm 4 lượt
  (phân loại, điền form, viết lời, chấm lời) và mất khoảng 5,7 giây. Gần hết thời gian là chờ mô hình: phần còn lại
  (đọc hồ sơ, guard, chạy SQL trên DuckDB, phân rã, ghi lịch sử) khoảng 60 đến 130 ms (hiệu hai trung vị).
- Theo giá trả phí của mô hình này, một câu thường tốn khoảng 0,003 USD, một câu "vì sao" khoảng 0,005 USD. Gói miễn
  phí không tính tiền nhưng giới hạn số lượt gọi.
- Lập hồ sơ cột sau khi chụp bảng: nhóm `orders` và `order_items` dùng được sau 8,5 giây thay vì 14,1 giây. Bảng
  `students` không nhanh hơn (5,1 giây trước, 5,5 giây sau) và tốn token ra gấp khoảng 2,4 lần.
- Chụp bảng (#246): DuckDB đọc thẳng Postgres (`SOURCE_SNAPSHOT_MODE=direct`, nay là mặc định) nhanh hơn cách
  `COPY` ra CSV: `sales.orders` 0,37 giây thay vì 0,90 giây, `sales.order_items` 0,45 thay vì 1,73 giây,
  `edu.student_daily_clicks` (1,8 triệu dòng) 0,89 thay vì 15,9 giây; đĩa tạm ít hơn khoảng 3 đến 4 lần. Mục Chụp bảng.

Số liệu dưới đây là trung vị của ít lần đo (ghi rõ ở từng bảng), trên một máy, trong một buổi. Thời gian trả lời của
Gemini thay đổi theo tải: trong cùng buổi đo, có một lượt đo mà mỗi lượt gọi mất 7 đến 10 giây và có lượt bị trả lỗi
503 (mô hình quá tải); lượt đó đã bỏ và đo lại. Lượt đo trên code cuối của nhánh giữ nguyên cả lỗi, ghi ở mục Câu hỏi.

## Cách đo

- Máy: Intel Core 7 240H (16 luồng), 30 GiB RAM, Linux. Backend build `--release`, chạy trên máy, Postgres của
  ứng dụng trong Docker.
- Dữ liệu: database mẫu `company_demo` (bản trước khi tách thành `sales_demo` và `edu_demo`, cùng bảng và cùng dữ
  liệu), chụp `sales.orders` (65 752 dòng) kèm `sales.order_items` (180 519 dòng), và `edu.students` (32 593 dòng). Câu hỏi hỏi trên `orders`, được nối sang `order_items` khi cần doanh thu.
- Bản "trước": commit `04289e0` (main tới #239, thêm phần đo của #184, chưa chạy song song), `SEMANTIC_JOBS=1` như
  semaphore cũ. Bản "sau": commit `be76ef8` (chạy song song), `SEMANTIC_JOBS=2` mặc định; vòng đo hồ sơ thứ 2 và 3
  thêm dòng log thời gian từng phần, không đổi cách chạy.
- Hai bản chạy cùng lúc trên hai cổng, mỗi bản một database và thư mục dữ liệu riêng, hỏi xen kẽ từng câu (trước rồi
  sau, nghỉ 12 giây giữa hai lần hỏi) để cả hai gặp cùng tải của Gemini.
- Câu hỏi: 5 câu thường ("Doanh thu theo thị trường năm 2017", "Doanh thu theo tháng năm 2017 của Europe", "Số đơn
  theo trạng thái giao hàng", "Tỷ lệ giao trễ theo phương thức vận chuyển", "Top 5 quốc gia có nhiều đơn nhất năm
  2016") và 3 câu "vì sao" ("Vì sao tỷ lệ giao trễ tháng 11/2016 tăng so với tháng 10/2016?", "Vì sao số đơn tháng
  12/2016 thay đổi so với tháng 11/2016?", "Vì sao doanh thu năm 2017 khác nhau giữa Europe và LATAM?"), mỗi câu hỏi
  2 lần trên mỗi bản.
- Thời gian, số lượt gọi và số token lấy từ `model_calls` của response (`usageMetadata` của Gemini, token suy nghĩ
  tính vào token ra). Tổng thời gian là thời gian request HTTP phía người gọi.
- Chi phí = token vào x 0,30 USD + token ra x 2,50 USD cho một triệu token, giá trả phí của Gemini 3.5 Flash-Lite trên
  https://ai.google.dev/gemini-api/docs/pricing, xem ngày 31/10/2026.

## Câu hỏi

Trung vị thời gian, trung bình lượt gọi, token và chi phí; chỉ tính câu `answered`.

| Loại câu | Bản | Số câu | Tổng thời gian | Chờ mô hình | Lượt gọi | Token vào | Token ra | Chi phí mỗi câu |
|---|---|---|---|---|---|---|---|---|
| Câu thường | Trước | 10 | 4,55 s | 4,45 s | 3,0 | 6 763 | 439 | 0,0031 USD |
| Câu thường | Sau | 9 | 4,56 s | 4,51 s | 2,9 | 6 855 | 467 | 0,0032 USD |
| Câu "vì sao" | Trước | 6 | 3,71 s | 3,58 s | 2,7 | 5 125 | 315 | 0,0023 USD |
| Câu "vì sao" | Sau | 6 | 4,48 s | 4,37 s | 2,7 | 5 301 | 374 | 0,0025 USD |

Một câu thường của bản sau bị từ chối ("Số đơn theo trạng thái giao hàng", lần đầu): hồ sơ cột do mô hình viết lại khi
chụp bảng nên khác giữa hai bản, prompt viết SQL vì thế cũng khác. Chênh lệch thời gian giữa hai bản ở bảng trên nằm
trong độ dao động của Gemini, không do code: hai bản gọi mô hình giống nhau cho câu hỏi. Phần code đổi cho câu hỏi là
các câu SQL từng chiều của phân rã chạy cùng lúc; bước `explain` của câu "vì sao" giảm từ 71 ms xuống 49 ms (trung vị).

Đo thêm một lượt trên code cuối của nhánh (commit `59948bf`, đã gộp #240), bản sau, mỗi câu một lần:

| Loại câu | Số câu | Tổng thời gian | Chờ mô hình | Lượt gọi | Token vào | Token ra | Chi phí mỗi câu |
|---|---|---|---|---|---|---|---|
| Câu thường | 4 | 3,87 s | 3,81 s | 3,25 | 7 272 | 519 | 0,0035 USD |
| Câu "vì sao" | 3 | 5,66 s | 5,56 s | 4,0 | 7 958 | 1 011 | 0,0049 USD |

Sau lượt đo này, #241 thêm cổng chấm (`judge`) cho lời của câu thường: lời do mô hình viết có thêm một lượt gọi mô
hình. Đo lại ngày 31/10/2026 tối, trên nhánh `feat/answer-clarity` (sau #243, chưa commit), cùng 5 câu thường, mỗi câu 2
lần, nghỉ 12 giây giữa hai lần hỏi, chỉ một bản chạy. Khác với cách đo ở trên: backend build debug (`cargo run` của
`make dev`), Postgres của ứng dụng chạy bằng Podman, các bảng `orders`, `order_items`, `customers`, `products` chụp từ
database mẫu nạp lại bằng `load.sh` nên hồ sơ cột do mô hình viết lại. Vì build debug, cột "Tổng thời gian" không so
được với bảng trên; số lượt gọi, token và thời gian từng lượt gọi thì so được.

| Loại câu | Số câu | Tổng thời gian | Chờ mô hình | Lượt gọi | Token vào | Token ra | Chi phí mỗi câu |
|---|---|---|---|---|---|---|---|
| Câu thường | 10 | 6,34 s | 6,15 s | 4,1 | 7 453 | 499 | 0,0035 USD |

Cả 10 câu `answered`, cả 10 lời do mô hình viết được chấm `match`; một câu ("Top 5 quốc gia có nhiều đơn nhất năm
2016", lần hai) viết lời hai lần vì lần đầu trượt bước kiểm của code, nên trung bình 4,1 lượt. Theo loại việc:

| Lượt gọi | Số lượt | Thời gian | Token vào | Token ra |
|---|---|---|---|---|
| `classify` | 10 | 1,41 s | 2 068 | 75 |
| `sql` | 10 | 2,44 s | 3 733 | 273 |
| `insight` | 11 | 1,06 s | 645 | 120 |
| `judge` (chấm lời) | 10 | 1,04 s | 943 | 18 |

Lượt chấm thêm trung vị 1,04 giây và trung bình 0,00033 USD mỗi câu. Trong buổi đo này Gemini chậm hơn buổi đo ở trên:
`classify` mất 1,41 giây so với 1,22 giây, nên phần lớn chênh lệch của cột "Chờ mô hình" so với bảng trên (6,15 giây so
với 3,81 giây) đến từ các lượt gọi vốn có, không chỉ từ lượt chấm. Một lượt đo trước đó cùng buổi, trên DataCo đầy đủ
nạp từ CSV (180 519 dòng, một bảng), ra lượt chấm trung vị 0,82 giây, 917 token vào.

Câu thứ năm ("Tỷ lệ giao trễ theo phương thức vận chuyển") lỗi 502 sau 61 giây: lượt viết SQL chờ hết
`GEMINI_TIMEOUT_SECS` (60 giây) mà Gemini không trả lời, lần hỏi lại trả 503. Cùng lượt đo, phần cột của hồ sơ
`orders` mất 57 giây cho một lượt gọi thường mất 2 đến 4 giây. Thời gian trung vị vì thế không nói hết: thỉnh thoảng
một lượt gọi chậm gấp chục lần.

Theo loại việc, bản trước (16 câu, bản sau tương tự):

| Lượt gọi | Số lượt | Thời gian | Token vào | Token ra |
|---|---|---|---|---|
| `classify` (phân loại câu) | 16 | 1,22 s | 1 600 | 70 |
| `sql` (viết SQL) | 10 | 2,09 s | 4 645 | 257 |
| `insight` (lời giải thích câu thường) | 10 | 1,08 s | 523 | 109 |
| `why` (điền form câu "vì sao") | 6 | 1,69 s | 2 549 | 228 |
| `narrate` (lời phân rã) | 4 | 0,77 s | 1 452 | 33 |

## Lập hồ sơ cột

Ba vòng, mỗi vòng chụp lại từ database trống. "Dùng được sau" tính từ lúc gửi request chụp bảng tới khi mọi bảng
của nhóm có hồ sơ ngữ nghĩa (chụp bảng mất 2,2 đến 2,4 giây cho nhóm `orders`, 0,4 đến 0,5 giây cho `students`).

| Bảng | Bản | Thời gian lập hồ sơ | Lượt gọi | Token vào | Token ra | Chi phí |
|---|---|---|---|---|---|---|
| `orders` | Trước | 8,3 s | 1 | 2 046 | 3 765 | 0,0100 USD |
| `orders` | Sau | 6,3 s | 3 | 4 810 | 3 521 | 0,0102 USD |
| `order_items` | Trước | 3,4 s | 1 | 1 187 | 937 | 0,0027 USD |
| `order_items` | Sau | 2,6 s | 2 | 1 700 | 876 | 0,0027 USD |
| `students` | Trước | 5,1 s | 1 | 1 665 | 1 360 | 0,0039 USD |
| `students` | Sau | 5,5 s | 3 | 3 667 | 3 216 | 0,0091 USD |
| Nhóm `orders`, `order_items` dùng được sau | Trước | 14,1 s | | | | |
| Nhóm `orders`, `order_items` dùng được sau | Sau | 8,5 s | | | | |

Ở vòng 2 và 3 (có log thời gian từng phần), phần chậm nhất của bản sau luôn là phần bí danh giá trị (4,6 đến 6,3
giây): khi chỉ phải viết bí danh, mô hình viết
nhiều bí danh hơn so với khi viết cả hồ sơ trong một câu trả lời, nên token ra của `students` tăng. `order_items` không
có cột phân loại nào có danh sách giá trị nên không gọi phần này.

## Cái gì chạy song song, cái gì không

- Lập hồ sơ: ba prompt (cột; bí danh giá trị; câu gợi ý) chạy cùng lúc, mỗi prompt có JSON Schema riêng; một phần lỗi
  thì giữ các phần còn lại (`backend/src/service/semantic/parts.rs`).
- Nhiều bảng: tối đa `SEMANTIC_JOBS` lượt lập hồ sơ cùng lúc, mặc định 2, tức tối đa 6 request tới mô hình cùng lúc.
- Câu "vì sao": các câu SQL từng chiều gửi cùng lúc, mỗi câu vẫn chờ lượt trong giới hạn 2 câu DuckDB cùng lúc.
- Không làm: các lượt gọi của một câu hỏi phụ thuộc nhau (phân loại, rồi viết SQL hay điền form, rồi viết lời) nên
  vẫn nối tiếp; không chạy trước đường câu thường song song với form "vì sao", vì tốn gấp đôi lượt gọi cho mọi câu
  "vì sao".

## Chụp bảng: copy và direct

Đo ngày 31/10/2026 (#246), không gọi mô hình. Trung vị của 3 lần đo cho mỗi bảng và mỗi cách; đĩa tạm và bộ nhớ đỉnh ghi
giá trị lớn nhất trong 3 lần, lần lượt là trung vị.

| Bảng | Số dòng | Cách | Thời gian (3 lần) | Trung vị | Đĩa tạm đỉnh | Bộ nhớ đỉnh (trung vị) |
|---|---|---|---|---|---|---|
| `sales.orders` | 65 752 | copy | 0,91; 0,90; 0,88 s | 0,90 s | 12,5 MB | 106 MiB |
| `sales.orders` | 65 752 | direct | 0,38; 0,37; 0,36 s | 0,37 s | 4,1 MB | 110 MiB |
| `sales.order_items` | 180 519 | copy | 1,73; 1,65; 1,77 s | 1,73 s | 19,2 MB | 132 MiB |
| `sales.order_items` | 180 519 | direct | 0,45; 0,44; 0,46 s | 0,45 s | 6,1 MB | 125 MiB |
| `edu.student_daily_clicks` | 1 808 119 | copy | 15,92; 15,91; 19,20 s | 15,92 s | 51,4 MB | 507 MiB |
| `edu.student_daily_clicks` | 1 808 119 | direct | 0,88; 1,05; 0,89 s | 0,89 s | 11,7 MB | 326 MiB |

Hai cách cho cùng số dòng và cùng tên, kiểu từng cột trên cả ba bảng, trong cả 18 lần đo; không lần nào `direct` phải quay
về `copy`. So từng dòng của hai cách nằm ở test `both_modes_give_the_same_types_and_rows`
(`backend/src/service/sources/direct_tests.rs`), trên một bảng có đủ loại kiểu Postgres (số nguyên, số thực, `numeric`,
ngày giờ, chữ có dấu phẩy, ngoặc kép và xuống dòng, `char(n)`, `uuid`, `jsonb`, mảng, `interval`, `bytea`, enum, NULL và
chuỗi rỗng).

Cách đo:

- Máy như mục Cách đo ở trên. Backend build `--release` từ nhánh của #246 (commit `fba6129` cộng thay đổi đặt
  `ISOLATION_LEVEL` khi ATTACH), chạy trên máy; Postgres của ứng dụng và database mẫu `company_demo` (trước khi tách)
  (`data/postgres-demo`, `127.0.0.1:5440`, tài khoản `analyst`) trong Docker. Extension `postgres` của DuckDB `v1.5.6`,
  nạp từ file (`make duckdb-extension`).
- Mỗi lần đo: tạo lại database ứng dụng và `DATA_DIR` trống, khởi động một tiến trình backend mới (`LLM_PROVIDER=mock`,
  `SEED_SAMPLE=false`, `SOURCE_MAX_ROWS=2000000` để chụp được `student_daily_clicks`, `MAX_UPLOAD_MB=4000`), tạo tài
  khoản, thêm nguồn với `sslmode` `disable`, rồi gửi một request chụp một bảng. Hai cách chạy xen kẽ (copy rồi direct)
  cho từng lần. Trong lúc đo, máy còn một bản build của nhánh khác chạy nền (load average 2 đến 5).
- Thời gian: thời gian request `POST .../snapshot` phía người gọi, gồm chép bảng, đổi sang Parquet, lập hồ sơ cột lớp
  cấu trúc và ghi phiên bản; không gồm lập hồ sơ bằng mô hình (chạy nền sau đó).
- Đĩa tạm: tổng dung lượng file trong `DATA_DIR/staging`, đọc mỗi 20 ms trong lúc request chạy, lấy giá trị lớn nhất.
  Với `copy` là file CSV cộng Parquet; với `direct` là Parquet đọc thẳng cộng Parquet cuối.
- Bộ nhớ: `VmHWM` của tiến trình backend sau request, tức bộ nhớ đỉnh của cả tiến trình kể từ lúc khởi động (gồm cả
  phần nền của backend, không riêng bước chụp).
- Script đo không nằm trong repo; mỗi lần đo chỉ dùng HTTP API và `/proc`, làm lại được theo các bước trên.

## Đo lại

- Mỗi response của câu hỏi có `model_calls` (loại việc, thời gian, token); tab **Các bước** của khung Minh chứng hiện
  danh sách này.
- Log của backend có một dòng `llm call` mỗi lượt gọi, một dòng `question done` mỗi câu hỏi và một dòng
  `semantic profile done` mỗi lượt lập hồ sơ.
- `xdi-backend llm-stats [--since <thời điểm RFC 3339>] [--price-in <USD> --price-out <USD>]` đọc lịch sử câu hỏi và in
  thời gian, lượt gọi, token theo loại câu và theo loại việc; có giá thì in chi phí trung bình mỗi câu.
