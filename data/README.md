# Dữ liệu mẫu

`samples/` dành cho vài file nhỏ (dưới 1 MB) để chạy ngay và để test.
Compose gắn thư mục này vào container backend ở `/samples`, chỉ đọc. `domains/` chứa gói khái niệm ngành
([docs/DOMAINS.md](../docs/DOMAINS.md)): được commit và backend nhúng vào binary lúc biên dịch, thêm một ngành chỉ cần
thêm một file JSON.

| File trong `samples/` | Nguồn | Giấy phép | Dùng cho |
|---|---|---|---|
| `dataco.csv` (1500 dòng lấy ngẫu nhiên) | Mendeley Data, https://doi.org/10.17632/8gx2fvg2k6.5 | CC BY 4.0 | Dataset mẫu nạp sẵn vào workspace đầu tiên, bộ đánh giá câu hỏi mẫu (`make eval`), thử nhanh với mock |
| `dataco.metrics.json` | Viết cho dự án này | Apache 2.0, như mã nguồn | Chỉ số mẫu của dataset DataCo (bảng `metrics`) |
| `sample_database.json`, `order_items.metrics.json`, `student_assessments.metrics.json` | Viết cho dự án này | Apache 2.0, như mã nguồn | Các nhóm bảng thiết lập lần đầu chụp từ database mẫu ([postgres-demo](postgres-demo/README.md)), và chỉ số mẫu của hai bảng |

## Dataset mẫu nạp sẵn

Khi màn thiết lập lần đầu tạo xong workspace đầu tiên, nếu tới được database mẫu ([postgres-demo](postgres-demo/README.md),
đã nạp bằng `load.sh`; compose gốc trỏ `SAMPLE_DATABASE_URL` vào hai database `sales_demo` và `edu_demo` của dịch vụ
`demo-db` khi bật `--profile demo`, `make dev` trỏ vào cổng `DEMO_DB_PORT` trên `127.0.0.1`) thì backend thêm mỗi
database làm một nguồn Postgres của workspace, rồi chụp các nhóm bảng ghi cho database đó (trường `database`) trong
`sample_database.json` (bảng bắt đầu và các bảng chụp kèm, như request chụp ở trang **Dữ liệu**): mỗi bảng một
dataset, khóa ngoại ghi thành quan hệ. Lần chụp không ghi `database` áp cho mọi database mẫu. Kế hoạch chụp 12 bảng; bỏ `edu.student_daily_clicks` (1,8 triệu dòng,
quá `SOURCE_MAX_ROWS` mặc định). Bảng nào có `<tên bảng>.metrics.json` (hiện có `order_items.metrics.json`,
`student_assessments.metrics.json`) thì ghi thêm chỉ số mẫu. Hồ sơ cột và câu gợi ý của `mock` cho các bảng này nằm ở
`backend/src/engine/llm/mock_answers_database.json`.

Không có `SAMPLE_DATABASE_URL` (để rỗng, hay chạy binary trực tiếp không đặt biến này; `make dev` luôn đặt nó trỏ vào
`127.0.0.1:5440`), hoặc không database nào chụp được (database mẫu không chạy hay chưa nạp, không có khóa mã hóa dùng
được), backend ghi cảnh báo và nạp `<SAMPLES_DIR>/dataco.csv` thành dataset tên `DataCo`
(bảng `dataco`, bảng mà các câu mẫu DataCo của `LLM_PROVIDER=mock` viết cho), qua đúng đường nạp file ở trang **Dữ
liệu**, kể cả hồ sơ cột. Sau đó ghi các chỉ số trong `dataco.metrics.json` vào bảng `metrics` của dataset đó (`origin`
là `sample`, đã xác nhận); chỉ số nào không qua guard hay không chạy được trên dữ liệu thì bỏ qua và ghi log. Prompt
sinh SQL dùng các chỉ số này làm công thức của tổ chức.

- `SEED_SAMPLE=false`: không nạp, workspace đầu tiên để trống.
- `SAMPLES_DIR`: thư mục chứa các file trên. Mặc định `../data/samples` tính từ `backend/` (nơi `make dev` chạy
  `cargo run`); compose đặt `/samples`. Thư mục không có `dataco.csv` thì bỏ qua, không lỗi.
- Chỉ chạy một lần, lúc thiết lập: khởi động lại không nạp thêm, và đã xóa dataset mẫu thì nó không quay lại. Bản cài
  thiết lập trước khi có bước này thì kết nối database mẫu ở trang **Dữ liệu** (docs/INSTALL.md mục 7, bước 1), hoặc nạp
  tay `data/samples/dataco.csv` với tên dataset `DataCo`.
- Lỗi khi nạp mẫu chỉ ghi log mức cảnh báo, không chặn việc tạo tài khoản quản trị.

Mỗi chỉ số trong `dataco.metrics.json` có `name`, `description`, `expression` (biểu thức tổng hợp trên cột thật),
`filter`, `time_column`, `unit` (ba trường sau có thể là `null`), như cột cùng tên của bảng `metrics`.

## Tải bản đầy đủ

Dataset đầy đủ không commit vào repo. `sh data/download.sh` tải vào `data/downloads/` (git bỏ qua), kiểm SHA-256 từng
file, file đã có và đúng checksum thì bỏ qua; `sh data/download.sh dataco`, `retail` hay `oulad` để tải một bộ. Tải
không được hay checksum lệch thì script in trang nguồn để tải tay. Nạp file tải về ở trang **Dữ liệu**.

| Dataset | File trong `downloads/` | Nguồn | Giấy phép, ghi công |
|---|---|---|---|
| DataCo Smart Supply Chain for Big Data Analysis | `DataCoSupplyChainDataset.csv` (khoảng 96 MB, windows-1252) | Mendeley Data, https://doi.org/10.17632/8gx2fvg2k6.5 | CC BY 4.0; Fabian Constante, Fernando Silva, António Pereira (2019) |
| Online Retail II | `online_retail_ii.zip` và `online_retail_II.xlsx` giải nén từ đó (khoảng 46 MB) | UCI Machine Learning Repository, https://archive.ics.uci.edu/dataset/502/online+retail+ii | CC BY 4.0; Daqing Chen (2019), https://doi.org/10.24432/C5CG6D |
| Open University Learning Analytics (OULAD) | `oulad.zip` (khoảng 47 MB) và 7 file CSV giải nén vào `oulad/` (tổng khoảng 464 MB, riêng `studentVle.csv` 454 MB) | UCI Machine Learning Repository, https://archive.ics.uci.edu/dataset/349/open+university+learning+analytics+dataset | CC BY 4.0; Jakub Kuzilek, Martin Hlosta, Zdenek Zdrahal (2015), https://doi.org/10.24432/C5KK69 |

Câu mẫu DataCo của mock (`backend/src/engine/llm/mock_answers.json`) viết cho bảng `dataco`: nạp bản đầy
đủ của DataCo với tên dataset `DataCo` vào workspace chưa có dataset đó (đã có thì bảng mới thành `dataco_2`). Online
Retail II và OULAD chưa có chỉ số mẫu hay câu mẫu cho mock, hỏi trên đó cần Gemini hoặc Ollama. OULAD là dữ liệu
học tập: bảng dễ hỏi nhất là `studentInfo.csv` (32 593 dòng, mỗi dòng một sinh viên trong một học phần, có giới tính,
khu vực, trình độ, kết quả cuối khóa); đã nạp thử qua trang **Dữ liệu** và hỏi với Gemini. Online Retail II bản đầy
đủ chưa được thử nạp qua trang **Dữ liệu**, giới hạn của file Excel ở [docs/DATA.md](../docs/DATA.md).

Cùng hai bộ DataCo và OULAD, tách thành nhiều bảng có khóa chính, khóa ngoại trong một Postgres riêng để thử kết nối
cơ sở dữ liệu công ty: [postgres-demo/README.md](postgres-demo/README.md).

Khi thêm dataset: ghi nguồn, giấy phép, và kiểm tra giấy phép cho phép phân phối lại trước khi đưa vào `samples/`.
