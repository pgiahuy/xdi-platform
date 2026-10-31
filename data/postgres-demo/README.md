# Postgres mẫu: cơ sở dữ liệu công ty nhiều bảng

Một Postgres 17 riêng (container `xdi-demo-db`, cổng 5440 trên localhost) chứa hai cơ sở dữ liệu, mỗi cơ sở dữ liệu
một mảng nghiệp vụ: `sales_demo` (bán hàng, DataCo) và `edu_demo` (học tập, OULAD). Hai bộ dữ liệu mà repo đã dùng được
tách thành nhiều bảng có khóa chính và khóa ngoại thật, để thử việc kết nối cơ sở dữ liệu của một công ty thay vì nạp
file. Container này tách khỏi Postgres của ứng dụng (`docker-compose.yml` ở gốc repo).

Trạng thái: đã chạy và kiểm trên máy dev (số liệu ở mục "Đã kiểm"). Chỉ dùng trên máy: mật khẩu ghi ngay trong file
này và cổng chỉ mở trên `127.0.0.1`. Mặc định `load.sh` cài sẵn hai nguyên nhân biết trước: giao trễ ở
`sales.orders` của `sales_demo` (tháng 11/2016) và bỏ học ở `edu.students` của `edu_demo` (kỳ 2014J), xem mục
"Nguyên nhân cài sẵn";
`SKIP_DEMO_CAUSE=1` để giữ dữ liệu đúng như CSV.

| Database, schema | Nguồn | Bảng |
|---|---|---|
| `sales_demo`, schema `sales` | DataCo Smart Supply Chain (`DataCoSupplyChainDataset.csv`, một dòng mỗi dòng hàng) | `departments`, `categories`, `products`, `customers`, `orders`, `order_items` |
| `edu_demo`, schema `edu` | OULAD (7 file CSV trong `oulad/`) | `courses`, `students`, `registrations`, `assessments`, `student_assessments`, `vle_materials`, `student_daily_clicks` |

Mỗi database giữ tên schema cũ (`sales`, `edu`), không chuyển về `public`: dùng lại nguyên các file SQL, tên bảng mà
ứng dụng chụp về (`orders`, `students`) không đổi, nên bộ đánh giá và câu mẫu của `mock` chạy như trước. Mỗi bảng và
mỗi cột có mô tả tiếng Việt (`COMMENT ON`), xem bằng `\d+ sales.orders` trong psql (database `sales_demo`), đọc
`sql/sales_schema.sql`, `sql/edu_schema.sql`, hoặc đọc tài liệu bảng sinh sẵn:
[docs/sales_demo](docs/sales_demo/README.md), [docs/edu_demo](docs/edu_demo/README.md).

## Chạy

Cần Docker kèm Docker Compose v2. Không cần cài psql: `load.sh` chạy psql trong container.

```bash
sh data/download.sh dataco
sh data/download.sh oulad
docker compose -f data/postgres-demo/docker-compose.yml up -d
sh data/postgres-demo/load.sh
```

`load.sh` tạo `sales_demo` và `edu_demo` nếu chưa có, rồi với từng database: chép CSV vào schema `staging` của
database đó (mọi cột kiểu text), tạo lại schema dữ liệu (`sales` hay `edu`), đổ dữ liệu bằng `INSERT ... SELECT`, chạy
bước cài nguyên nhân của mảng đó (`sql/plant_demo_cause_sales.sql`, `sql/plant_demo_cause_edu.sql`), cấp quyền cho
`analyst`, rồi xóa `staging`. Mỗi database nạp trong một giao dịch: lỗi giữa chừng thì dữ liệu cũ của database đó còn
nguyên. Chạy lại bao nhiêu lần cũng ra cùng kết quả. Nạp riêng một database: `sh data/postgres-demo/load.sh sales` (hay
`edu`). Database khác trong container (ví dụ `company_demo` của bản cũ) không bị đụng tới. Đo trên máy dev 16 nhân
ngày 31/10/2026, `time sh load.sh` cho cả hai database: 62,6 giây khi container đang chạy, 146,7 giây ở lần đầu sau khi
đổi `docker-compose.yml` (compose tạo lại container). Sau khi nạp, `sales_demo` chiếm 47 MB và `edu_demo` 204 MB
(`pg_database_size`); `staging` thêm khoảng 730 MB nếu giữ.

| Biến | Mặc định | Ý nghĩa |
|---|---|---|
| `DEMO_DB_PORT` | `5440` | Cổng trên localhost |
| `DEMO_ADMINER_PORT` | `8090` | Cổng của Adminer trên localhost, khi bật profile `adminer` |
| `DEMO_ADMIN_PORT` | `8091` | Cổng của màn xem nhanh (Flask-Admin) trên localhost, khi bật profile `admin` |
| `DEMO_DB_CONTAINER` | `xdi-demo-db` | Tên container; chỉ đổi khi cần bản thứ hai bên cạnh bản đang chạy (kèm `COMPOSE_PROJECT_NAME` và `DEMO_DB_PORT` khác) |
| `DEMO_DB_PASSWORD` | `demo` | Mật khẩu của người dùng chủ `demo`, chỉ đặt được trước lần chạy đầu |
| `DEMO_DATA_DIR` | `../downloads` | Thư mục chứa CSV, tính từ thư mục này, gắn chỉ đọc vào container ở `/downloads` |
| `KEEP_STAGING` | không đặt | `KEEP_STAGING=1 sh load.sh` giữ schema `staging` để chạy `sql/verify_sales.sql`, `sql/verify_edu.sql` |
| `SKIP_DEMO_CAUSE` | không đặt | `SKIP_DEMO_CAUSE=1 sh load.sh` bỏ cả hai bước cài nguyên nhân, `sales` và `edu` đúng như CSV |

Kết nối:

| Người dùng | Mật khẩu | Quyền |
|---|---|---|
| `analyst` | `analyst` | Chỉ `SELECT` trên `sales` của `sales_demo` và `edu` của `edu_demo`, giao dịch mặc định chỉ đọc, không thấy `staging` |
| `demo` | `demo` | Chủ cơ sở dữ liệu, `load.sh` dùng |

Chuỗi kết nối cho ứng dụng chạy trên máy (`make dev`), mỗi database một nguồn:
`postgres://analyst:analyst@127.0.0.1:5440/sales_demo` và `postgres://analyst:analyst@127.0.0.1:5440/edu_demo`.
Backend chạy bằng `docker-compose.yml` ở gốc repo: compose gốc có cùng dịch vụ `demo-db` (khai `extends` từ file này) trong
profile `demo`, chung mạng với backend. Nạp xong ở đây thì gỡ container của bản chạy riêng, volume giữ nguyên
(`docker compose -f data/postgres-demo/docker-compose.yml rm -sf demo-db`), rồi ở gốc repo chạy
`docker compose --profile demo up -d`; backend dùng `postgres://analyst:analyst@demo-db:5432/sales_demo` và
`postgres://analyst:analyst@demo-db:5432/edu_demo` (`SAMPLE_DATABASE_URL` mặc định của compose, hai chuỗi cách nhau bằng
dấu phẩy). Hai cách dùng chung volume `xdi-demo-db_demo-db-data` và cùng tên
container `xdi-demo-db`, nên không phải nạp lại, và Docker không cho hai bản chạy cùng lúc trên một volume (hai Postgres
trên một thư mục dữ liệu làm hỏng dữ liệu). Nạp lại bằng `load.sh` thì gỡ container ở gốc repo trước
(`docker compose rm -sf demo-db`). Compose gốc in cảnh báo volume có sẵn không do project này tạo: bình thường.

Xem bảng trên trình duyệt khi trình diễn: Adminer (image `adminer:5.4.1`), chỉ chạy khi bật profile `adminer`.

```bash
docker compose -f data/postgres-demo/docker-compose.yml --profile adminer up -d
```

Mở http://127.0.0.1:8090 (đổi cổng bằng `DEMO_ADMINER_PORT`), chọn System là PostgreSQL, Server để sẵn `demo-db`,
Username và Password `analyst`, Database `sales_demo` hay `edu_demo`. Đăng nhập bằng `analyst` thì chỉ đọc được, như ứng dụng.
Tắt: `docker compose -f data/postgres-demo/docker-compose.yml --profile adminer stop adminer`. `load.sh` chỉ bật
`demo-db`, không bật Adminer.

Xóa hẳn: `docker compose -f data/postgres-demo/docker-compose.yml --profile adminer --profile admin down -v`.

## Xóa company_demo của bản cũ

Volume nạp trước khi tách có database `company_demo` (cả hai schema). `load.sh` thêm hai database mới bên cạnh, không
xóa nó. Khi không còn gì dùng `company_demo` (bản cài đã thiết lập với nó, `SAMPLE_DATABASE_URL` tự đặt, nhánh khác
đang chạy `make eval` trên nó), xóa riêng nó, hai database mới và volume giữ nguyên:

```bash
docker compose -f data/postgres-demo/docker-compose.yml exec -T demo-db dropdb -U demo --if-exists --force company_demo
```

Chạy bằng compose gốc (`--profile demo`) thì dùng `docker compose exec -T demo-db dropdb -U demo --if-exists --force
company_demo` ở gốc repo. `--force` ngắt các kết nối còn mở tới `company_demo`.

## Xem nhanh dữ liệu

Màn chỉ đọc bằng Flask-Admin (`admin/`), đóng vai hệ thống nội bộ của công ty, để khi trình diễn đối chiếu số ứng
dụng trả lời với dòng dữ liệu gốc. Trang tổng quan liệt kê mọi bảng của `sales_demo` và `edu_demo` kèm mô tả, số
dòng, khóa chính, khóa ngoại; menu có một nhóm cho mỗi database. Bấm tên bảng để xem dòng, sắp xếp, lọc theo từng
cột (nút "Thêm bộ lọc"), tìm trên cột chữ. Rê chuột lên tên cột để xem mô tả (`COMMENT ON COLUMN`).

```bash
docker compose -f data/postgres-demo/docker-compose.yml --profile admin up -d --build
```

Mở http://127.0.0.1:8091 (đổi cổng bằng `DEMO_ADMIN_PORT`). Chỉ đọc hai lớp: màn kết nối bằng `analyst` (chỉ
`SELECT`), và mọi trang đã tắt thêm, sửa, xóa (mở `/<bảng>/new/` thì bị chuyển về danh sách). Database nào chưa nạp
thì trang tổng quan ghi lỗi của database đó, database còn lại vẫn xem được. Chạy không cần Docker:
`pip install -r admin/requirements.txt` rồi `python admin/app.py`, đọc `DEMO_SERVER_URL` (mặc định
`postgresql+psycopg://analyst:analyst@127.0.0.1:5440`) và `DEMO_DATABASES` (mặc định `sales_demo,edu_demo`). Tắt:
`docker compose -f data/postgres-demo/docker-compose.yml --profile admin stop admin`.

Kịch bản đối chiếu một con số, với câu "Vì sao tỷ lệ giao trễ tháng 11/2016 tăng so với tháng 10/2016?" (đáp án ở mục
"Nguyên nhân cài sẵn"): mở `sales_demo`, bảng `orders`, bấm "Thêm bộ lọc" ba lần: `order_date` nằm giữa 2016-11-01
và 2016-11-30 23:59:59, `market` bằng `Pacific Asia`, `shipping_mode` bằng `Standard Class`. Danh sách ra 323 đơn,
đúng nhóm mà câu trả lời nêu; thêm bộ lọc `delivery_status` bằng `Late delivery` thì còn 309 đơn (95,7%).

## Sơ đồ quan hệ

Sơ đồ ER và tài liệu từng bảng (cột, kiểu, mô tả, khóa) trong [docs/sales_demo](docs/sales_demo/README.md) và
[docs/edu_demo](docs/edu_demo/README.md), sinh bằng [tbls](https://github.com/k1LoW/tbls) (giấy phép MIT) từ database
đang chạy; mô tả lấy từ `COMMENT`.

![Sơ đồ ER của edu_demo](docs/edu_demo/schema.svg)

![Sơ đồ ER của sales_demo](docs/sales_demo/schema.svg)

Sinh lại sau khi đổi schema: chạy `load.sh`, rồi `TBLS=/đường/dẫn/tbls sh data/postgres-demo/docs.sh`. tbls không nằm
trong repo; tải bản cho máy ở https://github.com/k1LoW/tbls/releases (bản đã dùng: v1.96.1, linux amd64). `docs.sh`
kết nối bằng `analyst` theo `.tbls.sales_demo.yml` và `.tbls.edu_demo.yml`, mỗi database chỉ lấy schema dữ liệu của
nó, và xóa thư mục đích trước khi sinh.

Bản chữ dưới đây: mũi tên đi từ bảng "nhiều" sang bảng "một", theo khóa ngoại. PK là khóa chính.

```
sales_demo, schema sales
  order_items (PK order_item_id)            bảng sự kiện, 1 dòng = 1 dòng hàng
    order_id    -> orders (PK order_id)
                     customer_id -> customers (PK customer_id)
    product_id  -> products (PK product_id)
                     category_id -> categories (PK category_id)
                                      department_id -> departments (PK department_id)

edu_demo, schema edu
  student_assessments (PK id_assessment, id_student)
    id_assessment                              -> assessments (PK id_assessment)
                                                    (code_module, code_presentation) -> courses
    (code_module, code_presentation, id_student) -> students
  student_daily_clicks (PK code_module, code_presentation, id_student, date)
    (code_module, code_presentation, id_student) -> students
  registrations (PK code_module, code_presentation, id_student)
    (code_module, code_presentation, id_student) -> students     (đúng một dòng mỗi sinh viên)
  students (PK code_module, code_presentation, id_student)
    (code_module, code_presentation)           -> courses (PK code_module, code_presentation)
  vle_materials (PK id_site)
    (code_module, code_presentation)           -> courses
```

Nối `sales.order_items` theo mọi mũi tên tới hết các bảng chiều vẫn ra đúng 180 519 dòng, không nhân dòng (mục "Đã
kiểm"). Tương tự với `edu.student_assessments` (173 912) và `edu.student_daily_clicks` (1 808 119).

## Số dòng

| Bảng | Số dòng | Từ đâu |
|---|---|---|
| `sales.departments` | 11 | Department Id khác nhau |
| `sales.categories` | 51 | Category Id khác nhau |
| `sales.products` | 118 | Product Card Id khác nhau |
| `sales.customers` | 20 652 | Customer Id khác nhau |
| `sales.orders` | 65 752 | Order Id khác nhau |
| `sales.order_items` | 180 519 | Mỗi dòng CSV |
| `edu.courses` | 22 | `courses.csv` |
| `edu.students` | 32 593 | `studentInfo.csv` |
| `edu.registrations` | 32 593 | `studentRegistration.csv` |
| `edu.assessments` | 206 | `assessments.csv` |
| `edu.student_assessments` | 173 912 | `studentAssessment.csv` |
| `edu.vle_materials` | 6 364 | `vle.csv` |
| `edu.student_daily_clicks` | 1 808 119 | `studentVle.csv` (10 655 280 dòng) gộp theo sinh viên và ngày |

## Tách bảng DataCo

File DataCo phẳng: thông tin khách, đơn, sản phẩm lặp lại trên mỗi dòng hàng. Trước khi chọn khóa, đã nạp file vào
`staging.dataco` và kiểm, với mỗi khóa ứng viên và mỗi cột còn lại, có khóa nào mang hơn một giá trị không:

- `Customer Id` xác định mọi cột khách (tên, họ, phân khúc, địa chỉ, Latitude, Longitude): 0 vi phạm. `Order Customer
  Id` bằng `Customer Id` ở mọi dòng.
- `Order Id` xác định khách, hai ngày, trạng thái đơn, tình trạng giao, số ngày giao, cách giao, hình thức thanh toán
  (`Type`), Market, vùng, nước, bang, thành phố, mã bưu chính nơi giao: 0 vi phạm. Các cột này vào `orders`.
- Theo `Order Id`, số lượng, sản phẩm, Sales, giảm giá, `Order Item Total`, `Order Profit Per Order` đổi theo dòng
  (ví dụ `Order Profit Per Order` khác nhau trong 45 902 đơn, 160 669 dòng): chúng vào `order_items`. Tên
  `Order Profit Per Order` và `Benefit per order` nói "per order" nhưng là lợi nhuận của từng dòng hàng.
- `Product Card Id` xác định tên, giá, danh mục; `Category Id` xác định tên danh mục và phòng ban; `Department Id` xác
  định tên phòng ban: 0 vi phạm.
- Cột trùng nhau ở mọi dòng, giữ một: `Sales per customer` = `Order Item Total`, `Benefit per order` = `Order Profit
  Per Order`, `Order Item Cardprod Id` = `Product Card Id`, `Product Category Id` = `Category Id`, `Order Item Product
  Price` = `Product Price` (giữ cả hai, thành `order_items.unit_price` và `products.price`).
- Bỏ: `Customer Email`, `Customer Password` (một giá trị che `XXXXXXXXX`), `Product Status` (luôn 0), `Product
  Description` (trống), `Product Image` (đường dẫn ảnh).

Sửa khi nạp: bỏ khoảng trắng thừa ở cuối tên (3 206 dòng, ví dụ `Smart watch `); 3 khách bị lệch cột trong nguồn (mã
bưu chính nằm ở cột bang) được dời về đúng cột, địa chỉ đường để trống. Số tiền giữ đúng giá trị trong CSV (kiểu
`numeric`), nên tổng trên Postgres khớp tổng tính trên file. `Order Item Total` không phải lúc nào cũng bằng đúng
`Sales - Order Item Discount` (1 224 dòng lệch hơn 0,01, do số lẻ trong nguồn); giữ nguyên giá trị nguồn.

## OULAD

- Giá trị `?` và chuỗi rỗng thành NULL: `imd_band` (1 111 dòng), ngày đăng ký (45), ngày hủy đăng ký (22 521, sinh
  viên không hủy), ngày của bài đánh giá (11), điểm (173), tuần của tài liệu (5 243).
- `imd_band` trong nguồn có nhóm `10-20` thiếu dấu `%`, đổi thành `10-20%` cho cùng dạng với 9 nhóm còn lại.
- `student_assessments` chép thêm môn và kỳ từ bài đánh giá để có khóa ngoại tới `students`; `sql/verify.sql` kiểm
  chúng khớp bài đánh giá.
- `studentVle.csv` (10,6 triệu dòng, 454 MB) không nạp nguyên. Gộp theo (sinh viên, tài liệu, ngày) vẫn còn 8,5 triệu
  dòng, quá nặng cho một bảng mà ứng dụng chụp về. Gộp theo (sinh viên, ngày) còn 1,8 triệu dòng
  (`student_daily_clicks`: tổng lượt bấm, số tài liệu đã mở), đủ để hỏi mức hoạt động theo thời gian và so với kết quả
  cuối khóa; mất chi tiết từng tài liệu. `vle_materials` vẫn nạp đủ để hỏi về tài liệu của từng học phần.

## Nguyên nhân cài sẵn

Để thử câu hỏi tìm nguyên nhân với đáp án biết trước, `load.sh` chạy hai file sau khi đổ dữ liệu thật:
`sql/plant_demo_cause_sales.sql` và `sql/plant_demo_cause_edu.sql`. Đây là dữ liệu sửa tay, không phải dữ liệu gốc.
Không ghi gì về hai việc sửa này vào COMMENT của bảng hay cột, để mô hình ngôn ngữ không đọc được đáp án; vì vậy sơ
đồ và tài liệu bảng trong `docs/` (sinh từ COMMENT) cũng không nhắc tới.

| Database | Câu hỏi mong đợi | Đáp án |
|---|---|---|
| `sales_demo` | "Vì sao tỷ lệ giao trễ tháng 11/2016 tăng so với tháng 10/2016?" | Pacific Asia và Standard Class |
| `edu_demo` | "Vì sao tỷ lệ bỏ học kỳ 2014J cao hơn kỳ 2014B?" | Scotland và nhóm tuổi 0-35 |

### Giao trễ ở `sales`

- Đơn bị đổi: tháng 11/2016, thị trường Pacific Asia, cách giao Standard Class, chưa hủy và đang giao sớm hoặc đúng hạn:
  187 đơn (123 giao sớm, 64 đúng hạn), 547 dòng `order_items`.
- Cột đổi, chỉ trong `sales.orders`: `days_for_shipping_real` cộng 3 (lịch 4 ngày, nên thành 5 đến 7 ngày),
  `shipping_date` cộng 3 ngày, `delivery_status` thành `Late delivery`, `late_delivery_risk` thành true. Tiền, số
  lượng và mọi bảng khác giữ nguyên.
- Chọn tháng 11/2016 vì từ 8/2016 đến 1/2017 mỗi tháng có từ bốn thị trường trở lên; các tháng khác DataCo chỉ có một
  hoặc hai thị trường. Tháng 10/2016 có cùng các thị trường nên dùng làm mốc so.

| Tỷ lệ giao trễ | Trước | Sau |
|---|---|---|
| Tháng 11/2016, theo đơn (1 721 đơn) | 54,3% | 65,2% |
| Tháng 11/2016, theo dòng hàng (5 210 dòng) | 54,6% (2 846 dòng) | 65,1% (3 393 dòng) |
| Tháng 11/2016, Pacific Asia và Standard Class, theo đơn (323 đơn) | 37,8% | 95,7% |
| Tháng 11/2016, Pacific Asia và Standard Class, theo dòng hàng (986 dòng) | 39,6% | 95,0% |

Mốc so: tháng 10/2016 có tỷ lệ 54,8% theo dòng hàng, Pacific Asia và Standard Class 40,1%. Trước khi sửa, tỷ lệ theo
đơn của mọi tháng nằm trong khoảng 52,4% đến 57,3%. Câu hỏi mong đợi: "Vì sao tỷ lệ giao trễ tháng 11/2016 tăng so
với tháng 10/2016?" hay "Vì sao số dòng hàng giao trễ tháng 11/2016 tăng?", đáp án Pacific Asia và Standard Class.
Phần đơn còn lại trong nhóm này không trễ là đơn đã hủy (`Shipping canceled`), bước cài không đụng tới.

### Bỏ học ở `edu`

- Sinh viên bị đổi: kỳ 2014J, vùng Scotland, nhóm tuổi 0-35, kết quả Pass: 381 sinh viên, rải trên cả 7 môn (AAA 12,
  BBB 66, CCC 69, DDD 58, EEE 91, FFF 68, GGG 17), nên chia theo môn không ra đáp án.
- Cột đổi: chỉ `edu.students.final_result`, từ `Pass` thành `Withdrawn`. Mọi bảng khác giữ nguyên.
- Chọn nhóm này vì là nhóm (vùng, nhóm tuổi) đông nhất của kỳ 2014J (925 sinh viên) và trước khi sửa có tỷ lệ bỏ học
  thấp hơn trung bình kỳ. Kỳ 2014B dùng làm mốc so: trước khi sửa, 2014J và 2014B gần bằng nhau.
- Dấu vết còn lại: 381 sinh viên này vẫn có điểm thi, lượt bấm tới cuối kỳ, và `registrations.date_unregistration`
  rỗng. Trong dữ liệu thật, 10 063 trên 10 156 sinh viên `Withdrawn` có ngày hủy đăng ký. Ai soi kỹ sẽ thấy nhóm này
  lạ, và cũng ra đúng đáp án.

| Tỷ lệ bỏ học (`final_result = 'Withdrawn'`) | Trước | Sau |
|---|---|---|
| Kỳ 2014J (11 260 sinh viên) | 34,0% (3 826) | 37,4% (4 207) |
| Kỳ 2014J, Scotland và 0-35 (925 sinh viên) | 26,3% (243) | 67,5% (624) |
| Kỳ 2014J, ngoài nhóm trên (10 335 sinh viên) | 34,7% (3 583) | 34,7% (3 583) |
| Kỳ 2014J, Scotland mọi nhóm tuổi (1 381 sinh viên) | 25,3% (350) | 52,9% (731) |
| Kỳ 2014J, 0-35 mọi vùng (7 885 sinh viên) | 35,6% (2 806) | 40,4% (3 187) |

Mốc so: kỳ 2014B có tỷ lệ 33,5% (2 613 trên 7 804), nhóm Scotland và 0-35 của 2014B 32,3% (536 sinh viên). Các kỳ
khác: 2013B 28,8%, 2013J 26,8%. Sau khi sửa, Scotland thành vùng có tỷ lệ bỏ học cao nhất kỳ 2014J (52,9%, vùng kế tiếp
West Midlands Region 40,4%). Tỷ lệ đạt (`Pass`) của 2014J giảm từ 38,1% (4 293) xuống 34,7% (3 912).

## Đã kiểm

`KEEP_STAGING=1 sh load.sh`, rồi với từng database
`docker compose -f data/postgres-demo/docker-compose.yml exec -T demo-db psql -U demo -d sales_demo -f - < data/postgres-demo/sql/verify_sales.sql`
(`edu_demo` với `sql/verify_edu.sql`). Lần chạy gần nhất (31/10/2026, có hai bước cài nguyên nhân) cả 42 dòng `ok` là
`t` (25 của `sales_demo`, 17 của `edu_demo`). Lúc tách, nội dung 13 bảng của hai database mới cũng được so với
`company_demo` (md5 của mọi dòng mỗi bảng): giống hệt, kể cả hai nguyên nhân cài sẵn.

- Số dòng mỗi bảng bằng số dòng CSV hoặc số khóa khác nhau trong CSV (bảng "Số dòng"); tổng `sum_click` bằng tổng
  trên `studentVle.csv` (39 605 099).
- Có đủ 12 khóa ngoại (`edu_demo` 7, `sales_demo` 5).
- Nối bảng sự kiện với mọi bảng chiều không đổi số dòng.
- Tổng `item_total` và `profit` theo Market, tổng `sales` theo phòng ban, điểm trung bình theo kết quả cuối khóa: bằng
  đúng số tính trên `staging`. Riêng tổng `Order Item Total` theo Market còn tính lại thẳng từ CSV bằng Python
  (`csv`, `Decimal`): Africa 2061679.392850406, Europe 9769198.321669751, LATAM 9235762.091841220, Pacific Asia
  7434262.652838773, USCA 4553499.921016465, khớp Postgres.
- So từng đơn với CSV: đúng 187 đơn khác, là mọi đơn của nhóm đã định ở mục "Nguyên nhân cài sẵn", mỗi đơn đổi đúng
  như mô tả (số ngày giao cộng 3, ngày giao cộng 3 ngày, `Late delivery`, `late_delivery_risk` true).
- So từng sinh viên với `studentInfo.csv`: đúng 381 dòng khác, là mọi sinh viên Pass của kỳ 2014J, Scotland, 0-35, chỉ
  `final_result` đổi thành `Withdrawn`, các cột khác giữ nguyên. Điểm trung bình theo kết quả cuối khóa (câu 7) so với
  CSV sau khi bỏ nhóm này ở cả hai phía, nên qua cả khi có và khi không có bước cài.

## Câu hỏi mẫu và bảng cần dùng

Bảng ghi bảng nào phải đọc để trả lời đúng. Hai câu "vì sao" cuối có đáp án biết trước (mục "Nguyên nhân cài sẵn");
các câu khác chưa có đáp án ghi sẵn ở đây.

| Câu hỏi | Bảng |
|---|---|
| Lợi nhuận theo phòng ban và thị trường năm 2017 | `sales.order_items`, `orders`, `products`, `categories`, `departments` |
| Tỷ lệ đơn giao trễ theo cách giao và phân khúc khách | `sales.orders`, `customers` |
| Mười sản phẩm có doanh số cao nhất ở Europe, kèm danh mục | `sales.order_items`, `orders`, `products`, `categories` |
| Khách ở bang nào mua nhiều nhất từ thị trường LATAM | `sales.order_items`, `orders`, `customers` |
| Điểm trung bình bài TMA theo nhóm `imd_band` và kết quả cuối khóa | `edu.student_assessments`, `assessments`, `students` |
| Sinh viên bỏ học có số lượt bấm trong 4 tuần đầu thấp hơn sinh viên đạt bao nhiêu | `edu.student_daily_clicks`, `students` |
| Tỷ lệ bỏ học theo môn và kỳ, kèm độ dài học phần | `edu.students`, `courses` |
| Vì sao tỷ lệ giao trễ tháng 11/2016 tăng so với tháng 10/2016? | `sales.orders` (theo đơn); thêm `order_items` nếu đếm theo dòng hàng |
| Vì sao tỷ lệ bỏ học kỳ 2014J cao hơn kỳ 2014B? | `edu.students` |

Đáp án của hai câu cuối:

- Giao trễ: Pacific Asia và Standard Class. Tỷ lệ giao trễ của nhóm này tháng 11/2016 từ 37,8% lên 95,7% theo đơn
  (39,6% lên 95,0% theo dòng hàng); cả tháng từ 54,3% lên 65,2% theo đơn, tháng 10/2016 là 54,8% theo dòng hàng.
- Bỏ học: Scotland và nhóm tuổi 0-35. Tỷ lệ bỏ học của nhóm này kỳ 2014J từ 26,3% lên 67,5%; cả kỳ từ 34,0% lên
  37,4%, kỳ 2014B là 33,5%; phần còn lại của kỳ 2014J giữ 34,7%.

## Giấy phép và ghi công

Cả hai bộ dữ liệu theo giấy phép CC BY 4.0; ghi công khi dùng lại. Thư mục này chỉ chứa script, không chứa dữ liệu;
CSV tải bằng `data/download.sh`, nguồn đầy đủ ở [data/README.md](../README.md).

| Dataset | Nguồn | Giấy phép, ghi công |
|---|---|---|
| DataCo Smart Supply Chain for Big Data Analysis | Mendeley Data, https://doi.org/10.17632/8gx2fvg2k6.5 | CC BY 4.0; Fabian Constante, Fernando Silva, António Pereira (2019) |
| Open University Learning Analytics (OULAD) | UCI Machine Learning Repository, https://archive.ics.uci.edu/dataset/349/open+university+learning+analytics+dataset | CC BY 4.0; Jakub Kuzilek, Martin Hlosta, Zdenek Zdrahal (2015), https://doi.org/10.24432/C5KK69 |

Bảng ở đây là bản đã biến đổi từ dữ liệu gốc: tách bảng, đổi tên cột, gộp `studentVle`, các sửa nêu ở trên.

## File

| File | Vai trò |
|---|---|
| `docker-compose.yml` | Container `xdi-demo-db`; Adminer `xdi-demo-adminer` trong profile `adminer`; màn xem nhanh `xdi-demo-admin` trong profile `admin` |
| `init/01_roles.sql` | Tạo người dùng `analyst`, chạy một lần khi volume trống |
| `load.sh` | Tạo `sales_demo`, `edu_demo` nếu chưa có, nạp từ CSV, chạy lại được |
| `sql/staging_sales.sql`, `sql/staging_edu.sql` | Bảng staging của từng database, mọi cột text |
| `sql/edu_schema.sql`, `sql/edu_load.sql` | Schema `edu` và cách đổ dữ liệu |
| `sql/sales_schema.sql`, `sql/sales_load.sql` | Schema `sales` và cách đổ dữ liệu |
| `sql/plant_demo_cause_sales.sql` | Cài một nguyên nhân biết trước vào `sales.orders`, bỏ qua bằng `SKIP_DEMO_CAUSE=1` |
| `sql/plant_demo_cause_edu.sql` | Cài một nguyên nhân biết trước vào `edu.students`, bỏ qua cùng cờ |
| `sql/grants.sql` | Quyền chỉ đọc cho `analyst` trên một database |
| `sql/row_counts.sql`, `sql/verify_sales.sql`, `sql/verify_edu.sql` | Đếm dòng; kiểm so với CSV |
| `.tbls.sales_demo.yml`, `.tbls.edu_demo.yml`, `docs.sh` | Cấu hình tbls và lệnh sinh lại `docs/` |
| `docs/sales_demo/`, `docs/edu_demo/` | Tài liệu bảng (Markdown) và sơ đồ ER (SVG), sinh bằng tbls, không sửa tay |
| `admin/` | Màn xem nhanh chỉ đọc (Flask-Admin): `app.py`, `templates/`, `requirements.txt` cố định phiên bản, `Dockerfile` |
