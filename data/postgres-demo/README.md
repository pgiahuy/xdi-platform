# Postgres mẫu: cơ sở dữ liệu công ty nhiều bảng

Một Postgres 17 riêng (container `xdi-demo-db`, cổng 5440 trên localhost, cơ sở dữ liệu `company_demo`) chứa hai bộ
dữ liệu mà repo đã dùng, tách thành nhiều bảng có khóa chính và khóa ngoại thật, để thử việc kết nối cơ sở dữ liệu
của một công ty thay vì nạp file. Container này tách khỏi Postgres của ứng dụng (`docker-compose.yml` ở gốc repo).

Trạng thái: đã chạy và kiểm trên máy dev (số liệu ở mục "Đã kiểm"). Chỉ dùng trên máy: mật khẩu ghi ngay trong file
này và cổng chỉ mở trên `127.0.0.1`. Mặc định `load.sh` cài sẵn một nguyên nhân biết trước vào `sales.orders` (tháng
11/2016, xem mục "Nguyên nhân cài sẵn"); `SKIP_DEMO_CAUSE=1` để giữ dữ liệu đúng như CSV.

| Schema | Nguồn | Bảng |
|---|---|---|
| `sales` | DataCo Smart Supply Chain (`DataCoSupplyChainDataset.csv`, một dòng mỗi dòng hàng) | `departments`, `categories`, `products`, `customers`, `orders`, `order_items` |
| `edu` | OULAD (7 file CSV trong `oulad/`) | `courses`, `students`, `registrations`, `assessments`, `student_assessments`, `vle_materials`, `student_daily_clicks` |

Mỗi bảng và mỗi cột có mô tả tiếng Việt (`COMMENT ON`), xem bằng `\d+ sales.orders` trong psql hoặc đọc
`sql/sales_schema.sql`, `sql/edu_schema.sql`.

## Chạy

Cần Docker kèm Docker Compose v2. Không cần cài psql: `load.sh` chạy psql trong container.

```bash
sh data/download.sh dataco
sh data/download.sh oulad
docker compose -f data/postgres-demo/docker-compose.yml up -d
sh data/postgres-demo/load.sh
```

`load.sh` chép CSV vào schema `staging` (mọi cột kiểu text), tạo lại `edu` và `sales`, đổ dữ liệu bằng
`INSERT ... SELECT`, chạy bước cài nguyên nhân (`sql/plant_demo_cause.sql`), cấp quyền cho `analyst`, rồi xóa
`staging`. Mọi bước chạy trong một giao dịch: lỗi giữa chừng thì
dữ liệu cũ còn nguyên. Chạy lại bao nhiêu lần cũng ra cùng kết quả. Trên máy dev 16 nhân, `time sh load.sh` đo được
từ 37 đến 46 giây, có lần tới 1 phút 50 giây; sau khi nạp, hai schema chiếm khoảng 320 MB
(`staging` thêm khoảng 730 MB nếu giữ).

| Biến | Mặc định | Ý nghĩa |
|---|---|---|
| `DEMO_DB_PORT` | `5440` | Cổng trên localhost |
| `DEMO_DB_PASSWORD` | `demo` | Mật khẩu của người dùng chủ `demo`, chỉ đặt được trước lần chạy đầu |
| `DEMO_DATA_DIR` | `../downloads` | Thư mục chứa CSV, tính từ thư mục này, gắn chỉ đọc vào container ở `/downloads` |
| `KEEP_STAGING` | không đặt | `KEEP_STAGING=1 sh load.sh` giữ schema `staging` để chạy `sql/verify.sql` |
| `SKIP_DEMO_CAUSE` | không đặt | `SKIP_DEMO_CAUSE=1 sh load.sh` bỏ bước cài nguyên nhân, `sales` đúng như CSV |

Kết nối:

| Người dùng | Mật khẩu | Quyền |
|---|---|---|
| `analyst` | `analyst` | Chỉ `SELECT` trên `edu` và `sales`, giao dịch mặc định chỉ đọc, không thấy `staging` |
| `demo` | `demo` | Chủ cơ sở dữ liệu, `load.sh` dùng |

Chuỗi kết nối cho ứng dụng chạy trên máy (`make dev`): `postgres://analyst:analyst@127.0.0.1:5440/company_demo`.
Backend chạy trong Docker không tới được cổng chỉ mở trên `127.0.0.1` của máy; nối container backend vào mạng của
container này (`docker network connect xdi-demo-db_default <tên container backend>`) rồi dùng
`postgres://analyst:analyst@xdi-demo-db:5432/company_demo`.

Xóa hẳn: `docker compose -f data/postgres-demo/docker-compose.yml down -v`.

## Sơ đồ quan hệ

Mũi tên đi từ bảng "nhiều" sang bảng "một", theo khóa ngoại. PK là khóa chính.

```
sales
  order_items (PK order_item_id)            bảng sự kiện, 1 dòng = 1 dòng hàng
    order_id    -> orders (PK order_id)
                     customer_id -> customers (PK customer_id)
    product_id  -> products (PK product_id)
                     category_id -> categories (PK category_id)
                                      department_id -> departments (PK department_id)

edu
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

Để thử câu hỏi tìm nguyên nhân với đáp án biết trước, `load.sh` chạy `sql/plant_demo_cause.sql` sau khi đổ dữ liệu
thật. Đây là dữ liệu sửa tay, không phải DataCo gốc.

- Đơn bị đổi: tháng 11/2016, thị trường Pacific Asia, cách giao Standard Class, chưa hủy và đang giao sớm hoặc đúng hạn:
  187 đơn (123 giao sớm, 64 đúng hạn), 547 dòng `order_items`.
- Cột đổi, chỉ trong `sales.orders`: `days_for_shipping_real` cộng 3 (lịch 4 ngày, nên thành 5 đến 7 ngày),
  `shipping_date` cộng 3 ngày, `delivery_status` thành `Late delivery`, `late_delivery_risk` thành true. Tiền, số
  lượng và mọi bảng khác giữ nguyên.
- Chọn tháng 11/2016 vì từ 8/2016 đến 1/2017 mỗi tháng có từ bốn thị trường trở lên; các tháng khác DataCo chỉ có một
  hoặc hai thị trường. Tháng 10/2016 có cùng các thị trường nên dùng làm mốc so.
- Không ghi gì về việc sửa này vào COMMENT của bảng hay cột, để mô hình ngôn ngữ không đọc được đáp án.

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

## Đã kiểm

`KEEP_STAGING=1 sh load.sh`, rồi
`docker compose -f data/postgres-demo/docker-compose.yml exec -T demo-db psql -U demo -d company_demo -f - < data/postgres-demo/sql/verify.sql`.
Lần chạy gần nhất (có bước cài nguyên nhân) mọi dòng `ok` là `t`:

- Số dòng mỗi bảng bằng số dòng CSV hoặc số khóa khác nhau trong CSV (bảng "Số dòng"); tổng `sum_click` bằng tổng
  trên `studentVle.csv` (39 605 099).
- 12 khóa ngoại, đều đã kiểm trên dữ liệu (`convalidated`).
- Nối bảng sự kiện với mọi bảng chiều không đổi số dòng.
- Tổng `item_total` và `profit` theo Market, tổng `sales` theo phòng ban, điểm trung bình theo kết quả cuối khóa: bằng
  đúng số tính trên `staging`. Riêng tổng `Order Item Total` theo Market còn tính lại thẳng từ CSV bằng Python
  (`csv`, `Decimal`): Africa 2061679.392850406, Europe 9769198.321669751, LATAM 9235762.091841220, Pacific Asia
  7434262.652838773, USCA 4553499.921016465, khớp Postgres.
- So từng đơn với CSV: đúng 187 đơn khác, đều thuộc nhóm đã định ở mục "Nguyên nhân cài sẵn" và đổi đúng như mô tả.

## Câu hỏi mẫu cần nhiều bảng

- Lợi nhuận theo phòng ban và thị trường năm 2017 (`order_items`, `products`, `categories`, `departments`, `orders`).
- Tỷ lệ đơn giao trễ theo cách giao và phân khúc khách (`orders`, `customers`).
- Mười sản phẩm có doanh số cao nhất ở Europe, kèm danh mục (`order_items`, `orders`, `products`, `categories`).
- Khách ở bang nào mua nhiều nhất từ thị trường LATAM (`order_items`, `orders`, `customers`).
- Điểm trung bình bài TMA theo nhóm `imd_band` và kết quả cuối khóa (`student_assessments`, `assessments`,
  `students`).
- Sinh viên bỏ học có số lượt bấm trong 4 tuần đầu thấp hơn sinh viên đạt bao nhiêu (`student_daily_clicks`,
  `students`).
- Tỷ lệ bỏ học theo môn và kỳ, kèm độ dài học phần (`students`, `courses`).

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
| `docker-compose.yml` | Container `xdi-demo-db` |
| `init/01_roles.sql` | Tạo người dùng `analyst`, chạy một lần khi volume trống |
| `load.sh` | Nạp từ CSV, chạy lại được |
| `sql/staging.sql` | Bảng staging, mọi cột text |
| `sql/edu_schema.sql`, `sql/edu_load.sql` | Schema `edu` và cách đổ dữ liệu |
| `sql/sales_schema.sql`, `sql/sales_load.sql` | Schema `sales` và cách đổ dữ liệu |
| `sql/plant_demo_cause.sql` | Cài một nguyên nhân biết trước vào `sales.orders`, bỏ qua bằng `SKIP_DEMO_CAUSE=1` |
| `sql/grants.sql` | Quyền chỉ đọc cho `analyst` |
| `sql/row_counts.sql`, `sql/verify.sql` | Đếm dòng; kiểm so với CSV |
