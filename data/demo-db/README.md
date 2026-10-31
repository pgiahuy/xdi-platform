# Database mẫu nhỏ của compose

Dịch vụ `demo-db` trong `docker-compose.yml` ở gốc repo: một Postgres 17 chứa database `company_demo`, đóng vai database
của một tổ chức mà ứng dụng kết nối và chụp bảng. Cùng schema `sales` và `edu` với bản đầy đủ ở
[data/postgres-demo](../postgres-demo/README.md), cùng các file SQL tạo bảng và tách bảng, nhưng nạp từ file nhỏ có sẵn
trong `data/samples/`, nên bản cài mới không phải tải gì thêm.

Trạng thái: đã chạy trên máy dev bằng `docker compose up -d --wait demo-db`; nạp xong trong vài giây, số dòng ở bảng
dưới.

## Chạy

`docker compose up` chạy cả `demo-db`. Lần đầu (volume `demodata` còn trống), image Postgres chạy hai file trong
`docker-entrypoint-initdb.d`:

1. `data/postgres-demo/init/01_roles.sql`: tạo người dùng chỉ đọc `analyst` (mật khẩu `analyst`).
2. `init/02_load.sh`: một giao dịch psql gồm `staging.sql`, chép CSV, `edu_schema.sql`, `edu_load.sql`,
   `sales_schema.sql`, `sales_load.sql`, `grants.sql` của `data/postgres-demo/sql`, rồi xóa `staging`.

Không chạy `plant_demo_cause_sales.sql`, `plant_demo_cause_edu.sql`: dữ liệu giữ đúng như CSV. Muốn nạp lại từ đầu:
`docker compose rm -sf demo-db && docker volume rm <tên dự án>_demodata && docker compose up -d demo-db`.

| Người dùng | Mật khẩu | Quyền |
|---|---|---|
| `analyst` | `analyst` | Chỉ `SELECT` trên `edu` và `sales`, giao dịch mặc định chỉ đọc |
| `demo` | `DEMO_DB_PASSWORD`, mặc định `demo` | Chủ database, chỉ file init dùng |

Chuỗi kết nối: trong compose `postgres://analyst:analyst@demo-db:5432/company_demo`; từ máy (`make dev`, `make eval`)
`postgres://analyst:analyst@127.0.0.1:5434/company_demo`, cổng đổi bằng `DEMO_DB_PORT`. Cổng chỉ mở trên `127.0.0.1`.
Mật khẩu ghi ngay ở đây vì đây là dữ liệu mẫu công khai; bản chạy thật tắt dịch vụ này (docs/INSTALL.md mục 3).

## Dữ liệu

| Schema | Nguồn | Số dòng |
|---|---|---|
| `sales` | `data/samples/dataco.csv` (1500 dòng DataCo lấy ngẫu nhiên) | `departments` 11, `categories` 47, `products` 83, `customers` 1385, `orders` 1466, `order_items` 1500 |
| `edu` | `data/samples/oulad/` (bản nhỏ của OULAD) | `courses` 22, `assessments` 206, `vle_materials` 6364, `students` 660, `registrations` 660, `student_assessments` 3368, `student_daily_clicks` 36978 |

Có đủ 12 khóa ngoại như bản đầy đủ; nối `sales.order_items` với mọi bảng chiều vẫn ra 1500 dòng. Vì `dataco.csv` lấy
ngẫu nhiên từng dòng hàng, phần lớn đơn chỉ còn một dòng hàng.

`data/samples/oulad/` dựng bằng `build_oulad_subset.py` từ bản đầy đủ (`sh data/download.sh oulad`):
`python3 data/demo-db/build_oulad_subset.py`. Mỗi học phần (22 cặp môn, kỳ) lấy 30 sinh viên ngẫu nhiên với hạt giống
cố định, giữ mọi dòng đăng ký và điểm của họ trong đúng học phần đó. `courses.csv`, `assessments.csv`, `vle.csv` giữ
mọi dòng. `studentVle.csv` (454 MB) không chép: lượt bấm của các sinh viên đã chọn gộp sẵn theo (sinh viên, ngày) thành
`studentDailyClicks.csv`, đúng cách `edu_load.sql` gộp bản đầy đủ. Mỗi file dưới 1 MB.

## Giấy phép

DataCo và OULAD theo CC BY 4.0, ghi công ở [data/README.md](../README.md). Bản ở đây đã biến đổi: lấy mẫu, gộp lượt
bấm, tách bảng như mô tả ở [data/postgres-demo/README.md](../postgres-demo/README.md).
