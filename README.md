# xdi-platform

Nền tảng phân tích dữ liệu bằng ngôn ngữ tự nhiên: hỏi bằng tiếng Việt, nhận câu trả lời kèm lời giải thích và minh
chứng cho từng con số.

Kết nối database Postgres của tổ chức, chụp các bảng cần hỏi cùng các bảng liên quan theo khóa ngoại, rồi gõ câu hỏi;
dữ liệu chưa nằm trong database thì nạp file CSV, Parquet hoặc Excel. Hệ thống viết SQL (nối nhiều bảng khi cần), kiểm
SQL bằng guard, chạy trên DuckDB và trả về bảng, biểu đồ cùng câu SQL đã chạy. Với câu hỏi tìm nguyên nhân, như vì sao
doanh thu tháng này giảm hay vì sao Europe cao hơn LATAM, hệ thống tính nhóm nào tạo ra phần lớn chênh lệch; bấm vào một
số để xem các dòng dữ liệu đứng sau nó.

Con số do DuckDB tính trên máy của bạn. Mô hình ngôn ngữ viết mô tả cột lúc nạp, phân loại câu hỏi, viết SQL, điền
form cho câu hỏi tìm nguyên nhân và viết lời. Nó nhận metadata của mọi cột (tên, kiểu, mô tả), danh sách giá trị của
cột phân loại không đánh dấu nhạy cảm (người bị lọc dòng theo vai dữ liệu thì không), và các ý do code tính (con số tổng
hợp kèm tên nhóm); không nhận dòng kết quả nào hay nội dung file. Chi tiết: [docs/PRIVACY.md](docs/PRIVACY.md).

Bản phát hành mới nhất: [v0.2.0](https://github.com/pgiahuy/xdi-platform/releases). Giấy phép Apache 2.0.
Tài liệu: https://pgiahuy.github.io/xdi-platform/

![Một câu hỏi đi qua xdi-platform: hàng trên đi từ giao diện qua dựng ngữ cảnh, mô hình, guard SQL tới DuckDB, hàng dưới quay về qua phần giải thích, kiểm số, ghi lịch sử; 16 bước](docs/images/luong-cau-hoi.png)

## Chạy thử

Cần Docker kèm Docker Compose v2 và Git.

```bash
git clone https://github.com/pgiahuy/xdi-platform.git
cd xdi-platform
cp .env.example .env
docker compose up -d --build
docker compose logs backend | grep setup_code
```

Mở http://localhost:8000, nhập mã thiết lập vừa in để tạo tài khoản quản trị. Workspace đầu tiên có sẵn dataset mẫu
DataCo (1500 dòng, từ `data/samples/dataco.csv`). Mô hình mặc định là `mock`: không cần khóa, chỉ trả lời các câu mẫu,
ví dụ "Doanh thu theo thị trường". Kết nối database của bạn ở trang **Dữ liệu**, khung **Kết nối database**. Hỏi câu tự
do thì đặt `LLM_PROVIDER=gemini` và `GEMINI_API_KEY` (hoặc `ollama`) trong `.env`, rồi chạy lại `docker compose up -d`.

Thử luồng database với database mẫu nhiều bảng (DataCo và OULAD đầy đủ, 13 bảng có khóa ngoại,
[data/postgres-demo](data/postgres-demo/README.md)): tải và nạp một lần (`sh data/download.sh dataco`,
`sh data/download.sh oulad`, `sh data/postgres-demo/load.sh`, rồi gỡ container chạy riêng đó, xem README ở đó), sau đó
chạy `docker compose --profile demo up -d --build` trước lần thiết lập đầu. Thiết lập lần đầu thêm database này làm nguồn và
chụp 12 bảng kèm quan hệ thành dataset; câu mẫu của `mock` trên đó có "Tỷ lệ bỏ học theo môn học" trên `students` và
"Doanh thu theo phân khúc khách hàng" trên `order_items` (nối `orders`, `customers`).

Build lần đầu biên dịch Rust và DuckDB (đo trên máy 16 nhân: 8 phút) và cần khoảng 1 GiB RAM trống cho mỗi nhân CPU; máy
ít RAM thì đặt `CARGO_BUILD_JOBS`. Hướng dẫn đầy đủ (build từ nguồn, đưa lên Internet, sao lưu, lỗi hay gặp, dùng lần
đầu): [docs/INSTALL.md](docs/INSTALL.md).

## Dữ liệu dùng trong repo

Bốn bộ, đều giấy phép CC BY 4.0: ba bộ bán hàng và chuỗi cung ứng, một bộ giáo dục. Chỉ bộ mẫu nằm trong repo; các
bộ đầy đủ tải bằng `data/download.sh` (kiểm SHA-256) vào `data/downloads/`, git bỏ qua thư mục này.

| Bộ | Lấy ở đâu | Dùng cho |
|---|---|---|
| DataCo mẫu, 1500 dòng | Có sẵn: `data/samples/dataco.csv`, kèm chỉ số mẫu `dataco.metrics.json` | Dataset nạp sẵn khi thiết lập lần đầu không tới được database mẫu, câu mẫu của `mock`, bộ đánh giá `make eval` trên CI |
| DataCo Smart Supply Chain đầy đủ, khoảng 96 MB CSV | `sh data/download.sh dataco` | Thử trên dữ liệu lớn; nạp với tên `DataCo` vào workspace chưa có DataCo thì câu mẫu của `mock` vẫn chạy (đã có thì bảng thành `dataco_2`) |
| Online Retail II, khoảng 46 MB Excel | `sh data/download.sh retail` | Thử file Excel nhiều sheet (bản đầy đủ chưa thử nạp, giới hạn Excel ở [DATA.md](docs/DATA.md)); chưa có câu mẫu cho `mock`, cần Gemini hoặc Ollama |
| OULAD (Open University Learning Analytics), 7 bảng CSV, khoảng 464 MB | `sh data/download.sh oulad` | Dữ liệu giáo dục: nạp `studentInfo.csv` (32 593 dòng, mỗi dòng một sinh viên trong một học phần) để hỏi theo kết quả học tập; cần Gemini hoặc Ollama |

DataCo từ Mendeley Data (Constante, Silva, Pereira, 2019); Online Retail II (Chen, 2019) và OULAD (Kuzilek, Hlosta,
Zdrahal, 2015) từ UCI Machine Learning Repository. Đường dẫn nguồn, ghi công và cách nạp:
[data/README.md](data/README.md). Bản đầy đủ của DataCo và OULAD tách thành nhiều bảng trong database mẫu duy nhất của
repo: [data/postgres-demo](data/postgres-demo/README.md).

## Đã có

- **Tài khoản và workspace:** mỗi người nhiều workspace; vai `admin` (nạp, xóa dữ liệu, quản lý thành viên) và
  `member` (xem, hỏi); quản trị hệ thống quản lý tài khoản.
- **Kết nối database:** thêm nguồn Postgres (thông tin kết nối mã hóa, tài khoản chỉ đọc, TLS với `require` hay
  `verify-full`), chụp một bảng cùng các bảng liên quan theo khóa ngoại, mỗi bảng một dataset có ghi quan hệ; câu hỏi
  nối các bảng đó, có chặn cộng lặp khi nối. Nạp file là lựa chọn phụ: CSV (UTF-8, windows-1252), Parquet, Excel .xlsx
  kèm chọn sheet. Mỗi lần chụp hay nạp là một phiên bản. Hồ sơ cột lập lúc nạp; mô hình thêm mô tả cột, bí danh và câu
  gợi ý, gồm cả câu hỏi tìm nguyên nhân.
- **Hỏi theo cuộc trò chuyện:** câu sau hiểu theo các câu trước. Câu hỏi qua guard (một câu `SELECT` chỉ đọc, trên
  bảng, cột, hàm được phép, có trần số dòng) rồi chạy trên DuckDB; lỗi thì mô hình sửa một lần. Kết quả có bảng, biểu
  đồ, tải CSV, thẻ "Hệ thống đã hiểu là" và cảnh báo khi kết quả đáng ngờ.
- **Câu hỏi tìm nguyên nhân** (vì sao một số tăng, giảm hay khác nhau giữa hai nhóm): mô hình điền form (đo gì, hai kỳ
  hay hai nhóm nào), code kiểm form và tự dựng mọi SQL, tính phần chênh lệch của từng nhóm (cộng lại bằng đúng chênh
  lệch). Code tính các nhận định, mô hình viết insight (câu mở đầu và các nguyên nhân chính) kèm khai từng số và nhóm
  nó dùng; code so mỗi khai báo với đúng ô của bảng đã tính, rồi một lần gọi mô hình riêng so nghĩa của lời với các
  khai báo. Sai hay có chỗ đọc được nhiều nghĩa thì mô hình viết lại một lần, kèm các lỗi đó; vẫn sai thì dùng câu do
  code viết, còn chỗ chưa rõ mà không chắc sai thì giữ insight, kèm cảnh báo.
- **Chỉ số:** admin định nghĩa chỉ số của từng dataset (chọn phép tính và cột, hoặc viết SQL; chạy thử trước khi lưu)
  và đặt cột số đo nào lấy tổng, cột nào lấy trung bình; câu hỏi sau dùng đúng công thức đó.
- **Minh chứng:** câu SQL trước và sau guard, bảng phân rã, các bước đã chạy, và dòng nguồn của ô số đo trong bảng kết
  quả hay bảng phân rã (câu có CTE, HAVING, hàm cửa sổ hay nhiều bảng chưa truy được).
- **Mô hình ngôn ngữ:** `mock`, Gemini, Ollama.
- **Kiểm chất lượng:** `make eval` hỏi 14 câu mẫu trên file DataCo, và khi database mẫu đang chạy thì thêm 17 câu trên
  ảnh chụp của nó (10 câu bán hàng, 7 câu học tập, phần lớn nối bảng), so với SQL đúng viết tay; CI chạy phần DataCo và
  báo khi tỷ lệ đúng giảm. `make eval-judge LLM_PROVIDER=gemini` đo mô hình chấm lời giải thích câu "tại sao" trên 55 ca
  có nhãn: số ca đúng trên tổng, số lời sai bị bỏ lọt hay chỉ bị cảnh báo, số lời đúng bị chặn nhầm hay bị cảnh báo
  thừa.

## Chưa có

- Gán vai dữ liệu cho thành viên: guard đã lọc dòng theo vai dữ liệu, nhưng chưa có API hay màn để đặt.
- Chế độ demo, màn xem lịch sử câu hỏi, màn sửa mô tả, bí danh và vai trò cột.
- Nguồn MySQL, SQLite; truy vấn thẳng database ngoài lúc hỏi (hiện là chụp bảng); `sslmode=verify-ca` và chứng chỉ phía
  client.

Bảng trạng thái chi tiết: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#trạng-thái). Lịch sử thay đổi:
[CHANGELOG.md](CHANGELOG.md). Tiến độ: [milestones](https://github.com/pgiahuy/xdi-platform/milestones).

## Build từ nguồn và phát triển

Cần Rust stable từ 1.89 kèm trình biên dịch C++, Node.js 24, Docker (cho Postgres) và GNU Make.

```bash
make dev            # Postgres trong Docker, backend bằng cargo run ở cổng 8000
make dev-frontend   # giao diện ở http://localhost:5173
make check-fast     # fmt, clippy, test
make help           # mọi lệnh
```

```
backend/     crate Rust: api/ -> service/ -> engine/ và auth/; migrations/ cho Postgres
frontend/    React, Vite, Tailwind, shadcn/ui; build ra file tĩnh, backend phục vụ
docs/        tài liệu, cũng là nguồn GitHub Pages
data/        nguồn và giấy phép dataset, samples/ (DataCo mẫu), postgres-demo/ (database mẫu), domains/ (gói ngành)
```

| Tài liệu | Nội dung |
|---|---|
| [INSTALL.md](docs/INSTALL.md) | Cài đặt, cấu hình, sao lưu, lỗi hay gặp, dùng lần đầu |
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | Kiến trúc, luồng một câu hỏi, trạng thái từng phần |
| [API.md](docs/API.md) | API, mã lỗi; đặc tả OpenAPI ở `/api/openapi.json`, thử trên trình duyệt ở `/api/docs` |
| [GUARD.md](docs/GUARD.md) | Guard SQL: luật, hạn chế đã biết |
| [DATA.md](docs/DATA.md) | Định dạng file nhận, phiên bản, tên cột |
| [PRIVACY.md](docs/PRIVACY.md) | Dữ liệu nằm ở đâu, gửi gì cho mô hình |
| [PERFORMANCE.md](docs/PERFORMANCE.md) | Thời gian, số lượt gọi mô hình và chi phí mỗi câu hỏi, đã đo |

## Tham gia

- Lỗi: [Issues](https://github.com/pgiahuy/xdi-platform/issues). Câu hỏi và ý tưởng:
  [Discussions](https://github.com/pgiahuy/xdi-platform/discussions).
- Đóng góp mã: [CONTRIBUTING.md](CONTRIBUTING.md); quy tắc làm việc: [AGENTS.md](AGENTS.md).
- Lỗ hổng bảo mật: báo riêng theo [SECURITY.md](SECURITY.md), không mở issue công khai.

Giấy phép: [Apache License 2.0](LICENSE); thành phần bên thứ ba: [docs/LICENSES.md](docs/LICENSES.md).
