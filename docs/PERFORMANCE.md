# Thời gian và số lượt gọi mô hình

Trạng thái: số đo câu hỏi và lập hồ sơ cột lấy ngày 31/10/2026 với Gemini `gemini-3.5-flash-lite`, trên database mẫu
cũ `company_demo` (cùng bảng và dữ liệu với `sales_demo`, `edu_demo` hiện nay), hỏi trên một bảng bắt đầu. Code hỏi
đã đổi sau các lượt đo này (hỏi trên mọi bảng của database, mô tả từ `COMMENT ON` trong prompt, kiểm nghĩa cho câu
hỏi số liệu, guard kiểm JOIN); chưa đo lại trên `main` hiện tại. Số đo chụp bảng lấy ngày 31/10 và 01/11/2026, không
gọi mô hình. Mỗi bảng dưới đây ghi commit, bản build và số câu.

Tóm tắt:

- Câu hỏi số liệu trên commit `59948bf` (chưa có lượt kiểm nghĩa): trung vị 3,9 giây, 3,25 lượt gọi, khoảng 0,0035 USD.
  Lượt kiểm nghĩa đo sau đó thêm trung vị 1,0 giây và khoảng 0,0003 USD mỗi câu.
- Câu hỏi nguyên nhân trên `59948bf`: trung vị 5,7 giây, 4 lượt gọi, khoảng 0,005 USD.
- Gần hết thời gian là chờ mô hình; phần còn lại (đọc hồ sơ, guard, chạy SQL, phân rã, ghi lịch sử) khoảng 60 đến 130
  ms (hiệu hai trung vị).
- Chụp bảng bằng `direct` nhanh hơn `copy` 2,4 đến 18 lần trên ba bảng đã đo. Bảng 20 triệu dòng 6 cột (1,25 GB theo
  Postgres) chụp trong 8,5 giây; bảng 5 triệu dòng 60 cột (1,86 GB) trong 24,5 giây.

Thời gian của Gemini dao động theo tải: trong các buổi đo có lượt gọi chậm gấp chục lần thường lệ, có lượt chờ hết
`GEMINI_TIMEOUT_SECS` (60 giây) rồi lỗi 502. Trung vị không nói hết các trường hợp đó.

## Số lượt gọi mô hình theo loại câu

Đếm theo code hiện tại, không phải số đo.

| Loại câu | Thường | Thêm khi |
|---|---|---|
| Câu hỏi số liệu | 4: phân loại, viết SQL, viết lời, kiểm nghĩa | Sửa SQL (+1); gửi lại kèm bảng chỉ có tên (+1); gửi lại kèm đủ giá trị (+1, có thể thêm một lần sửa); lời trượt kiểm thì viết lại và kiểm nghĩa lại (+2) |
| Câu hỏi số liệu không có lời | 2: phân loại, viết SQL | Danh sách dòng, nhóm theo cột nhạy cảm hay cột khóa, `INSIGHT_TEXT=false` |
| Câu hỏi nguyên nhân | 4: phân loại, điền form, viết lời, kiểm nghĩa | Sửa form (+1); lời trượt thì viết lại và kiểm nghĩa lại (+2). Chênh lệch bằng 0 thì không viết lời. Form sai hai lần thì hỏi như câu số liệu, không có lời |
| Báo cáo tự động | 2 mỗi phần: viết lời, kiểm nghĩa | Viết lại (+2 mỗi phần). Số thật lưu ở `reports.model_calls` |
| Lập hồ sơ ngữ nghĩa | 3 mỗi bảng, chạy song song | Bảng không có cột nào có danh sách giá trị thì 2 |

Lượt kiểm nghĩa chỉ chạy khi lời đã qua kiểm số. Các lượt của một câu hỏi phụ thuộc nhau nên chạy nối tiếp.

## Cách đo câu hỏi

- Máy: Intel Core 7 240H (16 luồng), 30 GiB RAM, Linux. Backend build `--release`, chạy trên máy; Postgres của ứng
  dụng trong Docker.
- Dữ liệu: `company_demo`, chụp `sales.orders` (65 752 dòng) kèm `sales.order_items` (180 519 dòng), và
  `edu.students` (32 593 dòng). Câu hỏi bắt đầu ở `orders`, nối sang `order_items` khi cần doanh thu.
- Câu hỏi số liệu: "Doanh thu theo thị trường năm 2017", "Doanh thu theo tháng năm 2017 của Europe", "Số đơn theo
  trạng thái giao hàng", "Tỷ lệ giao trễ theo phương thức vận chuyển", "Top 5 quốc gia có nhiều đơn nhất năm 2016".
- Câu hỏi nguyên nhân: "Vì sao tỷ lệ giao trễ tháng 11/2016 tăng so với tháng 10/2016?", "Vì sao số đơn tháng 12/2016
  thay đổi so với tháng 11/2016?", "Vì sao doanh thu năm 2017 khác nhau giữa Europe và LATAM?".
- Thời gian, số lượt gọi và token lấy từ `model_calls` của response (`usageMetadata` của Gemini, token suy nghĩ tính
  vào token ra). Tổng thời gian là thời gian request HTTP phía người gọi. Chỉ tính câu `answered`.
- Chi phí = token vào x 0,30 USD + token ra x 2,50 USD mỗi triệu token, giá trả phí của Gemini 3.5 Flash-Lite trên
  https://ai.google.dev/gemini-api/docs/pricing, xem ngày 31/10/2026. Gói miễn phí không tính tiền nhưng giới hạn số
  lượt gọi.

## Câu hỏi

Commit `59948bf`, build release, mỗi câu một lần, trung vị thời gian, trung bình lượt gọi và token:

| Loại câu | Số câu | Tổng thời gian | Chờ mô hình | Lượt gọi | Token vào | Token ra | Chi phí mỗi câu |
|---|---|---|---|---|---|---|---|
| Câu hỏi số liệu | 4 | 3,87 s | 3,81 s | 3,25 | 7 272 | 519 | 0,0035 USD |
| Câu hỏi nguyên nhân | 3 | 5,66 s | 5,56 s | 4,0 | 7 958 | 1 011 | 0,0049 USD |

Theo loại việc, commit `04289e0`, build release, 16 câu (mỗi câu hai lần):

| Lượt gọi | Số lượt | Thời gian | Token vào | Token ra |
|---|---|---|---|---|
| `classify` (phân loại) | 16 | 1,22 s | 1 600 | 70 |
| `sql` (viết SQL) | 10 | 2,09 s | 4 645 | 257 |
| `insight` (lời câu hỏi số liệu) | 10 | 1,08 s | 523 | 109 |
| `why` (điền form) | 6 | 1,69 s | 2 549 | 228 |
| `narrate` (lời câu hỏi nguyên nhân) | 4 | 0,77 s | 1 452 | 33 |

Lượt kiểm nghĩa của câu hỏi số liệu đo tối 31/10/2026 trên code chưa commit (nhánh sau commit có lượt kiểm nghĩa, không
tái lập được đúng trạng thái đó), build debug, cùng 5 câu số liệu, mỗi câu hai lần, bảng chụp lại nên hồ sơ cột khác
lượt trên. Tổng thời gian của build debug không so được với bảng trên; thời gian từng lượt gọi thì so được.

| Lượt gọi | Số lượt | Thời gian | Token vào | Token ra |
|---|---|---|---|---|
| `classify` | 10 | 1,41 s | 2 068 | 75 |
| `sql` | 10 | 2,44 s | 3 733 | 273 |
| `insight` | 11 | 1,06 s | 645 | 120 |
| `judge` (kiểm nghĩa) | 10 | 1,04 s | 943 | 18 |

Cả 10 câu `answered`, cả 10 lời được chấm `match`; một câu viết lời hai lần vì lần đầu trượt kiểm số, nên trung bình
4,1 lượt gọi mỗi câu. Lượt kiểm nghĩa thêm trung bình 0,00033 USD mỗi câu.

## Lập hồ sơ cột

Commit `be76ef8`, build release, `SEMANTIC_JOBS=2`, ba vòng, mỗi vòng chụp lại từ database trống. Trung vị:

| Bảng | Thời gian lập hồ sơ | Lượt gọi | Token vào | Token ra | Chi phí |
|---|---|---|---|---|---|
| `orders` | 6,3 s | 3 | 4 810 | 3 521 | 0,0102 USD |
| `order_items` | 2,6 s | 2 | 1 700 | 876 | 0,0027 USD |
| `students` | 5,5 s | 3 | 3 667 | 3 216 | 0,0091 USD |

Nhóm `orders`, `order_items` có hồ sơ ngữ nghĩa sau 8,5 giây tính từ lúc gửi request chụp bảng (chụp bằng `copy`).
Bản chạy một prompt (commit `04289e0`) cần 14,1 giây cho cùng nhóm. Phần chậm nhất luôn là bí danh giá trị (4,6 đến
6,3 giây). `order_items` không có cột nào có danh sách giá trị nên không gọi phần đó.

Chạy song song: mỗi bảng ba prompt cùng lúc (cột; bí danh giá trị; câu gợi ý), một phần lỗi thì giữ các phần còn lại
(`backend/src/service/semantic/parts.rs`); tối đa `SEMANTIC_JOBS` bảng cùng lúc, mặc định 2, tức tối đa 6 request tới
mô hình. Các câu SQL từng chiều của phân rã cũng chạy cùng lúc, trong giới hạn 2 câu DuckDB cùng lúc.

## Chụp bảng: copy và direct

Đo ngày 31/10/2026, commit `fba6129` cộng thay đổi đặt `ISOLATION_LEVEL` khi ATTACH, build release, extension
`postgres` của DuckDB `v1.5.6`. Trung vị của 3 lần mỗi bảng và mỗi cách; đĩa tạm và bộ nhớ đỉnh lấy lần trung vị.

| Bảng | Số dòng | Cách | Trung vị | Đĩa tạm đỉnh | Bộ nhớ đỉnh |
|---|---|---|---|---|---|
| `sales.orders` | 65 752 | copy | 0,90 s | 12,5 MB | 106 MiB |
| `sales.orders` | 65 752 | direct | 0,37 s | 4,1 MB | 110 MiB |
| `sales.order_items` | 180 519 | copy | 1,73 s | 19,2 MB | 132 MiB |
| `sales.order_items` | 180 519 | direct | 0,45 s | 6,1 MB | 125 MiB |
| `edu.student_daily_clicks` | 1 808 119 | copy | 15,92 s | 51,4 MB | 507 MiB |
| `edu.student_daily_clicks` | 1 808 119 | direct | 0,89 s | 11,7 MB | 326 MiB |

Hai cách cho cùng số dòng, cùng tên và kiểu cột trong cả 18 lần đo; không lần nào `direct` phải quay về `copy`. Test
`both_modes_give_the_same_types_and_rows` (`backend/src/service/sources/direct_tests.rs`) so từng dòng của hai cách trên
một bảng đủ loại kiểu Postgres.

Cách đo: mỗi lần tạo lại database ứng dụng và `DATA_DIR` trống, khởi động backend mới (`LLM_PROVIDER=mock`,
`SEED_SAMPLE=false`), thêm nguồn với `sslmode` `disable`, gửi một request chụp một bảng; hai cách chạy xen kẽ. Thời
gian là thời gian request `POST .../snapshot` phía người gọi (chép, đổi sang Parquet, hồ sơ cột lớp cấu trúc, ghi phiên
bản). Đĩa tạm là tổng dung lượng `DATA_DIR/staging`, đọc mỗi 20 ms. Bộ nhớ là `VmHWM` của backend sau request. Máy như
mục Cách đo câu hỏi, có một bản build khác chạy nền (load average 2 đến 5). Script của lượt này không nằm trong repo;
các bước chỉ dùng HTTP API và `/proc`.

## Chụp bảng lớn

Đo ngày 01/11/2026 để đặt trần chụp bảng theo dung lượng (`SOURCE_TABLE_MAX_MB`, `SOURCE_SNAPSHOT_MAX_MB`,
`SOURCE_TIMEOUT_SECS`). Code của nhánh merge thành commit `1529e4c` (không trần dòng, không `count(*)`), build release,
cách `direct`, không gọi mô hình. Trung vị của 3 lần mỗi bảng.

| Bảng | Số dòng | Số cột | Postgres | Thời gian chụp | Giao dịch trên nguồn | Parquet | Bộ nhớ đỉnh | Câu tổng hợp |
|---|---|---|---|---|---|---|---|---|
| `fact_1m` | 1 000 000 | 6 | 63 MB | 0,74 s | 0,2 s | 12 MB | 234 MiB | 0,01 s |
| `fact_5m` | 5 000 000 | 6 | 313 MB | 2,13 s | 1,6 s | 62 MB | 577 MiB | 0,04 s |
| `fact_10m` | 10 000 000 | 6 | 626 MB | 4,05 s | 3,3 s | 125 MB | 791 MiB | 0,07 s |
| `fact_20m` | 20 000 000 | 6 | 1 251 MB | 8,48 s | 6,5 s | 250 MB | 1 075 MiB | 0,14 s |
| `wide_1m` | 1 000 000 | 60 | 372 MB | 5,72 s | 3,8 s | 173 MB | 1 191 MiB | 0,04 s |
| `wide_5m` | 5 000 000 | 60 | 1 860 MB | 24,53 s | 19,3 s | 867 MB | 3 684 MiB | 0,06 s |

Chụp cả sáu bảng một lần với trần mặc định (`scripts/bench-snapshot.sh defaults`, 3 lần): 5 bảng chép (2 625 MB theo
Postgres) trong 18,9 s (26,6 s lần đầu), giao dịch 15,0 s, bộ nhớ đỉnh 1 272 MiB; `wide_5m` chỉ lưu metadata vì cộng
thêm nó thì quá 4096 MB.

Đọc bảng:

- Cùng một triệu dòng, bảng 60 cột tốn gấp khoảng 8 lần thời gian và 14 lần Parquet so với bảng 6 cột: dung lượng
  Postgres cho biết lần chụp nặng thế nào gần hơn số dòng. Khi bảng đã nằm trong cache, mỗi giây chép được khoảng 65
  đến 155 MB theo Postgres; lượt đầu (cache lạnh) chậm hơn tới khoảng 3 lần (`wide_1m` 17,6 s so với 5,5 s).
- Bộ nhớ đỉnh không nằm ở bước chép: trong một lần chụp `wide_5m`, suốt giao dịch tiến trình giữ khoảng 200 MiB
  (`VmRSS` đọc mỗi 0,5 giây), rồi lên 3,6 GiB trong khoảng một giây ở bước lập hồ sơ cột trên file Parquet.
- Câu tổng hợp (tổng theo vùng và tháng) trên Parquet chạy dưới 0,2 s ở mọi cỡ đã đo.
- Code còn `count(*)` (trước commit trên, `SOURCE_MAX_ROWS=4000000000`) cho số gần như nhau, ví dụ `fact_20m` 8,12 s,
  `wide_5m` 23,58 s.

Mặc định chọn từ các số này: `SOURCE_TABLE_MAX_MB=2048` (bảng lớn nhất đã đo vừa trần), `SOURCE_SNAPSHOT_MAX_MB=4096`
(khoảng gấp rưỡi lần chụp năm bảng trên), `SOURCE_TIMEOUT_SECS=300` để còn chỗ khi cache lạnh. Đây là một máy, bảng sinh
tự động, nguồn và backend cùng máy; database nguồn ở xa thì thời gian chép tăng theo đường truyền.

Cách đo:

- Máy như mục Cách đo câu hỏi, SSD NVMe, Linux 7.0; Postgres 17.11 trong Docker (`shared_buffers` 128 MB); extension
  `postgres` của DuckDB `v1.5.6`; máy còn build và test của nhánh khác chạy nền (load average 5 đến 8).
- Bảng sinh bằng `generate_series` (`scripts/bench-snapshot.sh setup`): bảng hẹp gồm khóa, hai mã số, ngày, số tiền
  `numeric(12,2)`, vùng; bảng rộng gồm 20 cột số nguyên, 20 cột `numeric(12,2)`, 10 cột chữ, 5 cột ngày, 4 cột cờ.
  Không có chỉ mục.
- Mỗi lần (`scripts/bench-snapshot.sh run`): database ứng dụng và `DATA_DIR` mới, backend mới (`LLM_PROVIDER=mock`,
  trần đặt rất cao), một request chụp một bảng. Giao dịch là `now() - xact_start` lớn nhất trong `pg_stat_activity`,
  đọc mỗi 0,2 giây. Bộ nhớ đỉnh là `VmHWM` sau request. Câu tổng hợp đo bằng DuckDB 1.5.6 (Python), lần chạy thứ hai.
- Làm lại: `BENCH_URL=postgres://...` trỏ vào một database trống, chạy `setup`, rồi `run` và `defaults`; biến và cột in
  ra ghi ở đầu script.

## Đo lại

- Mỗi response của câu hỏi có `model_calls` (loại việc, thời gian, token); tab **Các bước** của khung Minh chứng hiện
  danh sách này.
- Log của backend có một dòng `llm call` mỗi lượt gọi, một dòng `question done` mỗi câu hỏi và một dòng
  `semantic profile done` mỗi lượt lập hồ sơ.
- `xdi-backend llm-stats [--since <thời điểm RFC 3339>] [--price-in <USD> --price-out <USD>]` đọc lịch sử câu hỏi và in
  thời gian, lượt gọi, token theo loại câu và theo loại việc; có giá thì in chi phí trung bình mỗi câu.
