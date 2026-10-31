# Cài đặt

Trang này dành cho người cài và quản trị một bản xdi-platform: cài, cấu hình, đưa lên Internet, sao lưu, sửa lỗi. Cách
dùng giao diện (kết nối database, hỏi, đọc minh chứng, báo cáo) ở [Dùng lần đầu](USAGE.md). Phần nào đã có, phần nào
chưa: bảng Trạng thái trong [ARCHITECTURE.md](ARCHITECTURE.md#trạng-thái).

Chạy nhanh trên một máy có Docker:

```bash
git clone https://github.com/pgiahuy/xdi-platform.git && cd xdi-platform
cp .env.example .env
docker compose up -d --build                      # build lần đầu biên dịch Rust và DuckDB
docker compose logs backend | grep setup_code     # mã để tạo tài khoản quản trị
```

Mở http://localhost:8000 và nhập mã thiết lập. Workspace đầu tiên có sẵn dataset mẫu DataCo, hỏi được ngay với mô hình
`mock`. Muốn thử với database mẫu nhiều bảng thì nạp nó trước lần thiết lập đầu ([Dùng lần đầu](USAGE.md#1-chọn-mô-hình-và-dữ-liệu-mẫu)).

| Mục | Khi nào cần |
|---|---|
| [1. Chạy bằng Docker](#1-chạy-bằng-docker) | Cài, tạo tài khoản quản trị, cấu hình `.env`, mô hình ngôn ngữ, cập nhật, gỡ |
| [2. Build từ mã nguồn](#2-build-từ-mã-nguồn) | Phát triển, hoặc chạy backend không dùng Docker |
| [3. Đưa lên Internet](#3-đưa-lên-internet) | Máy chủ có tên miền, HTTPS bằng Caddy hoặc nginx |
| [4. Sao lưu và khôi phục](#4-sao-lưu-và-khôi-phục) | `make backup`, `make restore` |
| [5. Quản trị](#5-quản-trị) | Quên mật khẩu quản trị |
| [6. Lỗi hay gặp](#6-lỗi-hay-gặp) | Build hết RAM, trùng cổng, lỗi Gemini |

## 1. Chạy bằng Docker

Cần Docker Engine kèm Docker Compose v2 (`docker compose version` chạy được) và Git. Cài Docker theo
https://docs.docker.com/engine/install/; trên Linux, để chạy `docker` không cần `sudo` thì làm theo
https://docs.docker.com/engine/install/linux-postinstall/. Trên Windows, chạy các lệnh trong WSL2.

Build image cần khoảng 1 GiB RAM trống cho mỗi tiến trình biên dịch, và cargo mặc định chạy một tiến trình mỗi nhân CPU.
Máy ít RAM thì đặt `CARGO_BUILD_JOBS` trong `.env` bằng khoảng số GiB RAM còn trống, nếu không máy có thể đứng giữa
chừng (mục 6).

```bash
git clone https://github.com/pgiahuy/xdi-platform.git
cd xdi-platform
cp .env.example .env
docker compose up -d --build
docker compose ps                         # backend và postgres ở trạng thái healthy
curl http://localhost:8000/api/health     # {"status":"ok"}
```

Lần đầu build lâu vì phải biên dịch backend (Rust, kèm DuckDB) và giao diện. Các lần sau dùng lại cache; đổi phụ thuộc
(`Cargo.lock`) thì biên dịch lại phần phụ thuộc.

### Tạo tài khoản quản trị

Khi chưa có tài khoản nào, backend in một mã thiết lập ra log. Chỉ ai đọc được log của máy chủ mới tạo được tài khoản
quản trị, nên người ngoài không chiếm được bản cài vừa dựng.

```bash
docker compose logs backend | grep setup_code
```

Mở http://localhost:8000: màn **Thiết lập lần đầu** hiện ra. Nhập mã (chữ thường hay thừa khoảng trắng vẫn nhận), email
và mật khẩu từ 8 đến 128 ký tự; tên hiển thị và tên workspace đầu tiên không bắt buộc. Máy chủ không có trình duyệt thì
tạo bằng lệnh:

```bash
curl http://localhost:8000/api/auth/setup \
  -H 'content-type: application/json' \
  -d '{"setup_code":"ABCD-EFGH-JKMN","email":"ban@example.com","password":"mat khau it nhat 8 ky tu","workspace_name":"Cửa hàng"}'
```

Tài khoản này là quản trị hệ thống (quản lý tài khoản của cả bản cài) và là `admin` của workspace đầu tiên. Mã chỉ dùng
được một lần; khởi động lại backend khi vẫn chưa có tài khoản nào thì backend in mã mới.

### Nạp dữ liệu bằng script

Kết nối database và nạp file trên giao diện: [Dùng lần đầu](USAGE.md#2-kết-nối-database-hoặc-nạp-file). Máy chủ database
phải có trong `SOURCE_ALLOWED_HOSTS`. API của nguồn database: [API.md](API.md), mục Nguồn database.

Nạp file bằng `curl`: đăng nhập để lấy cookie phiên vào `cookie.txt` (file này là chìa khóa phiên của bạn, xóa sau khi
dùng), rồi gửi file. `<slug>` là slug của workspace, có trong kết quả lệnh đăng nhập, ví dụ `cua-hang`.

```bash
curl -c cookie.txt http://localhost:8000/api/auth/login \
  -H 'content-type: application/json' \
  -d '{"email":"ban@example.com","password":"mat khau it nhat 8 ky tu"}'
curl -b cookie.txt http://localhost:8000/api/workspaces/<slug>/datasets \
  -F name='Bán hàng' -F file=@thang1.csv -F file=@thang2.csv
```

Nhiều file CSV hay Parquet cùng cột gộp thành một bảng. File Excel (.xlsx) nạp từng file một: xem tên sheet trước, rồi
chọn sheet (lặp `-F sheet=` để gộp nhiều sheet cùng cột):

```bash
curl -b cookie.txt http://localhost:8000/api/workspaces/<slug>/datasets/sheets -F file=@ban_hang.xlsx
curl -b cookie.txt http://localhost:8000/api/workspaces/<slug>/datasets \
  -F name='Bán hàng' -F sheet='Năm 2023' -F sheet='Năm 2024' -F file=@ban_hang.xlsx
```

Định dạng nhận, mã hóa CSV, giới hạn Excel, phiên bản và tên cột: [Dữ liệu](DATA.md).

### Cấu hình

Sửa `.env` rồi chạy lại `docker compose up -d`. Danh sách đầy đủ, với giá trị mặc định, ở
[.env.example](https://github.com/pgiahuy/xdi-platform/blob/main/.env.example).

| Biến | Mặc định | Ý nghĩa |
|---|---|---|
| `BACKEND_PORT` | `8000` | Cổng của giao diện và API |
| `BACKEND_BIND` | trống (mọi địa chỉ) | Địa chỉ backend nghe trên máy host khi chạy compose; `127.0.0.1` khi có reverse proxy trên cùng máy (mục 3) |
| `POSTGRES_PORT` | `5433` | Cổng Postgres của ứng dụng mở ra máy host |
| `DB_MAX_CONNECTIONS` | `10` | Số kết nối Postgres tối đa của backend |
| `DB_ACQUIRE_TIMEOUT_SECS` | `5` | Số giây một request chờ kết nối Postgres khi mọi kết nối đang bận; quá thì request nhận lỗi |
| `PUBLIC_URL` | trống | Địa chỉ công khai; bắt đầu bằng `https://` thì cookie đăng nhập có cờ `Secure` (mục 3) |
| `MAX_UPLOAD_MB` | `200` | Tổng dung lượng tối đa của một lần nạp file |
| `QUERY_TIMEOUT_SECS` | `30` | Thời gian tối đa một câu SQL của câu hỏi chạy trên DuckDB ([Guard SQL](GUARD.md), mục Hạn chế đã biết) |
| `ASK_MAX_ROWS` | `100000` | Trần số dòng guard đặt cho SQL của câu hỏi; giao diện nhận 1000 dòng đầu |
| `RUST_LOG` | `info` | Mức log |
| `LLM_PROVIDER` | `mock` | `mock` (không gọi ra ngoài), `gemini` hay `ollama` (bên dưới) |
| `GEMINI_API_KEY` | trống | Khóa Gemini, tạo trong Google AI Studio |
| `GEMINI_BACKUP_API_KEY` | trống | Khóa dự phòng: khóa chính hết hạn mức (lỗi 429) thì backend gọi lại bằng khóa này trong cùng lần gọi |
| `GEMINI_MODEL` | `gemini-3.5-flash-lite` | Mô hình Gemini |
| `GEMINI_TIMEOUT_SECS` | `60` | Thời gian chờ tối đa một lần gọi Gemini |
| `EVAL_GEMINI_API_KEY` | trống | Khóa Gemini cho các lệnh `xdi-backend eval*`; server không đọc biến này (bên dưới) |
| `OLLAMA_URL` | compose: `http://host.docker.internal:11434` | Địa chỉ Ollama; chạy ngoài compose thì `http://localhost:11434` |
| `OLLAMA_MODEL` | `qwen2.5-coder:7b` | Mô hình Ollama |
| `OLLAMA_TIMEOUT_SECS` | `300` | Thời gian chờ tối đa một lần gọi Ollama |
| `OLLAMA_NUM_CTX` | `12288` | Cửa sổ ngữ cảnh (token) gửi Ollama trong `options.num_ctx` |
| `CLASSIFY_TIMEOUT_SECS` | `15` | Thời gian tối đa của bước phân loại câu hỏi; quá thì dùng luật từ khóa |
| `SEMANTIC_JOBS` | `2` | Số lượt lập hồ sơ cột bằng mô hình chạy cùng lúc; mỗi lượt gửi ba prompt song song |
| `ANSWER_TEXT` | `true` | Đoạn trả lời ngắn của câu hỏi số liệu dùng lời giải thích đã qua kiểm; `false` thì luôn là câu khuôn |
| `INSIGHT_TEXT` | `true` | Mô hình viết lời giải thích cho câu hỏi số liệu; `false` thì chỉ có câu do code viết |
| `SEED_SAMPLE` | `true` | Thiết lập lần đầu cho workspace đầu tiên dữ liệu mẫu; `false` để workspace trống |
| `SAMPLE_DATABASE_URL` | compose: `sales_demo`, `edu_demo` của `demo-db` | Các database mẫu mà thiết lập lần đầu thêm làm dataset (bên dưới) |
| `DEMO_DB_PORT` | `5440` | Cổng database mẫu trên `127.0.0.1` của máy host, cho `make dev` và `make eval` |
| `DEMO_DB_CONTAINER`, `DEMO_DB_VOLUME` | `xdi-demo-db`, `xdi-demo-db_demo-db-data` | Tên container và volume của database mẫu khi chạy `--profile demo` |
| `SOURCE_SECRET_KEY` | trống (tự sinh) | Khóa mã hóa thông tin kết nối của nguồn database, 32 byte base64 (bên dưới) |
| `SOURCE_ALLOWED_HOSTS` | trống | Máy chủ database được phép thêm làm nguồn, cách nhau bằng dấu phẩy; trống thì không thêm được nguồn nào |
| `SOURCE_TABLE_MAX_MB` | `2048` | Trần dung lượng một bảng khi chụp (theo `pg_table_size`, chưa nén); bảng lớn hơn chỉ lưu cấu trúc |
| `SOURCE_SNAPSHOT_MAX_MB` | `4096` | Trần tổng dung lượng các bảng chép dữ liệu trong một lần chụp; bảng nhỏ lấy trước |
| `SOURCE_TIMEOUT_SECS` | `300` | Thời gian tối đa của bước chép một lần chụp; quá thì bỏ cả lần chụp |
| `SOURCE_MAX_ROWS` | trống (không trần) | Trần số dòng một bảng, chỉ khi đặt; bảng quá trần chỉ lưu cấu trúc |
| `SOURCE_RESNAPSHOT_HOURS` | trống (tắt) | Chụp lại mọi nguồn database mỗi N giờ; `0` hay trống là tắt |
| `SOURCE_SNAPSHOT_MODE` | `direct` | `direct`: DuckDB đọc thẳng Postgres bằng extension `postgres`; `copy`: Postgres `COPY` ra CSV tạm |
| `SOURCE_POSTGRES_EXTENSION` | `/usr/local/lib/xdi/postgres_scanner.duckdb_extension` | File extension `postgres` của DuckDB cho `direct`; image Docker có sẵn |
| `CARGO_BUILD_JOBS` | trống (mọi nhân) | Số tiến trình biên dịch khi build image (mục 6) |
| `COMPOSE_PROJECT_NAME` | tên thư mục | Tên project compose, cũng là tiền tố của volume và image |
| `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` | trống | Để dành cho đăng nhập bằng Google; backend chưa đọc |

**Khóa nguồn database.** `SOURCE_SECRET_KEY` để trống thì lần đầu khởi động backend sinh một khóa ngẫu nhiên, ghi vào
`DATA_DIR/source_secret.key` (quyền 0600; compose: volume `xdidata`) và các lần sau đọc lại; log ghi một dòng khi sinh,
không in khóa. Mất file đó (ví dụ `docker compose down -v`) là mất mật khẩu của mọi nguồn đã lưu, phải thêm lại nguồn;
đổi khóa cũng vậy. `make backup` chép cả file khóa, nên ai có bản sao lưu là giải mã được thông tin kết nối. Bản chạy
thật nên đặt biến này (`openssl rand -base64 32`) và giữ khóa ngoài máy chủ.

**Máy chủ được phép và quyền đọc.** Máy ngoài `SOURCE_ALLOWED_HOSTS` bị từ chối, để backend không bị dùng dò máy trong
mạng nội bộ; riêng máy và cổng của `SAMPLE_DATABASE_URL` luôn được phép. Kết nối mặc định chụp mọi bảng tài khoản đọc
được, nên cấp cho xdi một tài khoản Postgres chỉ có quyền `SELECT` trên các bảng cần hỏi.

**Trần khi chụp.** Ba trần dung lượng và thời gian được xét trước khi chép, nên bảng quá trần không để lại file nào; bảng
đó chỉ lưu cấu trúc và chụp riêng được sau khi tăng trần. Số đo dùng để đặt các trần ở [PERFORMANCE.md](PERFORMANCE.md),
cơ chế ở [Dữ liệu](DATA.md#chụp-mọi-bảng-đọc-được). `.env` chép từ `.env.example` bản cũ có dòng `SOURCE_MAX_ROWS=1000000`:
xóa dòng đó để bỏ trần dòng. `direct` mà extension không nạp được (thiếu file, lệch bản DuckDB) hay không kết nối được
thì log ghi cảnh báo và lần chụp dùng `copy`.

**Database mẫu.** `SAMPLE_DATABASE_URL` là các chuỗi kết nối cách nhau bằng dấu phẩy (dấu phẩy trong mật khẩu viết
`%2C`). Compose mặc định trỏ vào `postgres://analyst:analyst@demo-db:5432/sales_demo` và `.../edu_demo`; `make dev` trỏ
vào hai database đó ở `127.0.0.1:DEMO_DB_PORT`; chạy binary trực tiếp thì mặc định trống. Không database nào tới được
(không bật `--profile demo`, chưa nạp dữ liệu) thì backend ghi cảnh báo và nạp `data/samples/dataco.csv`. Đặt
`SAMPLE_DATABASE_URL=` (rỗng) trong `.env` để không thử kết nối. Make không đọc `.env`: đổi `DEMO_DB_PORT` thì chạy kèm
cùng giá trị, ví dụ `DEMO_DB_PORT=5441 make eval`. Chi tiết: [data/README.md](https://github.com/pgiahuy/xdi-platform/blob/main/data/README.md).

### Mô hình ngôn ngữ

Chọn trước khi nạp dữ liệu: sau mỗi lần nạp hay chụp, backend gọi mô hình đang cấu hình để lập hồ sơ (mô tả cột, bí
danh, câu gợi ý). Đổi mô hình rồi khởi động lại thì backend lập lại hồ sơ còn thiếu.

- `mock`: không cần gì, chỉ trả lời các câu mẫu viết sẵn cho DataCo và các bảng của database mẫu
  ([Dùng lần đầu](USAGE.md#1-chọn-mô-hình-và-dữ-liệu-mẫu)).
- `gemini`: đặt `GEMINI_API_KEY`. Thiếu khóa thì backend vẫn chạy, ghi cảnh báo lúc khởi động, câu hỏi nhận lỗi nhắc đặt
  khóa. Khóa chỉ gửi tới Google trong header, không ghi vào log. Mặc định là bản lite vì hạn mức miễn phí theo ngày cao
  hơn bản flash; hạn mức hiện tại xem trong Google AI Studio. `GEMINI_BACKUP_API_KEY` (thường của một tài khoản Google
  khác) chỉ được dùng khi khóa chính trả 429; lỗi khác không đổi khóa.
- `ollama`: cần Ollama đang chạy và mô hình đã tải (`ollama pull qwen2.5-coder:7b`). Chạy compose thì backend gọi Ollama
  trên máy host qua `host.docker.internal`. Trên Linux, Ollama mặc định chỉ nghe `127.0.0.1`, container không gọi tới
  được: chạy Ollama với `OLLAMA_HOST=0.0.0.0` và chặn cổng 11434 từ bên ngoài, vì Ollama không có đăng nhập.
- `OLLAMA_NUM_CTX`: không gửi thì Ollama dùng mặc định của nó ([tài liệu Ollama](https://docs.ollama.com/context-length))
  và lặng lẽ bỏ phần đầu của prompt dài hơn, tức phần luật. Backend ghi cảnh báo `prompt is likely longer than
  OLLAMA_NUM_CTX` khi một prompt ước lượng vượt giá trị đã đặt; gặp cảnh báo đó (schema rộng, nhiều cột có mô tả) thì
  tăng, ví dụ `32768`. Ngữ cảnh lớn hơn cần thêm bộ nhớ K/V, xem bằng `ollama ps`.
- Bước phân loại câu hỏi chạy trước bước viết SQL nên có trần riêng `CLASSIFY_TIMEOUT_SECS`, không dài hơn trần của nhà
  cung cấp. Quá trần thì backend dùng luật từ khóa và câu hỏi chạy tiếp. Với Ollama, lần gọi đầu còn nạp mô hình vào
  RAM nên có thể quá trần; máy chậm thì tăng giá trị này.

`EVAL_GEMINI_API_KEY` dùng cho mọi lệnh bắt đầu bằng `eval` (`eval`, `eval-classify`, `eval-judge`, `eval-lessons`), để
chạy bộ đánh giá không ăn vào hạn mức của `GEMINI_API_KEY`. Không đặt thì các lệnh đó dùng `GEMINI_API_KEY`. Cách chạy
bộ đánh giá: [eval/README.md](https://github.com/pgiahuy/xdi-platform/blob/main/eval/README.md).

### Dữ liệu, dừng, cập nhật, gỡ

Dữ liệu nằm trong hai volume Docker: `xdi-platform_pgdata` (Postgres) và `xdi-platform_xdidata` (file Parquet, khóa
nguồn). Tiền tố là tên project compose, mặc định là tên thư mục chứa `docker-compose.yml`; image backend cũng mang tên
đó, ví dụ `xdi-platform-backend`.

```bash
docker compose down                       # dừng, giữ dữ liệu
git pull && docker compose up -d --build  # cập nhật; backend tự áp migration khi khởi động
docker compose down -v                    # gỡ hẳn, XÓA cả hai volume dữ liệu
docker compose down -v --rmi local        # gỡ hẳn và xóa image backend
```

`docker compose --profile demo down -v` xóa cả volume của database mẫu (`xdi-demo-db_demo-db-data`), phải tải và nạp lại
bằng `load.sh`. Chỉ gỡ ứng dụng thì chạy `down -v` không kèm profile. `docker builder prune` xóa cache build của mọi
project trên máy.

## 2. Build từ mã nguồn

Cần Rust stable (`rustup`; CI dùng bản stable mới nhất, image Docker dùng 1.96) kèm trình biên dịch C++ (`g++` hoặc
`clang`) để biên dịch DuckDB, Node.js 24 (CI dùng bản này), GNU Make và Docker (chỉ để chạy Postgres).

```bash
git clone https://github.com/pgiahuy/xdi-platform.git
cd xdi-platform
cp .env.example .env
make hooks          # một lần: hook pre-push chạy kiểm tra trước khi push (cho người đóng góp)
make dev            # Postgres trong Docker, backend bằng cargo run ở cổng 8000
make dev-frontend   # terminal khác: giao diện ở http://localhost:5173, gọi /api sang cổng 8000
```

`make dev` dùng database mẫu ở `127.0.0.1:5440` nếu nó đang chạy ([data/postgres-demo](https://github.com/pgiahuy/xdi-platform/blob/main/data/postgres-demo/README.md)),
không thì nạp `dataco.csv`. Muốn backend phục vụ giao diện như bản chạy thật:

```bash
cd frontend && npm ci && npm run build && cd ..
STATIC_DIR=$PWD/frontend/dist make dev   # mở http://localhost:8000
```

Backend chạy bằng `make dev` không đọc `.env` (chỉ bước bật Postgres đọc); biến cần đổi thì đặt trên dòng lệnh, ví dụ
khi đã đổi `POSTGRES_PORT=5434` và muốn backend ở cổng 8001, hay khi dùng Gemini:

```bash
DATABASE_URL=postgres://xdi:xdi@localhost:5434/xdi BACKEND_PORT=8001 make dev
BACKEND_URL=http://localhost:8001 make dev-frontend
LLM_PROVIDER=gemini GEMINI_API_KEY=<khóa> make dev
```

Mã thiết lập in ra terminal. File dữ liệu nằm ở `backend/data` (`DATA_DIR` mặc định `./data`, tính từ `backend/`), git
bỏ qua thư mục này. Đã chạy bản Docker trong cùng thư mục thì `make dev` dùng lại Postgres của bản đó (tài khoản đã có,
không in mã thiết lập), còn file dữ liệu thì không chung. Backend Docker đang giữ cổng 8000 thì dừng nó
(`docker compose stop backend`) hoặc đặt `BACKEND_PORT` khác.

`backend/target` lớn dần qua mỗi lần build, có thể lên hàng chục GB (`cargo clean` để xóa). Máy ít RAM thì giới hạn số
tiến trình biên dịch, ví dụ `CARGO_BUILD_JOBS=4 make dev`. `make help` liệt kê mọi lệnh.

Cài binary vào hệ thống (mặc định `/usr/local/bin`, đổi bằng `PREFIX`; binary lấy từ `backend/target` hay
`CARGO_TARGET_DIR`):

```bash
make install PREFIX=$HOME/.local
make uninstall PREFIX=$HOME/.local
```

Binary cần một Postgres (`DATABASE_URL`), thư mục dữ liệu ghi được (`DATA_DIR`), thư mục giao diện đã build nếu muốn có
giao diện (`STATIC_DIR`), và `SAMPLES_DIR` trỏ tới `data/samples` nếu muốn có dataset mẫu lúc thiết lập lần đầu (không
thấy file mẫu thì backend bỏ qua bước này).

Extension `postgres` của DuckDB cho `SOURCE_SNAPSHOT_MODE=direct` là một file nạp bằng đường dẫn; backend không tải
extension lúc chạy. `make dev` tự tải vào `duckdb-extensions/` (`make duckdb-extension`, kiểm SHA-256) và trỏ tới đó;
image Docker có sẵn file; gói binary của mỗi bản phát hành kèm `postgres_scanner.duckdb_extension`, chép vào
`/usr/local/lib/xdi/` hoặc đặt `SOURCE_POSTGRES_EXTENSION`. Cách nâng phiên bản: chú thích đầu
`scripts/fetch-duckdb-extension.sh`. Giấy phép của extension: [LICENSES.md](LICENSES.md).

## 3. Đưa lên Internet

Cần:

- Một máy chủ Linux có Docker như mục 1, Docker Compose 2.24.4 trở lên (bản đầu có thẻ `!override` mà
  `docker-compose.prod.yml` dùng, theo https://docs.docker.com/reference/compose-file/merge/).
- Một tên miền hoặc subdomain riêng (ví dụ `du-lieu.example.com`) trỏ về IP của máy chủ. Chạy dưới đường dẫn con
  (`example.com/xdi/`) chưa hỗ trợ: giao diện và API dùng đường dẫn tính từ gốc tên miền.
- Cổng 80 và 443 mở ra Internet cho reverse proxy.

### Cấu hình bản chạy thật

1. Clone và tạo `.env` như mục 1, rồi **trước khi chạy lần đầu** thêm ba dòng (thay tên miền):

   ```bash
   cat >> .env <<EOF
   COMPOSE_FILE=docker-compose.yml:docker-compose.prod.yml
   POSTGRES_PASSWORD=$(openssl rand -hex 24)
   PUBLIC_URL=https://du-lieu.example.com
   EOF
   ```

   Dòng `COMPOSE_FILE` làm mọi lệnh `docker compose` (và `make backup`, `make restore`) dùng thêm
   `docker-compose.prod.yml`, file chỉ ghi phần khác với bản chạy trên máy:

   - Postgres không mở cổng ra máy host. Docker tự mở cổng của container qua iptables, tường lửa như ufw không chặn
     được, nên cách chắc chắn là không mở.
   - Backend chỉ nghe trên `127.0.0.1:8000`; reverse proxy trên cùng máy nhận HTTPS rồi chuyển vào.
   - Thiếu `POSTGRES_PASSWORD` hoặc `PUBLIC_URL` thì `docker compose` dừng và báo thiếu biến nào.
   - `PUBLIC_URL` bắt đầu bằng `https://` thì cookie đăng nhập có cờ `Secure`.
   - Hai container tự chạy lại khi máy chủ khởi động lại.
   - `SAMPLE_DATABASE_URL` rỗng: thiết lập lần đầu nạp `dataco.csv`. Muốn thử database mẫu trên máy chủ thì chạy thêm
     `--profile demo` và đặt `SAMPLE_DATABASE_URL` trước lần thiết lập đầu.

   Kết nối database của tổ chức cần `SOURCE_ALLOWED_HOSTS`, và nên đặt `SOURCE_SECRET_KEY` (mục 1, Cấu hình). Tự đặt
   mật khẩu Postgres thì chỉ dùng chữ và số: mật khẩu nằm trong chuỗi kết nối, ký tự như `@`, `:`, `/` làm hỏng chuỗi.

2. Chạy và tạo tài khoản quản trị như mục 1 (`curl http://localhost:8000` vẫn chạy trên máy chủ). Kiểm Postgres không
   mở cổng ra máy host:

   ```bash
   docker compose ps                         # cột PORTS của postgres chỉ có 5432/tcp, không có 0.0.0.0:...->5432
   ss -ltnp | grep -E ':8000|:5433'          # chỉ thấy 127.0.0.1:8000, không thấy 5433
   ```

   Log của backend không có dòng `sample Postgres password` (`docker compose logs backend | grep -i password`).

3. Cài một reverse proxy theo một trong hai mẫu dưới.

**Đã chạy bằng mật khẩu mẫu rồi mới chuyển sang cấu hình này.** `POSTGRES_PASSWORD` chỉ có tác dụng khi Postgres tạo
volume lần đầu, nên đổi mật khẩu trong database trước, khi chưa sửa `.env`:

```bash
docker compose exec postgres psql -U xdi -d xdi -c "ALTER USER xdi PASSWORD 'mat-khau-moi'"
```

Rồi thêm ba dòng trên vào `.env` (cùng mật khẩu đó) và chạy `docker compose up -d`. Backend còn dùng mật khẩu mẫu thì
mỗi lần khởi động in cảnh báo `DATABASE_URL uses the sample Postgres password`.

### Caddy

Caddy tự xin và gia hạn chứng chỉ HTTPS, không giới hạn dung lượng file gửi lên hay thời gian chờ backend. Cài theo
https://caddyserver.com/docs/install, rồi thay nội dung `/etc/caddy/Caddyfile` bằng:

```
du-lieu.example.com {
    reverse_proxy localhost:8000
    header Strict-Transport-Security "max-age=31536000"
}
```

Rồi `sudo systemctl reload caddy` và mở https://du-lieu.example.com.

### nginx

Máy chủ đã có nginx thì dùng nó thay Caddy. Chứng chỉ lấy bằng certbot (https://certbot.eff.org); mẫu dưới giả sử
certbot đã ghi chứng chỉ vào `/etc/letsencrypt/live/du-lieu.example.com/`. Ghi vào `/etc/nginx/conf.d/xdi.conf`:

```nginx
server {
    listen 443 ssl;
    server_name du-lieu.example.com;
    ssl_certificate     /etc/letsencrypt/live/du-lieu.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/du-lieu.example.com/privkey.pem;

    # MAX_UPLOAD_MB cộng 1 (mặc định 200 + 1). Mặc định nginx chỉ nhận 1m, file lớn hơn bị trả 413.
    client_max_body_size 201m;
    # Backend chỉ trả lời khi đã đọc xong file. Một file Excel được đọc tới 360 giây và mỗi lúc chỉ đọc một file,
    # file thứ hai phải chờ file trước. Mặc định nginx chờ 60 giây rồi trả 504.
    proxy_read_timeout 900s;
    # Chuyển file sang backend ngay khi nhận, không ghi tạm ra đĩa của nginx trước.
    proxy_request_buffering off;

    # HSTS do proxy gửi, vì chỉ proxy biết chắc trình duyệt đang vào bằng HTTPS.
    add_header Strict-Transport-Security "max-age=31536000" always;

    location / {
        proxy_pass http://127.0.0.1:8000;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}

server {
    listen 80;
    server_name du-lieu.example.com;
    return 301 https://$host$request_uri;
}
```

Rồi `sudo nginx -t && sudo systemctl reload nginx`. Đổi `MAX_UPLOAD_MB` thì đổi `client_max_body_size` theo.

### Header bảo mật

Backend tự gửi `Content-Security-Policy`, `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff` và
`Referrer-Policy: same-origin` cho cả giao diện và API. `Strict-Transport-Security` (HSTS) để proxy gửi như hai mẫu
trên; HSTS làm trình duyệt chỉ vào tên miền này bằng HTTPS trong một năm, nên chỉ bật khi tên miền sẽ luôn có HTTPS.

## 4. Sao lưu và khôi phục

Bản sao lưu gồm database (`db.dump`) và thư mục dữ liệu (`data.tgz`: file Parquet, file khóa nguồn). Chạy trong thư mục
chứa `docker-compose.yml`:

```bash
make backup                                     # ghi vào ~/xdi-backups/<ngày-giờ>/
make backup BACKUP_DIR=/mnt/backup/xdi          # chỗ khác
```

Bản sao lưu có dữ liệu và chuỗi băm mật khẩu: thư mục chỉ chủ sở hữu đọc được, mặc định nằm ngoài thư mục repo. Chép
nó sang một máy hoặc ổ đĩa khác.

Sao lưu chạy được khi backend đang chạy, nhưng database và file được chụp ở hai thời điểm liền nhau: ai đó nạp hay xóa
dữ liệu đúng lúc đó thì hai phần có thể không khớp. Cần chắc chắn thì dừng backend trong lúc sao lưu:

```bash
docker compose stop backend && make backup && docker compose up -d
```

Khôi phục, trên bản cài đang chạy hoặc một bản cài mới (đã clone và tạo `.env` như mục 1 hoặc mục 3):

```bash
make restore FROM=~/xdi-backups/20261022-101500
```

Lệnh này **thay toàn bộ** dữ liệu hiện có. Trước hết nó đọc hết `db.dump` và `data.tgz`; file nào thiếu, hỏng hay bị
cắt cụt thì dừng và báo, chưa xóa gì. Đọc được thì nó dừng backend, tạo lại database từ `db.dump`, giải nén lại thư mục
dữ liệu, rồi chạy lại. Tài khoản, workspace và dataset trở về như lúc sao lưu; mật khẩu Postgres giữ của bản cài hiện
tại.

Khôi phục vào bản cài cùng phiên bản hoặc mới hơn (backend tự áp migration còn thiếu). Bản cài cũ hơn thì backend không
khởi động được vì database có migration mà code cũ không biết: `git pull` trước khi khôi phục. Không dùng Makefile thì
chạy tay đúng các lệnh trong target `backup` và `restore`.

## 5. Quản trị

Quên mật khẩu quản trị: chạy trên máy chủ, lệnh in một mật khẩu tạm và đăng xuất mọi phiên của tài khoản đó. Đăng nhập
bằng mật khẩu tạm rồi đổi ở **Tài khoản và mật khẩu** (menu tài khoản ở chân thanh bên).

```bash
docker compose exec backend xdi-backend reset-password --email ban@example.com
docker compose exec backend xdi-backend list-admins   # các quản trị hệ thống: email, tên, trạng thái
```

Build từ mã nguồn thì chạy cùng lệnh với cùng `DATABASE_URL`, ví dụ `cd backend && cargo run -- list-admins`, hoặc
`xdi-backend list-admins` nếu đã `make install`. Tạo tài khoản và thêm thành viên trên giao diện:
[Dùng lần đầu](USAGE.md#9-mời-thành-viên).

## 6. Lỗi hay gặp

**Build Docker làm máy đứng hoặc tiến trình biên dịch bị dừng vì hết RAM.** Đặt số tiến trình bằng khoảng số GiB RAM
còn trống (`free -g`, cột `available`) vào `.env`, ví dụ `CARGO_BUILD_JOBS=4`, rồi `docker compose up -d --build`. Biến
này giới hạn cả phần C++ của DuckDB; giữ dòng đó thì các lần cập nhật sau cũng theo giới hạn.

Đã đặt `CARGO_BUILD_JOBS` mà máy vẫn đứng: các bước khác của build (cài gói npm, build giao diện) vẫn dùng mọi nhân.
Builder buildx riêng giới hạn mọi bước trong `N` nhân (`N` không quá `nproc`):

```bash
N=4   # số nhân dùng để build
docker buildx create --name xdi-build --driver docker-container --driver-opt cpuset-cpus=0-$((N - 1))
docker buildx build --builder xdi-build --load -f backend/Dockerfile \
  -t "$(docker compose config --images | grep backend)" .
docker compose up -d --no-build
```

Lần cập nhật sau: `git pull`, rồi chạy lại `docker buildx build` và `docker compose up -d --no-build`. Giữ builder
`xdi-build` vì cache build nằm trong nó.

**Build Docker lỗi `spurious network error` hoặc `transfer too slow` khi tải crate.** Mạng của Docker trên một số máy
Linux chậm hoặc chập chờn. Build bằng mạng của máy host (tên image lấy từ `docker compose config` cho khớp tên compose):

```bash
docker build --network host -f backend/Dockerfile -t "$(docker compose config --images | grep backend)" .
docker compose up -d --no-build
```

**Cổng đã có chương trình khác dùng.** `docker compose up` báo `port is already allocated` hay `address already in
use`; `make dev` báo `cannot bind port, check BACKEND_PORT`.

- Docker: đổi `BACKEND_PORT` hoặc `POSTGRES_PORT` trong `.env`.
- `make dev`: đặt `BACKEND_PORT` trên dòng lệnh; đã đổi `POSTGRES_PORT` thì đặt cả `DATABASE_URL`, không thì backend báo
  `cannot connect to postgres, check DATABASE_URL`.

Hai bản cài trên cùng máy thì bản thứ hai đổi cả hai cổng. Hai thư mục cùng tên (cùng là `xdi-platform`) thì còn phải
đặt `COMPOSE_PROJECT_NAME` khác, không thì bản thứ hai dùng chung volume của bản thứ nhất.

**Không thấy mã thiết lập trong log.** Backend chỉ in mã khi database chưa có tài khoản nào. Đã tạo tài khoản ở lần chạy
trước thì dùng `reset-password` (mục 5). Mã in ở mức `warn`, nên `RUST_LOG=error` cũng làm mất dòng này.

**Hỏi thì nhận `Chế độ mock chỉ trả lời các câu hỏi mẫu của dataset mẫu`, hoặc trang hỏi ghi "Mô hình hiện tại không lập
được câu gợi ý cho dataset này".** Backend đang chạy với `LLM_PROVIDER=mock`. Đổi sang `gemini` hoặc `ollama` rồi khởi
động lại; backend lập hồ sơ cho các dataset còn thiếu, không phải nạp lại.

**Lỗi khi dùng Gemini.** Lời báo hiện dưới ô câu hỏi. Log có thêm dòng `gemini returned an error` kèm mã HTTP
(`status`), mã của Google (`gemini_status`) và lời gốc. Lỗi lúc lập hồ sơ sau khi nạp chỉ có trong log; sửa xong và khởi
động lại backend thì backend lập lại hồ sơ.

- `Thiếu khóa Gemini: đặt GEMINI_API_KEY trong .env rồi khởi động lại backend`.
- `Gemini từ chối khóa API (mã 403): kiểm tra GEMINI_API_KEY` (có khi mã 400 hay 401): khóa sai, chép thiếu, hoặc không
  phải khóa Gemini API tạo trong Google AI Studio.
- `Gemini không có mô hình ...: kiểm tra GEMINI_MODEL` (mã 404): khóa này không gọi được mô hình đó, đã gặp với khóa mới
  và mô hình cũ như `gemini-2.5-flash`. Bỏ dòng `GEMINI_MODEL` để dùng mặc định, hoặc chọn mô hình có trong Google AI
  Studio.
- `Gemini đang lỗi hoặc quá tải (mã 503). Thử lại sau`: chờ rồi hỏi lại; lặp lại nhiều lần thì thử mô hình khác.
- `Đã hết hạn mức gọi Gemini của khóa này...` (mã 429): hạn mức theo phút thì chờ một phút, theo ngày thì chờ sang ngày,
  đặt `GEMINI_BACKUP_API_KEY`, hoặc xem gói trả phí trong Google AI Studio.
- `Không kết nối được tới Gemini` hoặc `Gemini không trả lời trong 60 giây`: máy chủ không ra được Internet (tường lửa,
  proxy `HTTPS_PROXY`) hoặc mô hình trả lời chậm; số giây theo `GEMINI_TIMEOUT_SECS`.

**Hết dung lượng đĩa.** `cargo clean` trong `backend/` xóa thư mục build; `docker image prune` xóa image cũ,
`docker builder prune` xóa cache build, `docker buildx rm xdi-build` xóa builder riêng cùng cache của nó.

**Caddy không khởi động, báo cổng 80 hoặc 443 đang bận.** Một web server khác (nginx, apache) đang dùng cổng đó. Tắt nó,
hoặc cấu hình chính nó làm reverse proxy tới `localhost:8000`.
