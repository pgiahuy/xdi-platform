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
- Chụp bảng lớn (01/11/2026): bảng 20 triệu dòng 6 cột (1,25 GB Postgres) chụp trong 8,5 giây, bảng 5 triệu dòng 60 cột
  (1,86 GB) trong 24,5 giây; trần chụp bảng đặt theo dung lượng từ các số này. Mục Chụp bảng lớn.

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

## Chụp bảng lớn

Đo ngày 01/11/2026 để đặt trần của chụp bảng (`SOURCE_TABLE_MAX_MB`, `SOURCE_SNAPSHOT_MAX_MB`, `SOURCE_TIMEOUT_SECS`) theo
dung lượng thay cho trần dòng cũ (1 triệu dòng, chưa từng đo). Cách `direct`, không gọi mô hình. Trung vị của 3 lần đo
mỗi bảng, trên code của nhánh (không trần dòng, không `count(*)`).

| Bảng | Số dòng | Số cột | Postgres | Thời gian chụp | Giao dịch trên nguồn | Parquet | Bộ nhớ đỉnh | Câu tổng hợp |
|---|---|---|---|---|---|---|---|---|
| `fact_1m` | 1 000 000 | 6 | 63 MB | 0,74 s | 0,2 s | 12 MB | 234 MiB | 0,01 s |
| `fact_5m` | 5 000 000 | 6 | 313 MB | 2,13 s | 1,6 s | 62 MB | 577 MiB | 0,04 s |
| `fact_10m` | 10 000 000 | 6 | 626 MB | 4,05 s | 3,3 s | 125 MB | 791 MiB | 0,07 s |
| `fact_20m` | 20 000 000 | 6 | 1 251 MB | 8,48 s | 6,5 s | 250 MB | 1 075 MiB | 0,14 s |
| `wide_1m` | 1 000 000 | 60 | 372 MB | 5,72 s | 3,8 s | 173 MB | 1 191 MiB | 0,04 s |
| `wide_5m` | 5 000 000 | 60 | 1 860 MB | 24,53 s | 19,3 s | 867 MB | 3 684 MiB | 0,06 s |

Chụp cả sáu bảng một lần với trần mặc định (thân rỗng, `scripts/bench-snapshot.sh defaults`, 3 lần): 5 bảng chép
(2 625 MB theo Postgres) trong 18,9 s (26,6 s lần đầu), giao dịch 15,0 s, bộ nhớ đỉnh 1 272 MiB; `wide_5m` chỉ lưu
metadata vì cộng thêm nó thì quá 4096 MB.

Đọc bảng:

- Cùng một triệu dòng, bảng 60 cột tốn gấp khoảng 8 lần thời gian và 14 lần Parquet so với bảng 6 cột: số dòng không
  nói được lần chụp nặng thế nào, dung lượng Postgres thì gần hơn. Tính trên thời gian chụp, mỗi giây được khoảng 65 đến
  155 MB theo Postgres khi bảng đã nằm trong cache của máy; lượt đầu tiên của bộ đo (cache lạnh, code trên `main`) chậm
  hơn tới khoảng 3 lần (`wide_1m` 17,6 s so với 5,5 s).
- Bộ nhớ đỉnh không nằm ở bước chép: đọc `VmRSS` mỗi 0,5 giây trong một lần chụp `wide_5m`, suốt giao dịch tiến trình
  giữ khoảng 200 MiB, rồi lên 3,6 GiB trong khoảng một giây ở bước nạp sau đó (hồ sơ cột trên file Parquet), không
  thuộc `service/sources/`. Từ khi có `DUCKDB_MEMORY_LIMIT_MB` (đo bên dưới), đỉnh này còn khoảng 1,1 GiB.
- Câu tổng hợp (tổng theo vùng và tháng) trên file Parquet chạy dưới 0,2 s ở mọi cỡ đã đo.
- Code trên `main` (còn `count(*)` khi có trần dòng, đặt `SOURCE_MAX_ROWS=4000000000`) cho số gần như nhau, ví dụ
  `fact_20m` 8,12 s, `wide_5m` 23,58 s: bỏ `count(*)` không làm nhanh hơn rõ trên các bảng này.

Chọn mặc định từ đó: `SOURCE_TABLE_MAX_MB=2048` (bảng lớn nhất đã đo vừa trần), `SOURCE_SNAPSHOT_MAX_MB=4096` (khoảng
gấp rưỡi lần chụp năm bảng trên, ước 30 đến 60 s khi cache ấm), `SOURCE_TIMEOUT_SECS` giữ 300 để còn chỗ khi cache lạnh
chậm hơn vài lần. Đây là một máy, bảng sinh tự động, nguồn và backend cùng máy (không qua mạng); database nguồn ở xa
thì thời gian chép tăng theo đường truyền.

Cách đo:

- Máy: Intel Core 7 240H (16 luồng), 30 GiB RAM, SSD NVMe, Linux 7.0. Postgres 17.11 của ứng dụng trong Docker
  (`shared_buffers` 128 MB), backend build `--release` chạy trên máy, extension `postgres` của DuckDB `v1.5.6`. Trong
  lúc đo máy còn build và test của nhánh khác chạy nền (load average 5 đến 8).
- Bảng sinh bằng `generate_series` trong một database riêng (`scripts/bench-snapshot.sh setup`): bảng hẹp gồm khóa,
  hai mã số, ngày, số tiền `numeric(12,2)`, vùng (chữ); bảng rộng gồm 20 cột số nguyên, 20 cột `numeric(12,2)`, 10 cột
  chữ, 5 cột ngày, 4 cột cờ. Không có chỉ mục, nên `pg_total_relation_size` bằng `pg_table_size`.
- Mỗi lần đo (`scripts/bench-snapshot.sh run`): database ứng dụng và `DATA_DIR` mới, một tiến trình backend mới
  (`LLM_PROVIDER=mock`, trần đặt rất cao), tạo tài khoản, thêm nguồn với `sslmode` `disable`, một request chụp một bảng.
  Thời gian chụp là thời gian request phía người gọi (chép, ghi Parquet, hồ sơ cột lớp cấu trúc, ghi phiên bản). Giao
  dịch là `now() - xact_start` lớn nhất trong `pg_stat_activity` của database đo, đọc mỗi 0,2 giây. Bộ nhớ đỉnh là
  `VmHWM` của backend sau request. Câu tổng hợp đo bằng DuckDB 1.5.6 (Python) trên file Parquet đã nạp, lần chạy thứ
  hai.
- Làm lại: `BENCH_URL=postgres://...` trỏ vào một database trống, `scripts/bench-snapshot.sh setup`, rồi `run` và
  `defaults`; biến và cột in ra ghi ở đầu script.

Trần bộ nhớ của DuckDB khi nạp và lập hồ sơ cột (`DUCKDB_MEMORY_LIMIT_MB`), đo ngày 01/11/2026 trên cùng máy (Intel Core
7 240H, 30 GiB RAM, Linux 7.0, load average 3 đến 5 vì máy còn build nhánh khác). Trước khi có trần, các kết nối DuckDB
của bước nạp (`engine/analytics.rs`), hồ sơ cột (`engine/profile.rs`) và bước chép (`service/sources/direct.rs`) chạy
không có `memory_limit`, nên DuckDB dùng tới 80% RAM của máy. Bản "trước": commit `6636eff` (`main`); bản "sau": nhánh
`fix/profile-memory-limit`, mỗi kết nối có trần và thư mục tạm riêng dưới `DATA_DIR/duckdb-spill/`. Cùng
`scripts/bench-snapshot.sh run wide_1m wide_5m`, trung vị của 3 lần, `LLM_PROVIDER=mock`.

| Bảng | Bản | Trần | Thời gian chụp | Bộ nhớ đỉnh |
|---|---|---|---|---|
| `wide_1m` | Trước | không có | 5,15 s | 1 204 MiB |
| `wide_1m` | Sau | 1024 MiB (mặc định) | 5,29 s | 611 MiB |
| `wide_5m` | Trước | không có | 22,51 s | 3 619 MiB |
| `wide_5m` | Sau | 1024 MiB (mặc định) | 24,38 s | 1 134 MiB |

Một lần đo mỗi bảng với trần khác: 512 MiB cho `wide_5m` 685 MiB trong 28,10 s (`wide_1m` 426 MiB, 6,21 s); 2048 MiB cho
`wide_5m` 1 679 MiB trong 22,83 s (`wide_1m` 1 003 MiB, 5,23 s). Trần thấp hơn thì DuckDB ghi nhiều hơn ra thư mục tạm và
chậm hơn; mặc định 1024 MiB giảm đỉnh của `wide_5m` khoảng 3 lần, đổi lại chậm hơn khoảng 8%. Đỉnh vượt trần một ít vì
trần chỉ tính bộ nhớ DuckDB quản lý, không tính phần còn lại của tiến trình. Mỗi kết nối một trần: hai lần nạp cùng lúc
thì tới khoảng gấp đôi.

## Đo lại

- Mỗi response của câu hỏi có `model_calls` (loại việc, thời gian, token); tab **Các bước** của khung Minh chứng hiện
  danh sách này.
- Log của backend có một dòng `llm call` mỗi lượt gọi, một dòng `question done` mỗi câu hỏi và một dòng
  `semantic profile done` mỗi lượt lập hồ sơ.
- `xdi-backend llm-stats [--since <thời điểm RFC 3339>] [--price-in <USD> --price-out <USD>]` đọc lịch sử câu hỏi và in
  thời gian, lượt gọi, token theo loại câu và theo loại việc; có giá thì in chi phí trung bình mỗi câu.
