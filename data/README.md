# Dữ liệu mẫu

`samples/` dành cho vài file nhỏ (dưới 1 MB) để chạy ngay và để test.
Compose gắn thư mục này vào container backend ở `/samples`, chỉ đọc. `domains/` chứa gói khái niệm ngành
([docs/DOMAINS.md](../docs/DOMAINS.md)): được commit và backend nhúng vào binary lúc biên dịch, thêm một ngành chỉ cần
thêm một file JSON.

| File trong `samples/` | Nguồn | Giấy phép | Dùng cho |
|---|---|---|---|
| `dataco.csv` (1500 dòng lấy ngẫu nhiên) | Mendeley Data, https://doi.org/10.17632/8gx2fvg2k6.5 | CC BY 4.0 | Dataset mẫu của workspace đầu tiên khi không chụp được database mẫu, bộ đánh giá câu hỏi mẫu (`make eval`), thử nhanh với mock |
| `dataco.metrics.json` | Viết cho dự án này | Apache 2.0, như mã nguồn | Chỉ số mẫu của dataset DataCo (bảng `metrics`) |
| `order_items.metrics.json`, `student_assessments.metrics.json` | Viết cho dự án này | Apache 2.0, như mã nguồn | Chỉ số mẫu của hai bảng thiết lập lần đầu chụp từ database mẫu ([postgres-demo](postgres-demo/README.md)) |
| `suggested_questions.json` | Viết cho dự án này | Apache 2.0, như mã nguồn | Câu gợi ý chọn tay của `sales_demo` và `edu_demo` trên trang hỏi ([docs/DATA.md](../docs/DATA.md#dataset-mẫu)) |

## Dữ liệu mẫu lúc thiết lập lần đầu

Thiết lập lần đầu thêm mỗi database mẫu tới được ([postgres-demo](postgres-demo/README.md), chọn bằng
`SAMPLE_DATABASE_URL`) làm một dataset của workspace đầu tiên và chụp mọi bảng tài khoản `analyst` đọc được, như admin
kết nối ở trang **Dữ liệu** mà không bỏ tick bảng nào: 13 bảng, khóa ngoại ghi thành quan hệ. Bảng có
`<tên bảng>.metrics.json` (hiện là `order_items`, `student_assessments`) thì có thêm chỉ số mẫu. Hồ sơ cột và câu mẫu
của `mock` cho các bảng này nằm ở `backend/src/engine/llm/mock_answers_database.json`.

Không database nào chụp được (không đặt `SAMPLE_DATABASE_URL`, database mẫu không chạy hay chưa nạp, không có khóa
mã hóa nguồn dùng được), backend ghi cảnh báo và nạp `<SAMPLES_DIR>/dataco.csv` thành dataset `DataCo` (bảng
`dataco`, bảng mà câu mẫu DataCo của `mock` viết cho) qua đúng đường nạp file, kể cả hồ sơ cột. Sau đó ghi các chỉ số trong
`dataco.metrics.json` vào bảng `metrics` (`origin` là `sample`); chỉ số không qua guard hay không chạy được thì bỏ qua
và ghi log.

- Chỉ chạy một lần, lúc thiết lập: khởi động lại không nạp thêm, dataset mẫu đã xóa thì không quay lại. Lỗi khi nạp mẫu
  chỉ ghi log, không chặn việc tạo tài khoản quản trị.
- `SEED_SAMPLE=false` để workspace đầu tiên trống; `SAMPLES_DIR` là thư mục chứa các file trên (mặc định
  `../data/samples` tính từ `backend/`, compose đặt `/samples`). Ý nghĩa và mặc định:
  [docs/INSTALL.md](../docs/INSTALL.md#cấu-hình).
- Bản cài đã thiết lập rồi thì kết nối database mẫu ở trang **Dữ liệu**, hoặc nạp tay `data/samples/dataco.csv` với tên
  dataset `DataCo` ([docs/USAGE.md](../docs/USAGE.md#1-chọn-mô-hình-và-dữ-liệu-mẫu)).

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

Câu mẫu DataCo của `mock` (`backend/src/engine/llm/mock_answers.json`) viết cho bảng `dataco`: nạp bản đầy đủ với tên
dataset `DataCo` thì câu mẫu vẫn chạy (mỗi file là một dataset riêng, bảng vẫn tên `dataco`). File Online Retail II và
OULAD chưa có chỉ số mẫu hay câu mẫu cho `mock`, hỏi trên đó cần Gemini hoặc Ollama. Với file OULAD, bảng dễ hỏi nhất là
`studentInfo.csv` (32 593 dòng, mỗi dòng một sinh viên trong một học phần). Online Retail II bản đầy đủ chưa được thử
nạp; giới hạn của file Excel ở [docs/DATA.md](../docs/DATA.md).

Bản đầy đủ của DataCo và OULAD, tách thành nhiều bảng có khóa chính, khóa ngoại trong hai database `sales_demo`,
`edu_demo`: [postgres-demo/README.md](postgres-demo/README.md).

Khi thêm dataset: ghi nguồn, giấy phép, và kiểm tra giấy phép cho phép phân phối lại trước khi đưa vào `samples/`.
